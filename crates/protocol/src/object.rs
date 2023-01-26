use crate::TaskSpec;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::io;
use std::time::SystemTime;
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    AddTask { id: Uuid, spec: TaskSpec },
    RemoveTask { id: Uuid },
    ListTask,
    Reload,
    PullTask { id: Uuid },
    ReportStatus { id: Uuid, log: AgentEventLog },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Error(String),
    Object(Vec<u8>),
}

impl Response {
    pub fn ok() -> Self {
        Response::Ok
    }

    pub fn err(err: String) -> Self {
        Response::Error(err)
    }

    pub fn object<T>(obj: &T) -> Self
    where
        T: Serialize,
    {
        let bytes = bincode::serialize(obj).expect("serialize object");
        Response::Object(bytes)
    }

    pub fn into<T>(self) -> Result<T, bincode::Error>
    where
        T: DeserializeOwned,
    {
        match self {
            Response::Object(bytes) => bincode::deserialize(&bytes),
            _ => Err(bincode::ErrorKind::DeserializeAnyNotSupported.into()),
        }
    }
}
