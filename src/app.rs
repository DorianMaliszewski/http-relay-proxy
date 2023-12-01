use std::{
    collections::HashMap,
    fs::{self},
    str::FromStr,
    sync::{Arc, Mutex},
};

use actix_session::{
    config::BrowserSession, storage::CookieSessionStore, Session, SessionMiddleware,
};
use actix_web::{
    cookie::Key,
    dev::PeerAddr,
    error,
    http::StatusCode,
    middleware,
    web::{self},
    App, Error, HttpRequest, HttpResponse, HttpServer,
};
use awc::{http::header, Client, Connector};
use log::debug;
use rustls::{OwnedTrustAnchor, RootCertStore};
use url::Url;
use uuid::Uuid;

use crate::cli::*;
use crate::records::*;

/// Forwards the incoming HTTP request using `awc`.
pub async fn forward(
    req: HttpRequest,
    payload: web::Payload,
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
    // State req identifier to get counter
    let identifier = format!("{}:{}", method, url).to_string();

    debug!("Handle request : {}", identifier);

    if use_record_dir {
        if let Some(session_id) = session.get::<String>("r-session")? {
            let session = sessions_lock
                .get(&session_id)
                .expect("Could not get session");
            let filepath = session.filepath.clone();
            let data = fs::read_to_string(filepath.to_owned()).expect("Cannot read record file");
            let record_file: RecordFile =
                serde_json::from_str(data.as_str()).expect("Cannot parse record file");
            let state = *session.states.get(&identifier).unwrap_or(&0);

            // Record dir and record mode off
            if record_options.record == false {
                debug!("Read from record dir");
                if let Some(res) = record_file.records.get(&identifier).unwrap().get(state) {
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
                }
            };

            // Record dir and record mode on
            if record_options.record {
                debug!("Write to record dir");

                let forwarded_req = client
                    .request_from(new_url.as_str(), req.head())
                    .no_decompress();
                let mut res = forwarded_req
                    .send_stream(payload)
                    .await
                    .map_err(error::ErrorInternalServerError)?;

                let mut new_record = Record {
                    body: String::default(),
                    headers: HashMap::new(),
                    status: res.status().to_string(),
                };

                let body = res.body().await.expect("Cannot get body");
                new_record.body = String::from_utf8(body.to_vec()).expect("Cannot parse body");

                let mut client_resp = HttpResponse::build(res.status());

                for (header_name, header_value) in res.headers().iter() {
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
                    .to_owned()
                    .insert(identifier, record_array.to_vec());

                return Ok(client_resp.streaming(res));
            };
        } else {
            return Ok(HttpResponse::InternalServerError().body("No session set"));
        };
    }

    debug!("Passthrough: {}", identifier);

    let forwarded_req = client
        .request_from(new_url.as_str(), req.head())
        .no_decompress();

    let forwarded_req = match peer_addr {
        Some(PeerAddr(addr)) => {
            forwarded_req.insert_header(("x-forwarded-for", addr.ip().to_string()))
        }
        None => forwarded_req,
    };

    let res = forwarded_req
        .send_stream(payload)
        .await
        .map_err(error::ErrorInternalServerError)?;

    let mut client_resp = HttpResponse::build(res.status());
    // Remove `Connection` as per
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.insert_header((header_name.clone(), header_value.clone()));
    }

    return Ok(client_resp.streaming(res));
}

pub async fn start_record_handler(
    session: Session,
    path: web::Path<String>,
    record_options: web::Data<RecordOptions>,
    record_sessions: web::Data<SessionState>,
) -> HttpResponse {
    let mut sessions_lock = record_sessions.sessions.lock().unwrap();
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
    sessions_lock
        .insert(session_id.to_string(), record_session)
        .expect("Cannot write to session");
    match session.insert("r-session", session_id.to_string()) {
        Ok(_) => HttpResponse::Ok().body("Session started"),
        Err(_) => HttpResponse::InternalServerError().body("Session error"),
    }
}

pub async fn end_record_handler(
    session: Session,
    record_options: web::Data<RecordOptions>,
    record_sessions: web::Data<SessionState>,
) -> Result<HttpResponse, Error> {
    let was_recording = record_options.record_dir == "" && record_options.record;

    if was_recording {
        if let Some(session_id) = session.get::<String>("r-session")? {
            let mut sessions_lock = record_sessions.sessions.lock().unwrap();
            let record_session = sessions_lock
                .get(&session_id)
                .expect("Could not get session");
            let filepath = record_session.filepath.clone();

            let data = serde_json::to_string(&record_session.records)
                .expect("Cannot parse in-memory records");
            fs::write(filepath, data).expect("Cannot write to file");

            sessions_lock.remove(&session_id);
            return Ok(HttpResponse::Ok().body("Record saved"));
        } else {
            return Ok(HttpResponse::BadRequest().body("No session was started"));
        }
    } else {
        return Ok(HttpResponse::Ok().body("Not recording"));
    }
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

    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let client_config = Arc::new(
        rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth()
            .dangerous()
            .cfg
            .to_owned(),
    );

    return HttpServer::new(move || {
        // create client _inside_ `HttpServer::new` closure to have one per worker thread
        let client = Client::builder()
            // Wikipedia requires a User-Agent header to make requests
            .add_default_header((header::USER_AGENT, "http_relay_proxy/1.0"))
            // a "connector" wraps the stream into an encrypted connection
            .connector(Connector::new().rustls_021(Arc::clone(&client_config)))
            .finish();

        App::new()
            .app_data(web::Data::new(client.clone()))
            .app_data(web::Data::new(forward_url.clone()))
            .app_data(web::Data::new(record_options.clone()))
            .app_data(record_sessions.clone())
            .wrap(middleware::Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::from(&[0; 64]))
                    .cookie_secure(false)
                    .session_lifecycle(BrowserSession::default())
                    .cookie_http_only(true)
                    .build(),
            )
            .route("/end-record", web::post().to(end_record_handler))
            .route("/start-record/{name}", web::post().to(start_record_handler))
            .default_service(web::to(forward))
    })
    .bind(format!("{}:{}", args.listen_addr, args.port))?
    .run()
    .await;
}
