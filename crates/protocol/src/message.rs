use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Action {
    Retry { times: u8, interval: u64 },
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TriggerSpec {
    Cron(String),
    Immediate,
    Startup,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileSpec {
    pub path: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub cmd: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub shell: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostSpec {
    pub ip: String,
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskType {
    FileUpdate(FileSpec),
    Command(CommandSpec),
    Hosts(HostSpec),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSpec {
    pub name: String,
    pub task: TaskType,
    pub on_error: Action,
    pub triggers: Vec<TriggerSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub pull: bool,
    pub server: String,
    pub pull_interval: u64,
    pub aes_key: String,
}
