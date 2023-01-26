use async_trait::async_trait;
use protocol::{CommandSpec, FileSpec, HostSpec, TaskError, TaskResult};
use shellexpand::tilde;
use std::path::Path;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

pub type AsyncTask = Box<dyn AsyncTaskTrait + Send + Sync>;
pub type AsyncTaskResult = Result<TaskResult, TaskError>;

#[async_trait]
pub trait AsyncTaskTrait {
    async fn run(&self) -> AsyncTaskResult;
}

pub struct FileUpdateTask {
    pub file_spec: FileSpec,
}

#[async_trait]
impl AsyncTaskTrait for FileUpdateTask {
    async fn run(&self) -> AsyncTaskResult {
        let resp = match reqwest::get(&self.file_spec.url).await {
            Ok(resp) => resp,
            Err(e) => {
                return Err(TaskError::NetError(e.to_string()));
            }
        };
        let bytes = match resp.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(TaskError::NetError(e.to_string()));
            }
        };

        let path = tilde(&self.file_spec.path);
        let mut file = File::create(path.as_ref()).await?;
        file.write_all(&bytes).await?;
        Ok(TaskResult {
            status: Some(0),
            message: "".to_string(),
        })
    }
}

pub struct CommandTask {
    pub command_spec: CommandSpec,
}

#[async_trait]
impl AsyncTaskTrait for CommandTask {
    /// execute command
    async fn run(&self) -> AsyncTaskResult {
        let mut args = vec![self.command_spec.cmd.clone()];
        args.extend(self.command_spec.args.clone());

        if self.command_spec.shell {
            let os = std::env::consts::OS;
            match os {
                "windows" => {
                    let mut prefix =
                        vec!["C:\\Windows\\System32\\cmd.exe".to_owned(), "/C".to_owned()];
                    prefix.extend(args);
                    args = prefix;
                }
                "linux" => {
                    // https://stackoverflow.com/a/1254322
                    let mut prefix = vec!["/bin/sh".to_owned(), "-c".to_owned(), args.join(" ")];
                    prefix.extend(args);
                    args = prefix;
                }
                _ => {
                    return Err(TaskError::UnsupportedPlatform(os.to_string()));
                }
            };
        };

        let mut child = tokio::process::Command::new(&args[0])
            .current_dir(&self.command_spec.cwd)
            .args(args[1..].iter())
            .spawn()?;
        let exit = child.wait().await?;
        Ok(TaskResult {
            status: exit.code(),
            message: "".to_string(),
        })
    }
}

pub struct HostTask {
    pub host_spec: HostSpec,
}

#[async_trait]
impl AsyncTaskTrait for HostTask {
    /// add host entry to hosts file
    async fn run(&self) -> AsyncTaskResult {
        let platform = std::env::consts::OS;
        let path = match platform {
            "windows" => Path::new("C:\\Windows\\System32\\drivers\\etc\\hosts"),
            "linux" => Path::new("/etc/hosts"),
            _ => {
                return Err(TaskError::UnsupportedPlatform(platform.to_string()));
            }
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        let mut lines = content.lines();
        let mut new_content = String::new();
        let mut found = false;
        let line_ending = if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        while let Some(line) = lines.next() {
            if line.trim_start().starts_with(&self.host_spec.ip) {
                found = true;
                new_content.push_str(&self.host_spec.ip);
                let origin: Vec<&str> = line.split_ascii_whitespace().skip(1).collect();
                for host in &origin {
                    new_content.push_str(" ");
                    new_content.push_str(host);
                }
                let keep_hosts: Vec<&str> = self
                    .host_spec
                    .hosts
                    .iter()
                    .map(|s| s.as_str())
                    .filter(|h| origin.contains(h))
                    .collect();
                for host in keep_hosts {
                    new_content.push_str(" ");
                    new_content.push_str(host);
                }
                new_content.push_str(line_ending);
            } else {
                new_content.push_str(line);
                new_content.push_str(line_ending);
            }
        }

        // add a new host entry
        if !found {
            new_content.push_str(&self.host_spec.ip);
            for host in &self.host_spec.hosts {
                new_content.push_str(" ");
                new_content.push_str(host);
            }
            new_content.push_str(line_ending);
        }

        file.write_all(new_content.as_bytes()).await?;
        Ok(TaskResult {
            status: Some(0),
            message: "".to_string(),
        })
    }
}
