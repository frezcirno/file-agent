use crate::async_job::{AsyncTask, CommandTask, FileUpdateTask, HostTask};
use crate::cron::CronSchedulerLocked;
use crate::trigger::{CronTrigger, ImmediateTrigger, StartupTrigger, Trigger};
use protocol::{Event, TaskError, TaskResult, TaskSpec, TaskType, TriggerSpec};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct TaskExecContext {
    pub task: AsyncTask,
    pub run_history: VecDeque<Event>,
}

impl TaskExecContext {
    pub async fn run(&mut self) {
        let start = SystemTime::now();
        let result = self.task.run().await;
        let end = SystemTime::now();
        self.run_history.push_back(Event {
            id: Uuid::new_v4(),
            type_: protocol::EventType::Run,
            start,
            end,
            result,
        });
    }

    pub fn export_log(&mut self) -> Vec<Event> {
        let mut res = vec![];
        while let Some(run) = self.run_history.pop_front() {
            res.push(run);
        }
        res
    }
}

pub type TaskExecContextLocked = Arc<Mutex<TaskExecContext>>;

pub enum TaskState {
    Activated,
    Deactivated,
}

pub struct Task {
    spec: TaskSpec,
    context: TaskExecContextLocked,
    triggers: Vec<Trigger>,
    sched: CronSchedulerLocked,
    state: TaskState,
}

impl Task {
    pub async fn new(spec: TaskSpec, sched: CronSchedulerLocked) -> Self {
        let mut triggers: Vec<Trigger> = vec![];
        for trig in &spec.triggers {
            let trigger = Self::make_trigger(&sched, trig).await;
            triggers.push(trigger);
        }

        Self {
            context: Arc::new(Mutex::new(TaskExecContext {
                task: Self::make_task(&spec.task).await,
                run_history: VecDeque::new(),
            })),
            triggers,
            sched,
            spec,
            state: TaskState::Deactivated,
        }
    }

    pub async fn export_log(&mut self) -> Vec<Event> {
        let mut ctx = self.context.lock().await;
        ctx.export_log()
    }

    pub fn is_activated(&self) -> bool {
        match self.state {
            TaskState::Activated => true,
            TaskState::Deactivated => false,
        }
    }

    pub async fn update(&mut self, spec: TaskSpec) {
        // if the task type has changed, we need to recreate the task
        if self.spec.task != spec.task {
            let task = Self::make_task(&spec.task).await;

            self.deactivate().await;
            self.context.lock().await.task = task;
        }

        // if the triggers have changed, we need to recreate the triggers
        if self.spec.triggers != spec.triggers {
            // diff the triggers
            let mut new_triggers: Vec<Trigger> = vec![];
            for trig in &spec.triggers {
                let trigger = Self::make_trigger(&self.sched, trig).await;
                new_triggers.push(trigger);
            }

            self.deactivate().await;
            self.triggers = new_triggers;
        }

        // update the spec
        self.spec = spec;

        // try to activate the task
        self.try_activate().await;
    }

    async fn make_task(task: &TaskType) -> AsyncTask {
        match task {
            TaskType::FileUpdate(spec) => Box::new(FileUpdateTask {
                file_spec: spec.clone(),
            }),
            TaskType::Command(spec) => Box::new(CommandTask {
                command_spec: spec.clone(),
            }),
            TaskType::Hosts(spec) => Box::new(HostTask {
                host_spec: spec.clone(),
            }),
        }
    }

    async fn make_trigger(sched: &CronSchedulerLocked, trigger: &TriggerSpec) -> Trigger {
        match trigger {
            TriggerSpec::Cron(expr) => {
                Box::new(CronTrigger::new(sched.clone(), expr.to_string()).await)
            }
            TriggerSpec::Immediate => Box::new(ImmediateTrigger::new()),
            TriggerSpec::Startup => Box::new(StartupTrigger::new()),
        }
    }

    pub async fn activate(&mut self) -> Result<(), TaskError> {
        if self.is_activated() {
            return Ok(());
        }
        for trigger in &mut self.triggers {
            let start = SystemTime::now();
            let result = trigger.install(self.context.clone()).await;

            let event = Event {
                id: Uuid::new_v4(),
                type_: protocol::EventType::TriggerInstall,
                start,
                end: SystemTime::now(),
                result: result.clone().map(|_| TaskResult {
                    status: Some(0),
                    message: "".to_string(),
                }),
            };
            self.context.lock().await.run_history.push_back(event);

            if let Err(_) = result {
                return result;
            }
        }
        self.state = TaskState::Activated;
        Ok(())
    }

    pub async fn deactivate(&mut self) {
        if !self.is_activated() {
            return;
        }
        for trigger in &mut self.triggers {
            let start = SystemTime::now();
            trigger.uninstall().await;
            let event = Event {
                id: Uuid::new_v4(),
                type_: protocol::EventType::TriggerInstall,
                start,
                end: SystemTime::now(),
                result: Ok(TaskResult {
                    status: Some(0),
                    message: "".to_string(),
                }),
            };
            self.context.lock().await.run_history.push_back(event);
        }
        self.state = TaskState::Deactivated;
    }

    pub async fn try_activate(&mut self) {
        if let Err(err) = self.activate().await {
            log::warn!("Failed to activate task: {}", err);
        }
    }
}
