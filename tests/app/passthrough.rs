use crate::helpers::create_app;
use actix_web::test;
use http_replay_proxy::cli::CliArguments;
use log::info;

#[actix_web::test]
async fn test_passthrough_mode() {

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
    let args = CliArguments {
        forward_to: "https://jsonplaceholder.typicode.com/".to_string(),
        record: false,
        record_dir: "".to_string(),
        listen_addr: "localhost".to_string(),
        port: 3333,
    };

    let app = test::init_service(create_app(args).await).await;
    let req = test::TestRequest::get().uri("/todos/1").to_request();
    info!("{}", req.uri());
    let resp = test::call_service(&app, req).await;

    info!("{}", resp.status());

    assert!(resp.status().is_success());
}
