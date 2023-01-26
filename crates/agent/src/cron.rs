pub use cron::Schedule;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

type DynFuture = dyn Future<Output = ()> + Send;
type ResFuture = Pin<Box<DynFuture>>;
type DynFnRetFuture = dyn FnMut() -> ResFuture + Send + Sync;
type AsyncJobLocked = Box<DynFnRetFuture>;

pub struct ScheduledJob {
    schedule: Schedule,
    job: AsyncJobLocked,
    job_id: Uuid,
    last_run: chrono::DateTime<chrono::Local>,
}

impl ScheduledJob {
    pub fn from<F>(schedule: Schedule, f: F) -> Self
    where
        F: 'static,
        F: FnMut() -> ResFuture + Send + Sync,
    {
        Self {
            schedule,
            job: Box::new(f),
            job_id: Uuid::new_v4(),
            last_run: chrono::Local::now(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.job_id
    }

    pub async fn tick(&mut self) {
        let now = chrono::Local::now();
        for time in self.schedule.after(&self.last_run) {
            if time > now {
                break;
            }

            let future = (self.job)();
            tokio::spawn(async move {
                future.await;
            });

            self.last_run = time;
        }
    }
}

pub type CronSchedulerLocked = Arc<Mutex<CronScheduler>>;

pub struct CronScheduler {
    pub jobs: Vec<ScheduledJob>,
}

impl CronScheduler {
    pub fn new() -> CronScheduler {
        CronScheduler { jobs: vec![] }
    }

    pub fn add(&mut self, job: ScheduledJob) {
        self.jobs.push(job);
    }

    pub fn remove(&mut self, uuid: Uuid) {
        self.jobs.retain(|j| j.job_id != uuid);
    }

    pub async fn tick(&mut self) {
        for job in &mut self.jobs {
            job.tick().await;
        }
    }
}
