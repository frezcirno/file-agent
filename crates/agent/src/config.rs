use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::{error::Error, fs, io, path::Path};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent_id: Uuid,
    pub server: String,
    pub key: String,
    pub pull: bool,
    pub pull_interval: u64,
    pub report: bool,
    pub report_interval: u64,
}

pub fn load<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<T, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let u = serde_json::from_reader(reader)?;
    Ok(u)
}

pub fn dump<P: AsRef<Path>>(cfg: &impl Serialize, path: P) -> Result<(), Box<dyn Error>> {
    let file = fs::File::create(path)?;
    let writer = io::BufWriter::new(file);
    let u = serde_json::to_writer(writer, cfg)?;
    Ok(u)
}
