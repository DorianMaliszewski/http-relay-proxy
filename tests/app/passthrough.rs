use crate::helpers::create_app;
use actix_web::test;
use http_replay_proxy::cli::CliArguments;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    user_id: i128,
    id: i128,
    title: String,
    completed: bool
}

#[actix_web::test]
async fn test_passthrough_mode_status() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let req = test::TestRequest::get().uri("/todos/1").to_request();
    let res = test::call_service(&app, req).await;
    assert!(res.status().is_success());

}

#[actix_web::test]
async fn test_passthrough_mode_body() {
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let req = test::TestRequest::get().uri("/todos/1").to_request();
    let res: User = test::call_and_read_body_json(&app, req).await;

    assert_eq!(res.id, 1);

}
