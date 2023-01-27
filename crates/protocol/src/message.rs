use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, fmt::Display, io, path::PathBuf, time::SystemTime};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskSpecError {
    InvalidCronExpresion,
}

impl Display for TaskSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskSpecError::InvalidCronExpresion => write!(f, "invalid cron expression"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub status: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskError {
    TaskNotFound,
    TaskSpecError(TaskSpecError),
    UnsupportedPlatform(String),
    IoError(String),
    NetError(String),
    RuntimeError(String),
}

impl Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::TaskNotFound => write!(f, "task not found"),
            TaskError::TaskSpecError(e) => write!(f, "task spec error: {}", e),
            TaskError::UnsupportedPlatform(e) => write!(f, "unsupported platform: {}", e),
            TaskError::IoError(e) => write!(f, "io error: {}", e),
            TaskError::NetError(e) => write!(f, "net error: {}", e),
            TaskError::RuntimeError(e) => write!(f, "runtime error: {}", e),
        }
    }
}

impl From<TaskSpecError> for TaskError {
    fn from(e: TaskSpecError) -> Self {
        TaskError::TaskSpecError(e)
    }
}

impl From<io::Error> for TaskError {
    fn from(e: io::Error) -> Self {
        TaskError::IoError(e.to_string())
    }
}

impl From<Box<dyn Error + Send + Sync>> for TaskError {
    fn from(e: Box<dyn Error + Send + Sync>) -> Self {
        TaskError::RuntimeError(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    TriggerInstall,
    Deactivate,
    Run,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub type_: EventType,
    pub start: SystemTime,
    pub end: SystemTime,
    pub result: Result<TaskResult, TaskError>,
}

pub type AgentEventLog = HashMap<Uuid, Vec<Event>>;
