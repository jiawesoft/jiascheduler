use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use automate::bridge::msg::{
    SftpDownloadChunkParams, SftpDownloadFinishParams, SftpDownloadParams,
    SftpDownloadStatParams, SftpReadDirParams,
    SftpRemoveParams, SftpUploadChunkParams, SftpUploadFinishParams, SftpUploadParams,
    SftpUploadStartParams,
};
use automate::comet::types::{
    SftpDownloadChunkRequest, SftpDownloadFinishRequest, SftpDownloadStatRequest,
    SftpUploadChunkRequest, SftpUploadFinishRequest, SftpUploadStartRequest,
};
use automate::ssh::AuthData;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use poem::web::websocket::{Message, WebSocketStream};
use russh::keys::*;
use russh::*;
use russh_sftp::client::SftpSession;
use serde_json::Value;

use crate::state::AppContext;

/// Chunk size, kept in sync with the agent side (512 KiB).
pub const SFTP_CHUNK_SIZE: usize = 512 * 1024;

use serde::{self, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::ToSocketAddrs;
use tokio::time::timeout;

#[derive(Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum MsgType {
    Resize = 1,
    Data = 2,
    Ping = 3,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Msg {
    pub r#type: MsgType,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub cols: u32,
    #[serde(default)]
    pub rows: u32,
}

struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
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

pub struct ConnectParams2<A: ToSocketAddrs, U: Into<String>> {
    pub user: U,
    pub auth: AuthData,
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

        if !auth_res.success() {
            anyhow::bail!("Authentication failed");
        }

        Ok(Self { session })
    }

    /// Connect to a server with the auth data configured for an instance login
    /// user (password, key file path or inline key content).
    pub async fn connect2<A: ToSocketAddrs, U: Into<String>>(
        ConnectParams2 { user, auth, addrs }: ConnectParams2<A, U>,
    ) -> Result<Self> {
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(90)),
            keepalive_interval: Some(Duration::from_secs(10)),
            ..Default::default()
        };

        let config = Arc::new(config);
        let sh = Client {};

        let user: String = user.into();

        let mut session =
            timeout(Duration::from_secs(1), client::connect(config, addrs, sh)).await??;

        let mut h = async |user, key_pair| {
            session
                .authenticate_publickey(
                    user,
                    PrivateKeyWithHashAlg::new(
                        Arc::new(key_pair),
                        session.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
        };

        let auth_res = match auth {
            AuthData::Password(password) => session.authenticate_password(user, password).await?,
            AuthData::KeyPath(path) => {
                let key_pair = load_secret_key(path, None)?;
                h(user, key_pair).await?
            }
            AuthData::KeyContent(val) => {
                let key_pair = decode_secret_key(&val, None)?;
                h(user, key_pair).await?
            }
        };

        if !auth_res.success() {
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
            inactivity_timeout: Some(Duration::from_secs(90)),
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

        if !auth_res.success() {
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
        mac_addr: String,
        port: u16,
        dir: Option<String>,
        user: String,
        auth_data: AuthData,
    ) -> Result<Value> {
        let logic = automate::Logic::new(self.ctx.redis().clone());
        let pair = logic.get_link_pair(ip.clone(), mac_addr.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/read-dir", pair.1.comet_addr);

        let body = automate::SftpReadDirRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            params: SftpReadDirParams {
                user,
                auth_data,
                ip,
                dir,
                port,
            },
            mac_addr,
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
        mac_addr: String,
        port: u16,
        user: String,
        auth_data: AuthData,
        filepath: String,
        data: Vec<u8>,
    ) -> Result<String> {        let logic = automate::Logic::new(self.ctx.redis());
        let pair = logic.get_link_pair(ip.clone(), mac_addr.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/upload", pair.1.comet_addr);

        let body = automate::SftpUploadRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            mac_addr,
            params: SftpUploadParams {
                ip,
                port,
                user,
                auth_data,
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
        mac_addr: String,
        port: u16,
        user: String,
        auth_data: AuthData,
        filepath: String,
        remove_type: String,
    ) -> Result<String> {
        let logic = automate::Logic::new(self.ctx.redis().clone());
        let pair = logic.get_link_pair(ip.clone(), mac_addr.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/remove", pair.1.comet_addr);

        let body = automate::SftpRemoveRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            mac_addr,
            params: SftpRemoveParams {
                ip,
                port,
                user,
                auth_data,
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
        mac_addr: String,
        port: u16,
        user: String,
        auth_data: AuthData,
        filepath: String,
    ) -> Result<Vec<u8>> {
        let logic = automate::Logic::new(self.ctx.redis().clone());
        let pair = logic.get_link_pair(ip.clone(), mac_addr.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/download", pair.1.comet_addr);

        let body = automate::SftpDownloadRequest {
            agent_ip: ip.clone(),
            namespace: namespace.clone(),
            mac_addr,
            params: SftpDownloadParams {
                ip,
                port,
                user,
                auth_data,
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

    /// Chunked upload: start a session and return the chunk size.
    pub async fn sftp_upload_start(
        &self,
        namespace: String,
        ip: String,
        mac_addr: String,
        port: u16,
        user: String,
        auth_data: AuthData,
        filepath: String,
        total_size: u64,
        session_id: String,
    ) -> Result<u64> {
        let logic = automate::Logic::new(self.ctx.redis());
        let pair = logic.get_link_pair(ip.clone(), mac_addr.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/upload/start", pair.1.comet_addr);

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&SftpUploadStartRequest {
                agent_ip: ip,
                namespace,
                mac_addr,
                params: SftpUploadStartParams {
                    session_id,
                    ip: String::new(),
                    port,
                    user,
                    auth_data,
                    filepath,
                    total_size,
                },
            })
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        }

        Ok(ret["data"]["chunk_size"].as_u64().unwrap_or(0))
    }

    /// Chunked upload: write one chunk and return the offset after it.
    pub async fn sftp_upload_chunk(
        &self,
        namespace: String,
        comet_addr: String,
        mac_addr: String,
        params: SftpUploadChunkParams,
    ) -> Result<u64> {
        let api_url = format!("http://{}/sftp/tunnel/upload/chunk", comet_addr);
        let ip = params.ip.clone();

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&SftpUploadChunkRequest {
                agent_ip: ip,
                namespace,
                mac_addr,
                params,
            })
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        }

        Ok(ret["data"]["next_offset"].as_u64().unwrap_or(0))
    }

    /// Chunked upload: finish and verify the remote size.
    pub async fn sftp_upload_finish(
        &self,
        namespace: String,
        comet_addr: String,
        mac_addr: String,
        params: SftpUploadFinishParams,
    ) -> Result<Value> {
        let api_url = format!("http://{}/sftp/tunnel/upload/finish", comet_addr);
        let ip = params.ip.clone();

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&SftpUploadFinishRequest {
                agent_ip: ip,
                namespace,
                mac_addr,
                params,
            })
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        }

        Ok(ret["data"].take())
    }

    /// Chunked download: query the remote size and the suggested chunk size.
    #[allow(clippy::too_many_arguments)]
    pub async fn sftp_download_stat(
        &self,
        namespace: String,
        ip: String,
        mac_addr: String,
        port: u16,
        user: String,
        auth_data: AuthData,
        filepath: String,
        session_id: String,
    ) -> Result<(u64, u64)> {
        let logic = automate::Logic::new(self.ctx.redis());
        let pair = logic.get_link_pair(ip.clone(), mac_addr.clone()).await?;
        let api_url = format!("http://{}/sftp/tunnel/download/stat", pair.1.comet_addr);

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&SftpDownloadStatRequest {
                agent_ip: ip.clone(),
                namespace,
                mac_addr,
                params: SftpDownloadStatParams {
                    session_id,
                    ip,
                    port,
                    user,
                    auth_data,
                    filepath,
                },
            })
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        }

        Ok((
            ret["data"]["size"].as_u64().unwrap_or(0),
            ret["data"]["chunk_size"].as_u64().unwrap_or(0),
        ))
    }

    /// Stream a remote file to an HTTP response body.
    ///
    /// Chunks are pulled from the agent one at a time and yielded immediately, so
    /// neither the console nor the browser has to hold the whole file. The total
    /// size is known upfront, which lets the caller send a `Content-Length` and
    /// gives the browser a native progress indicator.
    pub fn sftp_download_stream(
        &self,
        namespace: String,
        ip: String,
        mac_addr: String,
        port: u16,
        user: String,
        auth_data: AuthData,
        filepath: String,
    ) -> impl futures::Stream<Item = Result<Vec<u8>, std::io::Error>> + Send + 'static {
        // AppContext is cheap to clone (pool handles and Arcs) and the returned
        // stream must be 'static, so it owns its own context.
        let ctx = self.ctx.clone();

        async_stream::stream! {
            let logic = SshLogic::new(&ctx);

            let pair = match automate::Logic::new(ctx.redis())
                .get_link_pair(ip.clone(), mac_addr.clone())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("download stream: cannot resolve comet address - {e}");
                    return;
                }
            };

            let session_id = format!("dl-{}", nanoid::nanoid!(16));

            // Reuse stat to learn the size and to open the agent side session
            // that the following chunk requests rely on.
            let (total, _chunk) = match logic
                .sftp_download_stat(
                    namespace.clone(),
                    ip.clone(),
                    mac_addr.clone(),
                    port,
                    user.clone(),
                    auth_data.clone(),
                    filepath.clone(),
                    session_id.clone(),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("download stream: stat failed - {e}");
                    return;
                }
            };

            let chunk_size = SFTP_CHUNK_SIZE as u64;
            let mut offset = 0u64;
            while offset < total {
                let len = chunk_size.min(total - offset);
                match logic
                    .sftp_download_chunk(
                        namespace.clone(),
                        pair.1.comet_addr.clone(),
                        mac_addr.clone(),
                        SftpDownloadChunkParams {
                            session_id: session_id.clone(),
                            ip: ip.clone(),
                            port,
                            user: user.clone(),
                            auth_data: auth_data.clone(),
                            filepath: filepath.clone(),
                            offset,
                            len: len as u32,
                        },
                    )
                    .await
                {
                    Ok(data) if !data.is_empty() => {
                        offset += data.len() as u64;
                        yield Ok(data);
                    }
                    Ok(_) => break,
                    Err(e) => {
                        tracing::error!("download stream: chunk at {offset} failed - {e}");
                        break;
                    }
                }
            }

            // Release the agent side session instead of waiting for the idle timeout.
            let _ = logic
                .sftp_download_finish(
                    namespace,
                    pair.1.comet_addr,
                    mac_addr,
                    SftpDownloadFinishParams {
                        session_id,
                    },
                    ip,
                )
                .await;
        }
    }

    /// Chunked download: fetch one chunk.
    #[allow(clippy::too_many_arguments)]
    pub async fn sftp_download_chunk(
        &self,
        namespace: String,
        comet_addr: String,
        mac_addr: String,
        params: SftpDownloadChunkParams,
    ) -> Result<Vec<u8>> {
        let api_url = format!("http://{}/sftp/tunnel/download/chunk", comet_addr);
        let ip = params.ip.clone();

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&SftpDownloadChunkRequest {
                agent_ip: ip,
                namespace,
                mac_addr,
                params,
            })
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        }

        Ok(serde_json::from_value(ret["data"].take())?)
    }

    /// Chunked download: end the session and release the agent side connection.
    pub async fn sftp_download_finish(
        &self,
        namespace: String,
        comet_addr: String,
        mac_addr: String,
        params: SftpDownloadFinishParams,
        agent_ip: String,
    ) -> Result<()> {
        let api_url = format!("http://{}/sftp/tunnel/download/finish", comet_addr);

        let mut ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&SftpDownloadFinishRequest {
                agent_ip,
                namespace,
                mac_addr,
                params,
            })
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!(ret["msg"].take().to_string())
        }

        Ok(())
    }

    /// Resolve the comet address of an instance.
    pub async fn get_comet_addr(
        &self,
        ip: &str,
        mac_addr: &str,
    ) -> Result<String> {
        let logic = automate::Logic::new(self.ctx.redis());
        let pair = logic.get_link_pair(ip.to_string(), mac_addr.to_string()).await?;
        Ok(pair.1.comet_addr)
    }
}
