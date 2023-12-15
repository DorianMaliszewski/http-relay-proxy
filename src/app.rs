use cookie::Cookie;
use http::{HeaderName, HeaderValue, StatusCode};
use std::{
    collections::HashMap,
    fs::{self},
    str::FromStr,
    sync::Arc,
};

use hyper_util::rt::TokioIo;
use log::{debug, info, warn};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use uuid::Uuid;

use crate::config::Config;
use crate::records::*;
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{body::Incoming, client::conn::http1::Builder};
use hyper::{Method, Request, Response};

struct State {
    need_recording: bool,
    sessions: Mutex<HashMap<String, RecordSession>>,
    record_dir: String,
    hosts: Vec<String>,
}

type AppState = Arc<State>;
type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

const SET_COOKIE: &str = "Set-Cookie";


pub async fn launch_app(config: Config, need_recording: bool) -> std::io::Result<()> {
    if !config.record_dir.is_empty() {
        if need_recording {
            info!("MODE RECORD ENABLED");
        } else {
            info!("MODE REPLAY ENABLED");
        }
    } else {
        info!("MODE PASSTHROUGH ENABLED");
    }

    info!(
        "Starting proxy at {}:{}",
        &config.listen_addr,
        &config.listen_port
    );


    let addr =
        SocketAddr::from_str(&format!("{}:{}", &config.listen_addr, &config.listen_port)).unwrap();

    // We create a TcpListener and bind it to 127.0.0.1:3000
    let listener = TcpListener::bind(addr).await?;
    let app_state = Arc::new(State {
        need_recording,
        sessions: Mutex::new(HashMap::new()),
        hosts: config.hosts_to_record,
        record_dir: config.record_dir,
    });

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = app_state.clone();

        let service = service_fn(move |_req| handle_request(_req, state.clone()));

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service)
                .with_upgrades()
                .await
            {
                warn!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
async fn proxy(req: Request<hyper::body::Incoming>, state: AppState) -> Result<Response<BoxBody>> {
    let use_record_dir = !state.record_dir.is_empty();
    let method = req.method().clone();
    let url = req.uri().clone();
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let identifier = format!("{}:{}", method, url).to_string();

    info!("Handle request : {}", identifier);

    if use_record_dir {
        if let Some(session_id) = get_session(&req) {
            let mut sessions_lock = state.sessions.lock().await;
            let mut session = sessions_lock
                .get(&session_id)
                .expect("Could not get session")
                .clone();
            let filepath = session.filepath.clone();
            let record_state = *session.states.get(&identifier).unwrap_or(&0);

            if !state.need_recording {
                if fs::metadata(filepath.clone()).is_err() {
                    Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(full("No file found"))
                        .unwrap())
                } else {
                    let data =
                        fs::read_to_string(filepath.clone()).expect("Cannot read record file");
                    let record_file: HashMap<String, Vec<Record>> =
                        serde_json::from_str(data.as_str()).expect("Cannot parse record file");
                    if let Some(records) = record_file.get(&identifier) {
                        if let Some(res) = records.get(record_state) {
                            let status =
                                StatusCode::from_str(&res.status).unwrap_or(StatusCode::OK);

                            let mut client_resp = Response::builder().status(status);

                            for (header_name, header_value) in res.headers.iter() {
                                client_resp.headers_mut().unwrap().insert(
                                    HeaderName::from_str(header_name.as_str()).unwrap(),
                                    HeaderValue::from_str(header_value.as_str()).unwrap(),
                                );
                            }

                            let mut new_session = RecordSession {
                                filepath,
                                states: session.states.clone(),
                                records: session.records.clone(),
                            };

                            new_session.states.insert(identifier, record_state + 1);

                            sessions_lock.insert(session_id, new_session);

                            Ok(client_resp.body(full(res.body.clone())).unwrap())
                        } else {
                            Ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(full(format!(
                                    "No record in position {} found",
                                    record_state
                                )))
                                .unwrap())
                        }
                    } else {
                        Ok(Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(full("No identifier found"))
                            .unwrap())
                    }
                }
            } else {
                let stream = TcpStream::connect((host, port)).await.unwrap();
                let io = TokioIo::new(stream);


                let (mut sender, conn) = Builder::new()
                    .preserve_header_case(true)
                    .title_case_headers(true)
                    .handshake(io)
                    .await?;
                tokio::task::spawn(async move {
                    if let Err(err) = conn.await {
                        warn!("Connection failed: {:?}", err);
                    }
                });

                let resp = sender.send_request(req).await?;
                let res_status = resp.status();
                let res_headers = resp.headers().clone();
                let res_body =
                    String::from_utf8(resp.into_body().collect().await?.to_bytes().to_vec())
                        .unwrap_or(String::default());

                let mut new_record = Record {
                    body: String::default(),
                    headers: HashMap::new(),
                    status: res_status.to_string(),
                };

                new_record.body = res_body.clone();

                let mut client_resp = Response::builder().status(res_status);
                // Remove `Connection` as per
                // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
                for (header_name, header_value) in
                    res_headers.iter().filter(|(h, _)| *h != "connection")
                {
                    client_resp
                        .headers_mut()
                        .unwrap()
                        .insert(header_name.clone(), header_value.clone());
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

                Ok(client_resp.body(full(res_body)).unwrap())
            }
        } else {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(full("No session started"))
                .unwrap())
        }
    } else {
        if Method::CONNECT == req.method() {
            if let Some(addr) = host_addr(req.uri()) {
                tokio::task::spawn(async move {
                    match hyper::upgrade::on(req).await {
                        Ok(upgraded) => {
                            if let Err(e) = tunnel(upgraded, addr).await {
                                warn!("server io error: {}", e);
                            };
                        }
                        Err(e) => warn!("upgrade error: {}", e),
                    }
                });

                Ok(Response::new(empty()))
            } else {
                warn!("CONNECT host is not socket addr: {:?}", req.uri());
                let mut resp = Response::new(full("CONNECT must be to a socket address"));
                *resp.status_mut() = http::StatusCode::BAD_REQUEST;

                Ok(resp)
            }
        } else {
            let host = req.uri().host().expect("uri has no host");
            let port = req.uri().port_u16().unwrap_or(80);

            let stream = TcpStream::connect((host, port)).await.unwrap();
            let io = TokioIo::new(stream);

            let (mut sender, conn) = Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await?;
            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    warn!("Connection failed: {:?}", err);
                }
            });

            let resp = sender.send_request(req).await?;
            Ok(resp.map(|b| b.boxed()))
        }
    }
}

