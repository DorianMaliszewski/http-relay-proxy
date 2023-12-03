use std::{collections::HashMap, fs};

use crate::helpers::create_app;
use actix_web::test;
use http_replay_proxy::{cli::CliArguments, records::Record};

#[actix_web::test]
async fn test_recording_mode_start() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: true,
        record_dir: ".tmp/".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let req = test::TestRequest::post()
        .uri("/start-record/test")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    assert!(res.headers().contains_key("set-cookie"));
    assert_ne!(
        res.response().cookies().find(|c| c.name() == "r-session"),
        None
    );
}

#[actix_web::test]
async fn test_recording_mode_start_then_end() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: true,
        record_dir: "./.tmp/".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let mut req = test::TestRequest::post()
        .uri("/start-record/test")
        .to_request();
    let mut res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    let cookie = res
        .response()
        .cookies()
        .find(|c| c.name() == "r-session")
        .unwrap();
    req = test::TestRequest::post()
        .cookie(cookie)
        .uri("/end-record")
        .to_request();
    res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    assert!(fs::metadata("./.tmp/test.snap").is_ok());
    assert!(fs::remove_file("./.tmp/test.snap").is_ok());
}

#[actix_web::test]
async fn test_recording_mode_start_then_a_request_then_end() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: true,
        record_dir: "./.tmp/".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let mut req = test::TestRequest::post()
        .uri("/start-record/test2")
        .to_request();
    let mut res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    assert_ne!(
        res.response()
            .cookies()
            .find(|cookie| cookie.name() == "r-session"),
        None
    );
    let cookie = res
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "r-session")
        .unwrap();
    req = test::TestRequest::get()
        .uri("/todos/1")
        .cookie(cookie.clone())
        .to_request();
    let res2 = test::call_service(&app, req).await;
    assert!(res2.status().is_success());
    req = test::TestRequest::post()
        .cookie(cookie)
        .uri("/end-record")
        .to_request();
    res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    assert!(fs::metadata("./.tmp/test2.snap").is_ok());
    let content: HashMap<String, Vec<Record>> = serde_json::from_str(
        fs::read_to_string("./.tmp/test2.snap")
            .unwrap()
            .as_str(),
    )
    .unwrap();
    assert!(content.contains_key("GET:/todos/1"));
    assert!(fs::remove_file("./.tmp/test2.snap").is_ok());
}

#[actix_web::test]
async fn test_recording_mode_start_then_multiple_requests_then_end() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: true,
        record_dir: "./.tmp/".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let mut req = test::TestRequest::post()
        .uri("/start-record/test3")
        .to_request();
    let mut res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    assert_ne!(
        res.response()
            .cookies()
            .find(|cookie| cookie.name() == "r-session"),
        None
    );
    let cookie = res
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "r-session")
        .unwrap();
    req = test::TestRequest::get()
        .uri("/todos/1")
        .cookie(cookie.clone())
        .to_request();
    let mut res2 = test::call_service(&app, req).await;
    assert!(res2.status().is_success());
    req = test::TestRequest::get()
        .cookie(cookie.clone())
        .uri("/todos/1")
        .to_request();
    res2 = test::call_service(&app, req).await;
    assert!(res2.status().is_success());
    req = test::TestRequest::get()
        .cookie(cookie.clone())
        .uri("/todos/2")
        .to_request();
    res2 = test::call_service(&app, req).await;
    assert!(res2.status().is_success());
    req = test::TestRequest::post()
        .cookie(cookie)
        .uri("/end-record")
        .to_request();
    res = test::call_service(&app, req).await;
    assert!(res.status().is_success());
    assert!(fs::metadata("./.tmp/test3.snap").is_ok());
    let content: HashMap<String, Vec<Record>> = serde_json::from_str(
        fs::read_to_string("./.tmp/test3.snap")
            .unwrap()
            .as_str(),
    )
    .unwrap();
    assert!(content.contains_key("GET:/todos/1"));
    assert!(content.contains_key("GET:/todos/2"));
    assert_eq!(content.get("GET:/todos/1").unwrap().len(), 2);
}
