use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};

#[derive(Debug, Clone)]
pub struct RecordOptions {
    pub record: bool,
    pub record_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RecordSession {
    pub states: HashMap<String, usize>,
    pub filepath: String,
    pub records: HashMap<String, Vec<Record>>,
}

pub struct SessionState {
    pub sessions: Mutex<HashMap<String, RecordSession>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Record {
    pub status: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Serialize, Deserialize)]
pub struct RecordFile {
    pub records: HashMap<String, Vec<Record>>,
}