// Start a record session
async fn start_record_handler(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<BoxBody>> {
    let mut sessions_lock = state.sessions.lock().await;
    let record_name = req.uri().query().unwrap();
    let session_id = Uuid::new_v4();
    let record_session = RecordSession {
        filepath: format!(
            "{}/{}.snap",
            state.record_dir.trim_end_matches('/'),
            record_name
        ),
        states: HashMap::new(),
        records: HashMap::new(),
    };
    sessions_lock.insert(session_id.to_string(), record_session);
    let mut res = Response::default();
    let cookie = Cookie::build(("r-session", session_id.to_string()))
        .http_only(true)
        .build();
    res.headers_mut()
        .append(SET_COOKIE, cookie.to_string().parse().unwrap());
    *res.status_mut() = StatusCode::OK;
    Ok(res)
}

// End a record session
async fn end_record_handler(req: Request<Incoming>, state: AppState) -> Result<Response<BoxBody>> {
    if let Some(session_id) = get_session(&req) {
        let was_recording = !state.record_dir.is_empty() && state.need_recording;

        if was_recording {
            let mut sessions_lock = state.sessions.lock().await;
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
            fs::create_dir_all(&state.record_dir).expect("Cannot create dir");
            debug!("Writing to {}", filepath);
            fs::write(filepath, data).expect("Cannot write to file");

            sessions_lock.remove(&session_id);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(full("Record saved"))
                .unwrap())
        } else {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(full("Not recording"))
                .unwrap())
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(full("No session was started"))
            .unwrap())
    }
}

// Clear all sessions
async fn clear_sessions(state: AppState) -> Result<Response<BoxBody>> {
    let mut sessions_lock = state.sessions.lock().await;
    sessions_lock.clear();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(full("Sessions cleared"))
        .unwrap())
}

async fn handle_request(req: Request<Incoming>, state: AppState) -> Result<Response<BoxBody>> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/start_record") => start_record_handler(req, state).await,
        (&Method::POST, "/end_record") => end_record_handler(req, state).await,
        (&Method::POST, "/clear-sessions") => clear_sessions(state).await,
        _ => proxy(req, state).await,
    }
}

fn host_addr(uri: &http::Uri) -> Option<String> {
    uri.authority().and_then(|auth| Some(auth.to_string()))
}

fn empty() -> BoxBody {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn get_session(req: &Request<hyper::body::Incoming>) -> Option<String> {
    let cookie_header = req.headers().get("cookie").expect("No cookie header found");
    let mut cookies =
        Cookie::split_parse_encoded(cookie_header.to_str().expect("Cannot parse Cookie header"))
            .map(|c| c.unwrap());

    return cookies
        .find(|c| c.name() == "r-session")
        .map(|c| c.value().to_string());
}

// Create a TCP connection to host:port, build a tunnel between the connection and the upgraded connection
async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr.clone()).await?;
    let mut upgraded = TokioIo::new(upgraded);

    // Proxying data
    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    debug!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );
    Ok(())
}
