use crate::cron::{CronScheduler, ScheduledJob};
use crate::task::TaskExecContextLocked;
use async_trait::async_trait;
use protocol::{TaskSpecError, TaskError};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub type Trigger = Box<dyn TriggerTrait + Send + Sync>;

#[async_trait]
pub trait TriggerTrait {
    async fn install(&mut self, ctx: TaskExecContextLocked) -> Result<(), TaskError>;
    async fn uninstall(&mut self);
}

pub struct CronTrigger {
    crond: Arc<Mutex<CronScheduler>>,
    cron_expr: String,
    job_id: Option<Uuid>,
}

impl CronTrigger {
    pub async fn new(crond: Arc<Mutex<CronScheduler>>, cron_expr: String) -> Self {
        Self {
            crond,
            cron_expr,
            job_id: None,
        }
    }
}

#[async_trait]
impl TriggerTrait for CronTrigger {
    async fn install(&mut self, ctx: TaskExecContextLocked) -> Result<(), TaskError> {
        let sched = match self.cron_expr.parse() {
            Ok(v) => v,
            Err(_) => return Err(TaskSpecError::InvalidCronExpresion.into()),
        };

        let sched_job = ScheduledJob::from(sched, move || {
            let ctx = ctx.clone();
            Box::pin(async move {
                ctx.lock().await.run().await;
            })
        });
        self.job_id = Some(sched_job.id());
        self.crond.lock().await.add(sched_job);
        Ok(())
    }

    async fn uninstall(&mut self) {
        if let Some(uuid) = self.job_id.take() {
            self.crond.lock().await.remove(uuid);
        }
    }
}

pub struct ImmediateTrigger {}

impl ImmediateTrigger {
    pub fn new() -> ImmediateTrigger {
        ImmediateTrigger {}
    }
}

#[async_trait]
impl TriggerTrait for ImmediateTrigger {
    async fn install(&mut self, ctx: TaskExecContextLocked) -> Result<(), TaskError> {
        tokio::spawn(async move {
            ctx.lock().await.run().await;
        });
        Ok(())
    }

    async fn uninstall(&mut self) {}
}

pub struct StartupTrigger {}

impl StartupTrigger {
    pub fn new() -> StartupTrigger {
        StartupTrigger {}
    }
}

#[async_trait]
impl TriggerTrait for StartupTrigger {
    async fn install(&mut self, ctx: TaskExecContextLocked) -> Result<(), TaskError> {
        tokio::spawn(async move {
            ctx.lock().await.run().await;
        });
        Ok(())
    }

    async fn uninstall(&mut self) {}
}
