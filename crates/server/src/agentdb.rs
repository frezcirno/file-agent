use crate::config::load;
use protocol::TaskSpec;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::{collections::HashMap, fs::File, io::Write, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub server: String,
    pub key: String,
    pub pull: bool,
    pub pull_interval: u64,
    pub report: bool,
    pub report_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentData {
    #[serde(flatten)]
    pub config: Agent,
    pub tasks: HashMap<Uuid, TaskSpec>,
}

pub struct AgentDb {
    file: PathBuf,
    agent: HashMap<Uuid, AgentData>,
}

impl AgentDb {
    pub fn new(file: impl AsRef<Path>) -> Self {
        Self {
            file: file.as_ref().to_path_buf(),
            agent: match load(&file) {
                Ok(v) => v,
                Err(_) => HashMap::new(),
            },
        }
    }

    pub fn list_agents(&self) -> Vec<Uuid> {
        self.agent.keys().cloned().collect()
    }

    pub fn insert_config(&mut self, v: Agent) -> Uuid {
        let uuid = Uuid::new_v4();
        let agent = AgentData {
            config: v,
            tasks: HashMap::new(),
        };
        self.agent.insert(uuid, agent);
        self.sync();
        uuid
    }
    pub fn update_config(&mut self, k: &Uuid, v: Agent) -> Option<()> {
        let agent = self.agent.get_mut(k)?;
        agent.config = v;
        self.sync();
        Some(())
    }

    pub fn remove(&mut self, k: &Uuid) -> Option<AgentData> {
        let res = self.agent.remove(k)?;
        self.sync();
        Some(res)
    }

    pub fn get_agent(&self, k: &Uuid) -> Option<&AgentData> {
        self.agent.get(k)
    }

    pub fn insert_agent_task(&mut self, k: &Uuid, v: TaskSpec) -> Option<Uuid> {
        let agent = self.agent.get_mut(k)?;
        let id = Uuid::new_v4();
        agent.tasks.insert(id, v);
        self.sync();
        Some(id)
    }

    pub fn update_agent_task(&mut self, ak: &Uuid, tk: &Uuid, v: TaskSpec) -> Option<()> {
        let agent = self.agent.get_mut(ak)?;
        if !agent.tasks.contains_key(tk) {
            return None;
        }

        let task = agent.tasks.get_mut(tk).unwrap();
        *task = v;
        self.sync();
        Some(())
    }

    pub fn remove_agent_task(&mut self, ak: &Uuid, tk: &Uuid) -> Option<TaskSpec> {
        let agent = self.agent.get_mut(ak)?;
        let task = agent.tasks.remove(tk)?;
        self.sync();
        Some(task)
    }

    fn sync(&self) {
        let mut f = File::create(&self.file).unwrap();
        serde_json::to_writer_pretty(&mut f, &self.agent).unwrap();
        f.flush().unwrap();
    }
}
