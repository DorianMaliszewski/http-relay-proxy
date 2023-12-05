use clap::Parser;
use cli::*;

mod app;
mod cli;
mod records;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = CliArguments::parse();

    return app::launch_app(args).await;
}
