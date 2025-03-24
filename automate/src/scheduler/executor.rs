use anyhow::Result;
use file_rotate::{compression::Compression, suffix::AppendCount, FileRotate};

use std::io::Write;

use std::path::PathBuf;
use std::sync::Arc;
use std::{
    collections::HashMap,
    process::{Output, Stdio},
};
use tokio::sync::mpsc::Receiver;

use tokio::sync::{mpsc, Mutex};
use tracing::error;

use crate::scheduler::cmd::Cmd;

use super::types::{BaseJob, BundleOutput};

#[derive(Default)]
pub struct ExecutorBuilder {
    pub job: BaseJob,
    output_dir: String,
    disable_log: bool,
    pub env: HashMap<String, String>,
}

#[allow(unused)]
impl ExecutorBuilder {
    pub fn new() -> Self {
        Self {
            output_dir: String::from("./log"),
            ..Default::default()
        }
    }

    pub fn job(mut self, job: BaseJob) -> Self {
        self.job = job;
        self
    }

    pub fn output_dir(mut self, log_dir: impl Into<String>) -> Self {
        self.output_dir = log_dir.into();
        self
    }

    pub fn disable_write_log(mut self, disable: bool) -> Self {
        self.disable_log = disable;
        self
    }

    pub fn env(mut self, k: String, v: String) -> Self {
        self.env.insert(k, v);
        self
    }

    pub fn build(self) -> Executor {
        Executor {
            job: self.job,
            output_dir: self.output_dir,
            env: self.env,
            disable_log: self.disable_log,
        }
    }
}

pub struct Ctx {
    pub kill_signal_rx: Receiver<()>,
}

pub struct Executor {
    job: BaseJob,
    output_dir: String,
    disable_log: bool,
    env: HashMap<String, String>,
}

impl Executor {
    pub fn builder() -> ExecutorBuilder {
        ExecutorBuilder::new()
    }

    pub fn get_log_file_path(&self) -> PathBuf {
        PathBuf::from(&self.output_dir).join(format!("{}.log", self.job.eid))
    }

    pub async fn run(&self, mut ctx: Ctx) -> Result<BundleOutput> {
        if self.job.bundle_script.is_none() {
            let output = self
                .exec(
                    ctx,
                    self.job.cmd_name.clone(),
                    self.job.args.clone(),
                    self.job.code.clone(),
                )
                .await?;

            return Ok(BundleOutput::Output(output));
        }

        let kill_signal_tx: Arc<Mutex<Vec<mpsc::Sender<()>>>> = Arc::new(Mutex::new(vec![]));
        let kill_signal_tx_clone = kill_signal_tx.clone();
        let mut outputs = HashMap::new();

        let handler = tokio::spawn(async move {
            match ctx.kill_signal_rx.recv().await {
                Some(v) => {
                    for s in kill_signal_tx_clone.lock().await.to_vec() {
                        if let Err(e) = s.send(v).await {
                            error!("failed to send kill singal {e}");
                        }
                    }
                }
                None => {
                    error!("failed to recv kill signal");
                }
            };
        });

        for v in self.job.bundle_script.clone().unwrap().clone().into_iter() {
            let (tx, kill_signal_rx) = mpsc::channel::<()>(1);
            kill_signal_tx.lock().await.push(tx);
            let output = self
                .exec(
                    Ctx { kill_signal_rx },
                    v.cmd_name.clone(),
                    v.args.clone(),
                    v.code.clone(),
                )
                .await?;
            outputs.insert(v.eid, output);
        }

        handler.abort();
        return Ok(BundleOutput::Bundle(outputs));
    }

    async fn exec(
        &self,
        ctx: Ctx,
        cmd_name: String,
        args: Vec<String>,
        code: String,
    ) -> Result<Output> {
        let mut cmd = Cmd::new(cmd_name);
        let mut args = args;
        if self.job.read_code_from_stdin {
            cmd = cmd.read_code_from_stdin(&code);
            cmd.get_ref().stdin(Stdio::piped());
        } else {
            args.push(code.clone());
        }

        if let Some(ref work_dir) = self.job.work_dir {
            cmd.work_dir(work_dir);
        }

        if let Some(ref work_user) = self.job.work_user {
            cmd.work_user(work_user)?;
        }
        if self.job.timeout > 0 {
            cmd.timeout(self.job.timeout);
        }

        for (key, val) in self.env.iter() {
            cmd.get_ref().env(key, val);
        }

        cmd.get_ref().args(&args);

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        let filepath = self.get_log_file_path();
        let mut logfile = if self.disable_log {
            None
        } else {
            Some(FileRotate::new(
                filepath,
                AppendCount::new(2),
                file_rotate::ContentLimit::Bytes(1 << 20),
                Compression::None,
                None,
            ))
        };

        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if let Some(f) = logfile.as_mut() {
                    if let Err(e) = write!(f, "{}", line) {
                        error!("cannot write to log file - {e}");
                    }
                }
            }
        });

        cmd.get_ref().stdout(Stdio::piped());
        cmd.get_ref().stderr(Stdio::piped());

        let output = cmd.wait_with_output(tx, ctx.kill_signal_rx).await?;

        Ok(output)
    }
}

#[tokio::test]
async fn test_command_exec() {
    use nanoid::nanoid;
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::info;
    std::env::set_var("RUST_LOG", "debug");
    tracing_subscriber::fmt::init();
    let c = Executor::builder()
        .job(BaseJob {
            bundle_script: None,
            eid: nanoid!(),
            cmd_name: "bash".to_string(),
            code: "ls -alh;sleep 20;echo hello".into(),
            args: vec!["-c".to_string()],
            upload_file: None,
            read_code_from_stdin: false,
            timeout: 2,
            work_dir: None,
            work_user: None,
            max_retry: 1,
            max_parallel: 1,
        })
        .build();

    let (kill_signal_tx, kill_signal_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;
        info!("start manual kill");
        kill_signal_tx.send(()).await.unwrap();
        info!("end manual kill");
    });
    let output = c.run(Ctx { kill_signal_rx }).await.unwrap();

    println!("stdout: {:?}", output.get_stdout());
    println!("stderr: {:?}", output.get_stderr());
    println!("exit_status: {:?}", output.get_exit_status());
    println!("exit_code: {:?}", output.get_exit_code())
}
