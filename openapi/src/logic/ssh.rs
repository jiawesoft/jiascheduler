use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use async_trait::async_trait;
use automate::bridge::msg::{
    SftpDownloadParams, SftpReadDirParams, SftpRemoveParams, SftpUploadParams,
};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use poem::web::websocket::{Message, WebSocketStream};
use russh::*;
use russh_keys::*;
use russh_sftp::client::SftpSession;
use serde_json::Value;

use crate::api::terminal::types::{Msg, MsgType};
use crate::state::AppContext;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::ToSocketAddrs;
use tokio::time::timeout;
use tracing::info;

struct Client {}

#[async_trait]
impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub struct Session {
    session: client::Handle<Client>,
}

pub struct ConnectParams<A: ToSocketAddrs, U: Into<String>, P: Into<String>> {
    pub user: U,
    pub password: P,
    pub addrs: A,
}

impl Session {
    pub async fn connect<A: ToSocketAddrs, U: Into<String>, P: Into<String>>(
        ConnectParams {
            user,
            password,
            addrs,
        }: ConnectParams<A, U, P>,
    ) -> Result<Self> {
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(60)),
            keepalive_interval: Some(Duration::from_secs(10)),
            ..Default::default()
        };

        let config = Arc::new(config);
        let sh = Client {};

        let mut session =
            timeout(Duration::from_secs(1), client::connect(config, addrs, sh)).await??;

        let auth_res = session.authenticate_password(user, password).await?;

        if !auth_res {
            anyhow::bail!("Authentication failed");
        }

        Ok(Self { session })
    }

    #[allow(dead_code)]
    pub async fn connect_stream<T: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        user: String,
        password: String,
        stream: T,
    ) -> Result<Self> {
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(60)),
            keepalive_interval: Some(Duration::from_secs(10)),
            ..Default::default()
        };

        let config = Arc::new(config);
        let sh = Client {};

        let mut session = timeout(
            Duration::from_secs(1),
            client::connect_stream(config, stream, sh),
        )
        .await??;

        let auth_res = session.authenticate_password(user, password).await?;

        if !auth_res {
            anyhow::bail!("Authentication failed");
        }

        Ok(Self { session })
    }

    pub async fn call(
        &self,
        _command: &str,
        cols: u32,
        rows: u32,
        sink: &mut SplitSink<WebSocketStream, Message>,
        mut stream: SplitStream<WebSocketStream>,
    ) -> Result<u32> {
        let mut channel = self.session.channel_open_session().await?;

        // This example doesn't terminal resizing after the connection is established
        // let (w, h) = termion::terminal_size()?;
        // let (w, h) = (self.default_cols, self.default_rows);

        // Request an interactive PTY from the server
        channel
            .request_pty(
                false,
                &env::var("TERM").unwrap_or("xterm".into()),
                cols,
                rows,
                0,
                0,
                &[
                    (Pty::ECHO, 1),
                    (Pty::TTY_OP_ISPEED, 144000),
                    (Pty::TTY_OP_OSPEED, 144000),
                ], // ideally you want to pass the actual terminal modes here
            )
            .await?;

        // channel.exec(true, command).await?;
        channel.request_shell(true).await?;

        let code;

        loop {
            // Handle one of the possible mutevents:
            tokio::select! {

                result = stream.next() => {
                    let text ={
                        match result {
                            Some(Ok(Message::Text(text))) =>text,
                            _=>return Ok(1u32),
                        }

                    };
                    let msg: Msg = serde_json::from_str(text.as_str()).expect("invalid json type");

                    match msg.r#type {
                        MsgType::Resize => {
                            info!("resize {},{}",msg.cols,msg.rows);
                            channel.window_change(msg.cols, msg.rows, 0, 0).await.expect("failed resize windows");

                        },
                        MsgType::Data => {
                            channel.data(msg.msg.as_ref()).await.expect("failed send msg");
                        },
                        MsgType::Ping => {
                            channel.exec(false, "ping").await.expect("failed ping");
                        },
                    }
                },

                Some(msg) = channel.wait() => {
                    match msg {
                        // Write data to the terminal
                        ChannelMsg::Data { ref data } => {
                            sink.send(Message::Text(String::from_utf8_lossy(&data.to_vec()).to_string())).await?;
                        }
                        // The command has returned an exit code
                        ChannelMsg::ExitStatus { exit_status } => {
                            code = exit_status;
                            channel.eof().await?;
                            break;
                        }
                        _ => {}
                    }
                },
            }
        }
        Ok(code)
    }

    pub async fn sftp_client(&self) -> Result<SftpSession> {
        let channel = self.session.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await.unwrap();
        let sftp = SftpSession::new(channel.into_stream()).await.unwrap();
        Ok(sftp)
    }

    pub async fn close(&self) -> Result<()> {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}

pub struct SshLogic<'a> {
    #[allow(dead_code)]
    ctx: &'a AppContext,
}

impl<'a> SshLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn sftp_read_dir(
        &self,
        namespace: String,
        ip: String,
        port: u16,
        dir: Option<String>,
        user: String,
        password: String,
    ) -> Result<Value> {
        let logic = automate::Logic::new(self.ctx.redis().clone());
        let pair = logic.get_link_pair(namespace.clone(), ip.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/read-dir", pair.1.comet_addr);

        let body = automate::SftpReadDirRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            params: SftpReadDirParams {
                user,
                password,
                ip,
                dir,
                port,
            },
        };
        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        } else {
            Ok(ret["data"].take())
        }
    }

    pub async fn sftp_upload(
        &self,
        namespace: String,
        ip: String,
        port: u16,
        user: String,
        password: String,
        filepath: String,
        data: Vec<u8>,
    ) -> Result<String> {
        let logic = automate::Logic::new(self.ctx.redis());
        let pair = logic.get_link_pair(namespace.clone(), ip.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/upload", pair.1.comet_addr);

        let body = automate::SftpUploadRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            params: SftpUploadParams {
                ip,
                port,
                user,
                password,
                filepath,
                data,
            },
        };

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        } else {
            Ok(ret["data"].to_string())
        }
    }

    /// remove type, dir or file
    pub async fn sftp_remove(
        &self,
        namespace: String,
        ip: String,
        port: u16,
        user: String,
        password: String,
        filepath: String,
        remove_type: String,
    ) -> Result<String> {
        let logic = automate::Logic::new(self.ctx.redis().clone());
        let pair = logic.get_link_pair(namespace.clone(), ip.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/remove", pair.1.comet_addr);

        let body = automate::SftpRemoveRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            params: SftpRemoveParams {
                ip,
                port,
                user,
                password,
                filepath,
                remove_type,
            },
        };

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        } else {
            Ok(ret["data"].to_string())
        }
    }

    pub async fn sftp_download(
        &self,
        namespace: String,
        ip: String,
        port: u16,
        user: String,
        password: String,
        filepath: String,
    ) -> Result<Vec<u8>> {
        let logic = automate::Logic::new(self.ctx.redis().clone());
        let pair = logic.get_link_pair(namespace.clone(), ip.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/download", pair.1.comet_addr);

        let body = automate::SftpDownloadRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            params: SftpDownloadParams {
                ip,
                port,
                user,
                password,
                filepath,
            },
        };

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        } else {
            let data: Vec<u8> = serde_json::from_value(ret["data"].take())?;
            Ok(data)
        }
    }
}
