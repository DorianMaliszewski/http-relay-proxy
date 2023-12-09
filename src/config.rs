use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub hosts_to_record: Vec<String>,
    pub listen_addr: String,
    pub listen_port: u16,
    pub record_dir: String,
}

impl Config {
}
