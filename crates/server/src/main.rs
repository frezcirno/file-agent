mod agentdb;
mod config;
mod webapi;
use agentdb::AgentDb;
use bytes::BytesMut;
use clap::Parser;
use config::Config;
use log::LevelFilter;
use protocol::{make_key, DecodeError, Key, Request, Response};
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
    sync::RwLock,
};
use uuid::Uuid;
use webapi::WebApi;

pub struct Server {
    ctl_addr: String,
    api_addr: String,
    aes_key: Key,
    agents: RwLock<AgentDb>,
    logs_dir: PathBuf,
}

impl Server {
    async fn new(config: Config, agentdb_path: impl AsRef<Path>, logs_dir: PathBuf) -> Self {
        Self {
            ctl_addr: config.ctl_addr,
            api_addr: config.api_addr,
            aes_key: make_key(&config.key),
            agents: RwLock::new(AgentDb::new(agentdb_path)),
            logs_dir,
        }
    }

    async fn start(self: &Arc<Self>) {
        let me = self.clone();
        tokio::spawn(async move {
            me.start_ctl().await;
        });

        let me = self.clone();
        tokio::spawn(async move {
            WebApi::start_api(&me).await;
        });

        // wait for shutdown
        tokio::signal::ctrl_c().await.unwrap();
    }

    async fn start_ctl(self: &Arc<Self>) {
        let listener = tokio::net::TcpListener::bind(self.ctl_addr.as_str())
            .await
            .expect("Failed to bind");
        log::info!("Controller listening on {}", self.ctl_addr);

        while let Ok((stream, client)) = listener.accept().await {
            log::info!("New agent connection from: {}", client);

            stream.set_nodelay(true).expect("Failed to set nodelay");

            let me = self.clone();
            tokio::spawn(async move {
                let _ = me.handle_agent(stream, client).await;
            });
        }
    }

    async fn handle_agent(
        self: &Arc<Self>,
        stream: TcpStream,
        client: SocketAddr,
    ) -> io::Result<()> {
        let mut wfile = BufWriter::new(stream);
        let mut buf = BytesMut::new();
        buf.reserve(1024);

        loop {
            let n = wfile.read_buf(&mut buf).await?;
            if n == 0 {
                log::info!("Agent connection closed: {}", client);
                return Ok(());
            }

            while buf.len() > protocol::HEADER_LEN {
                let req = protocol::decode(&mut buf, &self.aes_key);
                let req = match req {
                    Ok(msg) => msg,
                    Err(DecodeError::NotEnoughData) => {
                        break;
                    }
                    Err(DecodeError::InvalidData) => {
                        log::error!("invalid data from client: {}", client);
                        return Err(ErrorKind::InvalidData.into());
                    }
                };

                let resp = match self.handle_request(req).await {
                    Ok(resp) => resp,
                    Err(e) => Response::err(e.to_string()),
                };

                let mut wbuf = BytesMut::new();
                protocol::encode(&resp, &mut wbuf, &self.aes_key);
                wfile.write_all(&wbuf).await?;
                wfile.flush().await?;
            }
        }
    }

    async fn handle_request(
        self: &Arc<Self>,
        req: protocol::Request,
    ) -> Result<protocol::Response, Box<dyn Error + Send + Sync>> {
        match req {
            Request::PullTask { id } => {
                if let Some(v) = self.agents.read().await.get_agent(&id).map(|a| &a.tasks) {
                    Ok(Response::object(v))
                } else {
                    log::warn!("Agent not found: [{}]", id);
                    Ok(Response::err("Agent not found".into()))
                }
            }
            Request::ReportStatus { id, log } => {
                // to json
                if let Err(e) = self.persist_log(id, log).await {
                    log::error!("Failed to persist log: {}", e);
                }
                Ok(Response::ok())
            }
            _ => {
                log::error!("Unhandled request: {:?}", req);
                Ok(Response::err("Unhandled request".to_string()))
            }
        }
    }

    async fn persist_log(&self, id: Uuid, log: protocol::AgentEventLog) -> Result<(), io::Error> {
        // append logs to jsonline file named by "logs/agent_id/task_id.json"
        for (tid, log) in log {
            // create parent dir
            let logs_dir = self.logs_dir.join(id.to_string());
            tokio::fs::create_dir_all(logs_dir).await?;

            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("logs/{}/{}.json", id, tid))
                .await?;

            // write each log as a json object in a line
            let mut wbuf = BytesMut::new();
            for l in log {
                let s = serde_json::to_string(&l)?;
                wbuf.extend_from_slice(s.as_bytes());
                wbuf.extend_from_slice(b"\r\n");
            }

            file.write_all(&wbuf).await?;
            file.flush().await?;
        }

        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(name = "server")]
#[command(author = "frezcirno")]
#[command(version = "1.0")]
#[command(about = "A Centralized Cron-like Task Manager - Server End", long_about = None)]
struct Args {
    /// Server config file
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,

    /// Agent data file
    #[arg(short, long, value_name = "FILE")]
    agentdb_path: PathBuf,

    /// Logs path
    #[arg(short, long, value_name = "FILE")]
    logs_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let args = Args::parse();

    let config = config::load(&args.config).expect("load config failed");

    Arc::new(Server::new(config, args.agentdb_path, args.logs_dir).await)
        .start()
        .await;
}
