use std::{
    collections::HashMap,
    fs::{self},
    str::FromStr,
    sync::Mutex,
};

use actix_session::{
    config::BrowserSession, storage::CookieSessionStore, Session, SessionMiddleware,
};
use actix_web::{
    cookie::Key,
    dev::PeerAddr,
    http::StatusCode,
    middleware,
    web::{self},
    App, Error, HttpRequest, HttpResponse, HttpServer,
};
use futures_util::StreamExt as _;
use log::debug;
use reqwest::Client;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use url::Url;
use uuid::Uuid;

use crate::cli::*;
use crate::records::*;

/// Forwards the incoming HTTP request.
pub async fn forward(
    req: HttpRequest,
    mut payload: web::Payload,
    url: web::Data<Url>,
    client: web::Data<Client>,
    record_options: web::Data<RecordOptions>,
    session: Session,
    record_sessions: web::Data<SessionState>,
    peer_addr: Option<PeerAddr>,
) -> Result<HttpResponse, Error> {
    let mut sessions_lock = record_sessions.sessions.lock().unwrap();
    let use_record_dir = record_options.record_dir != "";
    let mut new_url = (**url).clone();
    new_url.set_path(req.uri().path());
    new_url.set_query(req.uri().query());
    let method = req.method().to_string();
    let url = req.uri().to_string();
    let identifier = format!("{}:{}", method, url).to_string();

    debug!("Handle request : {}", identifier);

    if use_record_dir {
        if let Some(session_id) = session.get::<String>("r-session")? {
            let mut session = sessions_lock
                .get(&session_id)
                .expect("Could not get session")
                .clone();
            let filepath = session.filepath.clone();
            let state = *session.states.get(&identifier).unwrap_or(&0);

            // Record dir and record mode off
            if record_options.record == false {
                if fs::metadata(filepath.clone()).is_ok() == false {
                    return Ok(HttpResponse::NotFound().body("No file found"));
                }

                let data = fs::read_to_string(filepath.clone()).expect("Cannot read record file");
                let record_file: HashMap<String, Vec<Record>> =
                    serde_json::from_str(data.as_str()).expect("Cannot parse record file");
                if let Some(records) = record_file.get(&identifier) {
                    if let Some(res) = records.get(state) {
                        let status = StatusCode::from_str(&res.status).unwrap_or(StatusCode::OK);

                        let mut client_resp = HttpResponse::build(status);

                        for (header_name, header_value) in res.headers.iter() {
                            client_resp.insert_header((header_name.clone(), header_value.clone()));
                        }

                        let mut new_session = RecordSession {
                            filepath,
                            states: session.states.clone(),
                            records: session.records.clone(),
                        };

                        new_session.states.insert(identifier, state + 1);

                        sessions_lock.insert(session_id, new_session);

                        return Ok(client_resp.body(res.body.clone()));
                    } else {
                        return Ok(HttpResponse::NotFound().body(format!("No record in position {} found", state)));
                    }
                } else {
                    return Ok(HttpResponse::NotFound().body("No identifier found"));
                }
            };

            // Record dir and record mode on
            if record_options.record {
                let (tx, rx) = mpsc::unbounded_channel();

                actix_web::rt::spawn(async move {
                    while let Some(chunk) = payload.next().await {
                        tx.send(chunk).unwrap();
                    }
                });

                let forwarded_req = client
                    .request(req.method().clone(), new_url)
                    .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));

                // TODO: This forwarded implementation is incomplete as it only handles the unofficial
                // X-Forwarded-For header but not the official Forwarded one.
                let forwarded_req = match peer_addr {
                    Some(PeerAddr(addr)) => {
                        forwarded_req.header("x-forwarded-for", addr.ip().to_string())
                    }
                    None => forwarded_req,
                };

                let res = forwarded_req
                    .send()
                    .await
                    .map_err(actix_web::error::ErrorInternalServerError)?;

                let res_status = res.status();
                let res_headers = res.headers().clone();
                let res_body = res.text().await.expect("Cannot get response");

                let mut new_record = Record {
                    body: String::default(),
                    headers: HashMap::new(),
                    status: res_status.to_string(),
                };

                new_record.body = res_body.clone();

                let mut client_resp = HttpResponse::build(res_status);
                // Remove `Connection` as per
                // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
                for (header_name, header_value) in
                    res_headers.iter().filter(|(h, _)| *h != "connection")
                {
                    client_resp.insert_header((header_name.clone(), header_value.clone()));
                    new_record.headers.insert(
                        header_name.to_string(),
                        header_value.to_str().unwrap_or("").to_string(),
                    );
                }

                let mut record_array = Vec::<Record>::new();
                if let Some(existing_array) = session.records.get(&identifier) {
                    record_array = existing_array.clone();
                    record_array.push(new_record);
                } else {
                    record_array.push(new_record);
                }

                session
                    .records
                    .insert(identifier.clone(), record_array.to_vec());
                sessions_lock.insert(session_id, session);

                return Ok(client_resp.body(res_body));
            };
        } else {
            return Ok(HttpResponse::InternalServerError().body("No session set"));
        };
    }

    let (tx, rx) = mpsc::unbounded_channel();

    actix_web::rt::spawn(async move {
        while let Some(chunk) = payload.next().await {
            tx.send(chunk).unwrap();
        }
    });

    let forwarded_req = client
        .request(req.method().clone(), new_url)
        .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));

    // TODO: This forwarded implementation is incomplete as it only handles the unofficial
    // X-Forwarded-For header but not the official Forwarded one.
    let forwarded_req = match peer_addr {
        Some(PeerAddr(addr)) => forwarded_req.header("x-forwarded-for", addr.ip().to_string()),
        None => forwarded_req,
    };

    let res = forwarded_req
        .send()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut client_resp = HttpResponse::build(res.status());
    // Remove `Connection` as per
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.insert_header((header_name.clone(), header_value.clone()));
    }

    return Ok(client_resp.streaming(res.bytes_stream()));
}

