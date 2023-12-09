use clap::Parser;
use cli::*;
use config::Config;

mod app;
mod cli;
mod config;
mod records;
mod tokiort;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let mut config = get_default_config();

    if let f = std::fs::File::open("config.yml") {
        config = serde_yaml::from_reader(f).expect("Error when parsing config");
    };

    let args = CliArguments::parse();

    return app::launch_app(config, args.record).await;
}

pub fn get_default_config() -> Config {
    Config {
        hosts_to_record: [".*"],
        listen_addr: "0.0.0.0".to_string(),
        listen_port: 3333,
        record_dir: "./.tmp".to_string(),
    }
}
