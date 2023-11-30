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
    error,
    http::StatusCode,
    middleware, post,
    web::{self},
    App, Error, HttpRequest, HttpResponse, HttpServer,
};
use awc::Client;
use clap::Parser;
use log::debug;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct RecordOptions {
    record: bool,
    record_dir: String,
}

#[derive(Serialize, Deserialize)]
struct RecordSession {
    states: HashMap<String, usize>,
    filepath: String,
    records: HashMap<String, Vec<Record>>,
}

struct SessionState {
    sessions: Mutex<HashMap<String, RecordSession>>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Record {
    status: String,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Serialize, Deserialize)]
struct RecordFile {
    records: HashMap<String, Vec<Record>>,
}

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArguments {
    #[arg(short, long, default_value = "localhost")]
    listen_addr: String,
    #[arg(short, long, default_value_t = 3333)]
    port: i16,
    #[arg(short, long)]
    forward_to: String,
    #[arg(
        short = 'u',
        long,
        default_value_t = false,
        help = "Use this to update your snapshots",
        requires = "record_dir"
    )]
    record: bool,
    #[arg(
        short = 'd',
        long = "dir",
        help = "Directory where to store/to get records",
        default_value = ""
    )]
    record_dir: String,
}

/// Forwards the incoming HTTP request using `awc`.
async fn forward(
    req: HttpRequest,
    payload: web::Payload,
    url: web::Data<Url>,
    client: web::Data<Client>,
    record_options: web::Data<RecordOptions>,
    session: Session,
    record_sessions: web::Data<SessionState>,
) -> Result<HttpResponse, Error> {
    let mut sessions_lock = record_sessions.sessions.lock().unwrap();
    let use_record_dir = record_options.record_dir == "";
    let mut new_url = (**url).clone();
    new_url.set_path(req.uri().path());
    new_url.set_query(req.uri().query());

    let forwarded_req = client
        .request_from(new_url.as_str(), req.head())
        .no_decompress();

    if use_record_dir {
        if let Some(session_id) = session.get::<String>("r-session")? {
            let session = sessions_lock
                .get(&session_id)
                .expect("Could not get session");
            let filepath = session.filepath.clone();
            let data = fs::read_to_string(filepath.to_owned()).expect("Cannot read record file");
            let record_file: RecordFile =
                serde_json::from_str(data.as_str()).expect("Cannot parse record file");
            let method = req.method().to_string();
            let url = req.uri().to_string();
            // State req identifier to get counter
            let identifier = format!("{}:{}", method, url).to_string();
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
                session.records.to_owned().insert(identifier, record_array.to_vec());

                return Ok(client_resp.streaming(res));
            };
        } else {
            return Ok(HttpResponse::InternalServerError().body("No session set"));
        };
    }

    debug!("Passthrough");
    // Just a passthrough
    let res = forwarded_req
        .send_stream(payload)
        .await
        .map_err(error::ErrorInternalServerError)?;

    let mut client_resp = HttpResponse::build(res.status());

    for (header_name, header_value) in res.headers().iter() {
        client_resp.insert_header((header_name.clone(), header_value.clone()));
    }

    Ok(client_resp.streaming(res))
}

#[post("/start-record/{name}")]
async fn use_record_handler(
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

#[post("/end-record")]
async fn end_record_handler(
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

            let data = serde_json::to_string(&record_session.records).expect("Cannot parse in-memory records");
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = CliArguments::parse();

    let forward_url = Url::parse(&args.forward_to).expect("Forward address invalid");

    log::info!(
        "Starting HTTP server at http://{}:{}",
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

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Client::default()))
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
            .default_service(web::to(forward))
    })
    .bind(format!("{}:{}", args.listen_addr, args.port))?
    .run()
    .await
}
