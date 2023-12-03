use crate::helpers::create_app;
use actix_web::test;
use http_replay_proxy::cli::CliArguments;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    user_id: i128,
    id: i128,
    title: String,
    completed: bool,
}

#[actix_web::test]
async fn test_replay_mode_start() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "./tests/data".to_string(),
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
async fn test_replaying_mode_start_then_end() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "./tests/data/".to_string(),
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
}

#[actix_web::test]
async fn test_replaying_mode_start_then_a_request_then_end() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "./tests/data/".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let mut req = test::TestRequest::post()
        .uri("/start-record/test")
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
    let res2: User = test::call_and_read_body_json(&app, req).await;
    assert_eq!(res2.id, 1);
    req = test::TestRequest::post()
        .cookie(cookie)
        .uri("/end-record")
        .to_request();
    res = test::call_service(&app, req).await;
    assert!(res.status().is_success());

}

#[actix_web::test]
async fn test_replaying_mode_start_then_multiple_requests_then_end() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "./tests/data/".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let mut req = test::TestRequest::post()
        .uri("/start-record/test")
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
}
