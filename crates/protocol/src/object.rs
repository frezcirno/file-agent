use crate::{AgentEventLog, TaskSpec};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

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
