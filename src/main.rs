use std::fs;

use clap::Parser;
use cli::*;
use config::Config;

mod app;
mod cli;
mod config;
mod records;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
    let f = fs::read_to_string("config.yml").expect("Cannot read file");
    let config = serde_yaml::from_str(f.as_str()).unwrap_or(get_default_config());

    let args = CliArguments::parse();

    return app::launch_app(config, args.record).await;
}

pub fn get_default_config() -> Config {
    Config {
        hosts_to_record: [".*".to_string()].to_vec(),
        listen_addr: "0.0.0.0".to_string(),
        listen_port: 3333,
        record_dir: "".to_string(),
    }
}
