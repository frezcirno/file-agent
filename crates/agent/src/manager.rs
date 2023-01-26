use crate::cron::CronScheduler;
use crate::task::Task;
use protocol::{Event, TaskError, TaskSpec};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct TaskManager {
    cron: Arc<Mutex<CronScheduler>>,
    crond: Option<tokio::task::JoinHandle<()>>,
    tasks: HashMap<Uuid, Task>,
}

impl TaskManager {
    pub async fn new() -> Self {
        Self {
            cron: Arc::new(Mutex::new(CronScheduler::new())),
            tasks: HashMap::new(),
            crond: None,
        }
    }

    pub async fn export_log(&mut self) -> HashMap<Uuid, Vec<Event>> {
        let mut res = HashMap::new();
        for (id, t) in self.tasks.iter_mut() {
            res.insert(id.clone(), t.export_log().await);
        }
        res
    }

    pub async fn start_tick(&mut self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let crond = self.cron.clone();
        let handle = tokio::spawn(async move {
            loop {
                interval.tick().await;
                crond.lock().await.tick().await;
            }
        });
        self.crond.replace(handle);
    }

    pub async fn stop_tick(&mut self) {
        if let Some(handle) = self.crond.take() {
            handle.abort();
            handle.await.unwrap_err();
        }
    }

    pub async fn reload(&mut self, specs: HashMap<Uuid, TaskSpec>) -> Result<(), TaskError> {
        log::info!("Task manager reloading tasks...");

        // remove tasks that are not in specs
        let mut to_remove = Vec::new();
        for (id, _) in self.tasks.iter() {
            if !specs.contains_key(id) {
                to_remove.push(id.clone());
            }
        }
        for id in to_remove {
            self.tasks.remove(&id).unwrap().deactivate().await;
        }

        // update or add tasks
        for (id, task_spec) in specs {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.update(task_spec).await;
            } else {
                self.add_task(id, task_spec).await;
            }
        }

        Ok(())
    }

    pub async fn add_task(&mut self, id: Uuid, task_spec: TaskSpec) {
        let mut task = Task::new(task_spec, self.cron.clone()).await;
        task.try_activate().await;
        self.tasks.insert(id, task);
    }
}
