use std::{ffi::OsStr, process::Output, time::Duration};

use anyhow::{anyhow, Result};
use bytes::BufMut;

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt},
    process::Command,
    sync::mpsc::{Receiver, UnboundedSender},
};
use tracing::error;

async fn read_to_end<A: AsyncRead + Unpin>(
    io: &mut Option<A>,
    tx: UnboundedSender<String>,
) -> std::io::Result<Vec<u8>> {
    let mut vec = Vec::new();
    if let Some(io) = io.as_mut() {
        let mut reader = tokio::io::BufReader::new(io);
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;

            if n == 0 {
                break;
            }

            if let Err(e) = tx.send(line.clone()) {
                error!("failed send job lot - {e}");
            }

            vec.put(line.as_bytes());
        }
    }

    std::result::Result::Ok(vec)
}

pub struct Cmd<'a> {
    inner: Command,
    timeout: Option<Duration>,
    read_code_from_stdin: (bool, &'a str),
}

impl<'a> Cmd<'a> {
    pub fn new<T: AsRef<OsStr>>(program: T) -> Self {
        Self {
            inner: Command::new(program),
            read_code_from_stdin: (false, ""),
            timeout: None,
        }
    }

    pub fn get_ref(&mut self) -> &mut Command {
        &mut self.inner
    }

    pub fn work_dir(&mut self, dir: &str) -> &mut Self {
        self.inner.current_dir(dir);
        self
    }

    pub fn timeout(&mut self, timeout: u64) -> &mut Self {
        self.timeout = Some(Duration::from_secs(timeout));
        self
    }

    pub fn work_user(&mut self, user: &str) -> Result<&mut Self> {
        let u = users::get_user_by_name(user).ok_or(anyhow!("invalid system user {user}"))?;
        self.inner.uid(u.uid());
        Ok(self)
    }

    pub fn read_code_from_stdin(mut self, code: &'a str) -> Self {
        self.read_code_from_stdin = (true, code);
        self
    }

    pub async fn wait_with_output(
        &mut self,
        tx: UnboundedSender<String>,
        mut kill_signal_rx: Receiver<()>,
    ) -> Result<Output> {
        let mut child = self.inner.spawn()?;

        if self.read_code_from_stdin.0 {
            if let Some(mut stdin_pipe) = child.stdin.take() {
                stdin_pipe
                    .write_all(self.read_code_from_stdin.1.as_bytes())
                    .await?;
            }
        }

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();

        let stdout_fut = read_to_end(&mut stdout_pipe, tx.clone());
        let stderr_fut = read_to_end(&mut stderr_pipe, tx.clone());

        let sleep = self
            .timeout
            .map_or(tokio::time::sleep(Duration::from_secs(600)), |v| {
                tokio::time::sleep(v)
            });
        tokio::pin!(sleep);

        tokio::select! {
            _ = &mut sleep => child.kill().await?,
            _ = kill_signal_rx.recv() => child.kill().await?,
            ret = child.wait() =>{
                ret?;
            },

        };

        let (status, stdout, stderr) =
            futures_util::future::try_join3(child.wait(), stdout_fut, stderr_fut).await?;

        // Drop happens after `try_join` due to <https://github.com/tokio-rs/tokio/issues/4309>
        drop(stdout_pipe);
        drop(stderr_pipe);

        Ok(Output {
            status,
            stderr,
            stdout,
        })
    }
}