// Start a record session
pub async fn start_record_handler(
    session: Session,
    path: web::Path<String>,
    record_options: web::Data<RecordOptions>,
    record_sessions: web::Data<SessionState>,
) -> HttpResponse {
    let mut sessions_lock = match record_sessions.sessions.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let record_name = path.into_inner();
    let session_id = Uuid::new_v4();
    let record_session = RecordSession {
        filepath: format!(
            "{}/{}.snap",
            record_options.record_dir.trim_end_matches('/'),
            record_name
        ),
        states: HashMap::new(),
        records: HashMap::new(),
    };
    sessions_lock.insert(session_id.to_string(), record_session);
    return match session.insert("r-session", session_id.to_string()) {
        Ok(_) => HttpResponse::Ok().body("Session started"),
        Err(_) => HttpResponse::InternalServerError().body("Session error"),
    };
}

// End a record session
pub async fn end_record_handler(
    session: Session,
    record_options: web::Data<RecordOptions>,
    record_sessions: web::Data<SessionState>,
) -> Result<HttpResponse, Error> {
    if let Some(session_id) = session.get::<String>("r-session")? {
        let was_recording = record_options.record_dir != "" && record_options.record == true;

        if was_recording {
            let mut sessions_lock = record_sessions.sessions.lock().unwrap();
            let record_session = sessions_lock
                .get(&session_id)
                .expect("Could not get session");
            let filepath = record_session.filepath.clone();

            debug!(
                "Number of records to write : {}",
                record_session.records.len()
            );

            let data = serde_json::to_string(&record_session.records)
                .expect("Cannot parse in-memory records");
            fs::create_dir_all(record_options.record_dir.to_string())?;
            debug!("Writing to {}", filepath);
            fs::write(filepath, data).expect("Cannot write to file");

            sessions_lock.remove(&session_id);
            return Ok(HttpResponse::Ok().body("Record saved"));
        } else {
            return Ok(HttpResponse::Ok().body("Not recording"));
        }
    } else {
        return Ok(HttpResponse::BadRequest().body("No session was started"));
    }
}


// Clear all sessions
pub async fn clear_sessions(record_sessions: web::Data<SessionState>, session: Session) -> Result<HttpResponse, Error>{
            let mut sessions_lock = record_sessions.sessions.lock().unwrap();
            sessions_lock.clear();
            session.clear();
            return Ok(HttpResponse::Ok().body("Sessions cleared"));
}

pub async fn launch_app(args: CliArguments) -> std::io::Result<()> {
    let forward_url = Url::parse(&args.forward_to).expect("Forward address invalid");

    if args.record_dir != "" {
        if args.record {
            log::info!("MODE RECORD ENABLED");
        } else {
            log::info!("MODE REPLAY ENABLED");
        }
    } else {
        log::info!("MODE PASSTHROUGH ENABLED");
    }

    log::info!(
        "Starting proxy at http://{}:{}",
        &args.listen_addr,
        &args.port
    );

    log::info!("Forward request to {forward_url}");

    let record_options = RecordOptions {
        record_dir: args.record_dir,
        record: args.record,
    };

    let record_sessions = web::Data::new(SessionState {
        sessions: Mutex::new(HashMap::<String, RecordSession>::new()),
    });
    let reqwest_client = reqwest::Client::default();

    return HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(reqwest_client.clone()))
            .app_data(web::Data::new(forward_url.clone()))
            .app_data(web::Data::new(record_options.clone()))
            .app_data(record_sessions.clone())
            .wrap(middleware::Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::from(&[0; 64]))
                    .cookie_secure(false)
                    .cookie_name("r-session".to_string())
                    .session_lifecycle(BrowserSession::default())
                    .cookie_http_only(true)
                    .build(),
            )
            .service(web::resource("/end-record").route(web::post().to(end_record_handler)))
            .service(
                web::resource("/start-record/{name}").route(web::post().to(start_record_handler)),
            )
            .service(
                web::resource("/clear-sessions").route(web::post().to(clear_sessions)),
            )
            .default_service(web::to(forward))
    })
    .bind(format!("{}:{}", args.listen_addr, args.port))?
    .run()
    .await;
}
