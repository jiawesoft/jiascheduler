use std::env;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::Result;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use poem::web::websocket::{Message, WebSocketStream};
use russh::*;
use russh_keys::*;
use russh_sftp::client::SftpSession;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::time::timeout;
use tracing::info;

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream as TWebSocketStream};

use crate::comet::types::{Msg, MsgType};
use crate::local_time;

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
            inactivity_timeout: Some(Duration::from_secs(90)),
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

    // call for websocket proxy request
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

    // call2 for client request
    pub async fn call2(
        &self,
        _command: &str,
        cols: u32,
        rows: u32,
        sink: &mut SplitSink<
            TWebSocketStream<MaybeTlsStream<TcpStream>>,
            tokio_tungstenite::tungstenite::Message,
        >,
        mut stream: SplitStream<TWebSocketStream<MaybeTlsStream<TcpStream>>>,
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
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) =>text,
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

                            sink.send(tokio_tungstenite::tungstenite::Message::Text(String::from_utf8_lossy(&data.to_vec()).to_string())).await?;
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

#[derive(Serialize, Default, Deserialize)]
pub struct DirEntry {
    pub file_name: String,
    pub file_type: String,
    pub permissions: String,
    pub size: u64,
    pub user: String,
    pub group: String,
    pub modified: String,
}

#[derive(Serialize, Default, Deserialize)]
pub struct DirDetail {
    current_dir: String,
    entry: Vec<DirEntry>,
}

pub async fn read_dir(
    _ip: &str,
    port: u16,
    user: &str,
    password: &str,
    dir: Option<&str>,
) -> Result<DirDetail> {
    let ssh_session = Session::connect(ConnectParams {
        user,
        password,
        addrs: ("127.0.0.1", port),
    })
    .await?;

    let sft_session = ssh_session.sftp_client().await?;

    let current_dir = sft_session
        .canonicalize(
            dir.filter(|v| *v != "")
                .map_or("./".to_string(), |v| v.to_string()),
        )
        .await?;

    let dir = sft_session.read_dir(&current_dir).await?;

    let mut ret = DirDetail {
        current_dir,
        entry: vec![],
    };

    for entry in dir {
        let meta = entry.metadata();
        let permissions = format!("{}", meta.permissions());
        let file_type = format!("{:?}", entry.file_type()).to_string();
        let file_name = entry.file_name();
        let modified = local_time!(DateTime::<Utc>::from(
            UNIX_EPOCH + Duration::from_secs(meta.mtime.unwrap_or(0) as u64),
        ));
        let user = if let Some(user) = &meta.user {
            user.to_string()
        } else {
            meta.uid.unwrap_or(0).to_string()
        };

        let group = if let Some(user) = &meta.group {
            user.to_string()
        } else {
            meta.gid.unwrap_or(0).to_string()
        };
        let size = meta.size.unwrap_or(0);

        ret.entry.push(DirEntry {
            file_name,
            file_type,
            permissions,
            modified,
            size,
            user,
            group,
        })
    }
    Ok(ret)
}

pub async fn upload(
    _ip: &str,
    port: u16,
    user: &str,
    password: &str,
    filepath: &str,
    data: Vec<u8>,
) -> Result<()> {
    let dir = std::path::Path::new(filepath)
        .parent()
        .map(|v| v.to_str())
        .flatten();

    let ssh_session = Session::connect(ConnectParams {
        user,
        password,
        addrs: ("127.0.0.1", port),
    })
    .await?;

    let sftp_session = ssh_session.sftp_client().await?;
    if let Some(dir) = dir {
        let is_exists = sftp_session.try_exists(dir).await?;
        if !is_exists {
            sftp_session.create_dir(dir).await?;
        }
    }

    let mut file = sftp_session.create(filepath).await?;
    file.write_all(&data).await?;
    Ok(())
}

pub async fn remove(
    _ip: &str,
    port: u16,
    user: &str,
    password: &str,
    remove_type: &str,
    filepath: &str,
) -> Result<()> {
    let ssh_session = Session::connect(ConnectParams {
        user,
        password,
        addrs: ("127.0.0.1", port),
    })
    .await?;

    let sftp_session = ssh_session.sftp_client().await?;

    if remove_type == "dir" {
        sftp_session.remove_dir(filepath).await?;
    } else {
        sftp_session.remove_file(filepath).await?;
    }

    Ok(())
}

pub async fn download(
    _ip: &str,
    port: u16,
    user: &str,
    password: &str,
    filepath: &str,
) -> Result<Vec<u8>> {
    let ssh_session = Session::connect(ConnectParams {
        user,
        password,
        addrs: ("127.0.0.1", port),
    })
    .await?;

    let sftp_session = ssh_session.sftp_client().await?;

    let data = sftp_session.read(filepath).await?;
    Ok(data)
}
