mod async_job;
mod config;
mod cron;
mod manager;
mod task;
mod trigger;
use bytes::BytesMut;
use clap::Parser;
use config::Config;
use log::LevelFilter;
use manager::TaskManager;
use protocol::{make_key, DecodeError, Request, Response, TaskSpec};
use serde::{de::DeserializeOwned, Serialize};
use std::io::ErrorKind;
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
    sync::Mutex,
};
use uuid::Uuid;

struct Agent {
    server: String,
    agent_id: Uuid,
    aes_key: [u8; 32],

    pull: bool,
    pull_interval: u64,

    task_file: PathBuf,

    report: bool,
    report_interval: u64,

    tm: Arc<Mutex<TaskManager>>,

    last_report: chrono::DateTime<chrono::Local>,
}

impl Agent {
    async fn new(config: Config, task_file: PathBuf) -> Self {
        Self {
            server: config.server,
            agent_id: config.agent_id,
            aes_key: make_key(&config.key),

            task_file,
            tm: Arc::new(Mutex::new(TaskManager::new().await)),

            pull: config.pull,
            pull_interval: config.pull_interval,

            report: config.report,
            report_interval: config.report_interval,
            last_report: chrono::Local::now(),
        }
    }

    async fn start(self: &Arc<Self>) {
        if let Ok(specs) = config::load(self.task_file.as_path()) {
            let mut tm = self.tm.lock().await;
            if let Err(e) = tm.reload(specs).await {
                log::error!("load tasks failed: {:?}", e);
            }
        }

        self.tm.lock().await.start_tick().await;

        if self.pull {
            log::info!("start pull loop");
            let me = self.clone();
            tokio::spawn(async move {
                me.pull_loop().await;
            });
        }

        if self.report {
            log::info!("start report loop");
            let me = self.clone();
            tokio::spawn(async move {
                me.report_loop().await;
            });
        }

        // wait forever
        tokio::signal::ctrl_c().await.unwrap();
    }

    async fn pull_loop(self: &Arc<Self>) {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(self.pull_interval));
        loop {
            if let Err(e) = self.pull().await {
                log::error!("Pull failed: {}", e);
            }
            interval.tick().await;
        }
    }

    async fn report_loop(self: &Arc<Self>) {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(self.report_interval));
        loop {
            interval.tick().await;
            if let Err(e) = self.report().await {
                log::error!("Report failed: {}", e);
            }
        }
    }

    fn encode(&self, t: impl Serialize, buf: &mut BytesMut) -> bool {
        protocol::encode(t, buf, &self.aes_key)
    }

    fn decode<T>(&self, buf: &mut BytesMut) -> Result<T, DecodeError>
    where
        T: DeserializeOwned,
    {
        protocol::decode(buf, &self.aes_key)
    }

    async fn pull(self: &Arc<Self>) -> io::Result<()> {
        let stream = TcpStream::connect(&self.server).await?;
        log::debug!("Pull tasks from: {}", self.server);

        stream.set_nodelay(true).expect("Failed to set nodelay");

        let mut wfile = BufWriter::new(stream);
        let mut buf = BytesMut::new();

        let req = Request::PullTask {
            id: self.agent_id.clone(),
        };
        if !self.encode(req, &mut buf) {
            return Err(ErrorKind::InvalidData.into());
        }
        wfile.write_all(&buf).await?;
        wfile.flush().await?;

        buf.clear();
        'resp: loop {
            let n = wfile.read_buf(&mut buf).await?;
            if n == 0 {
                log::error!("pull connection reset without response");
                return Err(ErrorKind::ConnectionReset.into());
            }

            while buf.len() > protocol::HEADER_LEN {
                let msg: Response = match self.decode(&mut buf) {
                    Ok(msg) => msg,
                    Err(DecodeError::NotEnoughData) => {
                        break;
                    }
                    Err(DecodeError::InvalidData) => {
                        log::error!("msg decode failed: invalid data");
                        return Err(ErrorKind::InvalidData.into());
                    }
                };

                match msg {
                    Response::Object(_) => {},
                    Response::Error(msg) => {
                        log::error!("server error: {}", msg);
                        return Err(ErrorKind::InvalidData.into());
                    }
                    _ => {
                        log::error!("unexpected response: {:?}", msg);
                        return Err(ErrorKind::InvalidData.into());
                    }
                };

                let msg = match msg.into() {
                    Ok(msg) => msg,
                    Err(e) => {
                        log::error!("msg decode failed: {:?}", e);
                        return Err(ErrorKind::InvalidData.into());
                    }
                };

                config::dump(&msg, "tasks.json").expect("save task failed");

                if let Err(e) = self.tm.lock().await.reload(msg).await {
                    log::error!("reload failed, error: {:?}", e);
                }

                break 'resp;
            }
        }

        Ok(())
    }

    async fn report(self: &Arc<Self>) -> io::Result<()> {
        let stream = TcpStream::connect(&self.server).await?;

        stream.set_nodelay(true).expect("Failed to set nodelay");

        let mut wfile = BufWriter::new(stream);
        let mut buf = BytesMut::new();

        let req = Request::ReportStatus {
            id: self.agent_id.clone(),
            log: self.tm.lock().await.export_log().await,
        };
        if !self.encode(req, &mut buf) {
            return Err(ErrorKind::InvalidData.into());
        }
        wfile.write_all(&buf).await?;
        wfile.flush().await?;

        buf.clear();
        'resp: loop {
            let n = wfile.read_buf(&mut buf).await?;
            if n == 0 {
                log::error!("report connection reset without response");
                return Err(ErrorKind::ConnectionReset.into());
            }

            while buf.len() > protocol::HEADER_LEN {
                let msg: Response = match self.decode(&mut buf) {
                    Ok(msg) => msg,
                    Err(DecodeError::NotEnoughData) => {
                        break;
                    }
                    Err(DecodeError::InvalidData) => {
                        log::error!("msg decode failed: invalid data");
                        return Err(ErrorKind::InvalidData.into());
                    }
                };

                let Response::Ok = msg else {
                    log::error!("Report failed");
                    return Err(ErrorKind::InvalidData.into());
                };

                break 'resp;
            }
        }

        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(name = "agent")]
#[command(author = "frezcirno")]
#[command(version = "1.0")]
#[command(about = "A Centralized Cron-like Task Manager - Agent End", long_about = None)]
struct Args {
    /// Control server config file
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,

    /// Tasks file
    #[arg(short, long, value_name = "FILE")]
    task_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let args = Args::parse();

    let config = config::load(&args.config).expect("load config failed");

    let task_file = args
        .task_file
        .unwrap_or(dirs::data_dir().unwrap().join("agent").join("tasks.json"));

    Arc::new(Agent::new(config, task_file).await).start().await;
}
