use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::{Context, Ok};
use futures::SinkExt;

use handler::{middleware::bearer_auth, SecretHeader};
use poem::{
    get, listener::TcpListener, post, web::websocket::WebSocketStream, EndpointExt, Route, Server,
};
use serde_json::{json, Value};
use tokio::sync::{mpsc::Sender, oneshot::Sender as OneSender, Mutex};
use tracing::{debug, error, info};
use types::SshLoginParams;

use crate::{
    bridge::{
        msg::{
            AgentOfflineParams, AgentOnlineParams, HeartbeatParams, Msg, MsgReqKind, MsgState,
            UpdateJobParams,
        },
        Bridge,
    },
    get_endpoint,
};

use anyhow::Result;

use self::logic::Logic;

pub mod handler;
pub mod logic;
mod macros;
pub mod types;

#[derive(Clone)]
pub struct Comet {
    pub bridge: Bridge,
    logic: Logic,
    secret: String,
    port: u16,
    pub ssh_ws_streams: Arc<Mutex<HashMap<String, WebSocketStream>>>,
}

impl Comet {
    pub fn new(redis_client: redis::Client, port: u16, secret: String) -> Self {
        Self {
            bridge: Bridge::new(),
            logic: Logic::new(redis_client),
            ssh_ws_streams: Arc::new(Mutex::new(HashMap::new())),
            port,
            secret,
        }
    }

    pub async fn register_ssh_stream(&mut self, key: String, ws: WebSocketStream) {
        self.ssh_ws_streams.lock().await.insert(key.clone(), ws);
        debug!("completed register ssh stream {key}");
    }

    pub async fn get_ssh_stream(&self, params: SshLoginParams) -> Option<WebSocketStream> {
        let key = get_endpoint(&params.ip, &params.mac_addr);
        debug!("get ssh stream {key}");
        if let Some(mut stream) = self.ssh_ws_streams.lock().await.remove(&key) {
            let res =
                serde_json::to_string(&params).expect("failed convert InstanceLoginParams to json");

            stream
                .send(poem::web::websocket::Message::text(res.to_string()))
                .await
                .expect("failed send ready msg");
            Some(stream)
        } else {
            None
        }
    }

    pub async fn pull_job(&self, _v: Value) -> Result<Value> {
        Ok(json!({"data":"success"}))
    }

    pub async fn client_online(
        &mut self,
        secret_header: SecretHeader,
        is_initialized: bool,
        namespace: String,
        ip: String,
        client: Sender<(Msg, Option<Sender<MsgState>>)>,
    ) {
        let mac_address = secret_header.mac_addr.clone();
        let key = get_endpoint(ip.clone(), mac_address.clone());
        info!("{ip}:{namespace}:{} online", secret_header.mac_addr);

        self.bridge.append_client(key, client).await;
        let ret = self
            .logic
            .agent_online(AgentOnlineParams {
                is_initialized,
                agent_ip: ip,
                mac_addr: mac_address,
                namespace,
                secret_header,
            })
            .await;
        if let Err(e) = ret {
            error!("failed to send agent online event - {e}")
        }
    }

    pub async fn client_offline(&self, ip: String, mac_address: String) {
        {
            let key = get_endpoint(ip.clone(), mac_address.clone());
            self.ssh_ws_streams.lock().await.remove(&key);
        }

        let ret = self
            .logic
            .agent_offline(AgentOfflineParams {
                agent_ip: ip,
                mac_addr: mac_address.clone(),
            })
            .await;

        if let Err(e) = ret {
            error!("failed to send agent offline event - {e}")
        }
    }

    pub async fn dispatch(&self, req: types::DispatchJobRequest) -> Result<Value> {
        let val = self.logic.dispath(req).await?;
        let ret = self.bridge.send_msg(&val.0, val.1).await?;
        Ok(ret)
    }

    pub async fn runtime_action(&self, req: types::RuntimeActionRequest) -> Result<Value> {
        let val = self.logic.runtime_action(req).await?;
        let ret = self.bridge.send_msg(&val.0, val.1).await?;
        Ok(ret)
    }

    pub async fn sftp_read_dir(&self, req: types::SftpReadDirRequest) -> Result<Value> {
        let val = self.logic.sfpt_read_dir(req).await?;
        let ret = self.bridge.send_msg(&val.0, val.1).await?;
        Ok(ret)
    }

    pub async fn sftp_upload(&self, req: types::SftpUploadRequest) -> Result<Value> {
        let val = self.logic.sftp_upload(req).await?;
        let ret = self.bridge.send_msg(&val.0, val.1).await?;
        Ok(ret)
    }
    pub async fn sftp_download(&self, req: types::SftpDownloadRequest) -> Result<Value> {
        let val = self.logic.sftp_download(req).await?;
        let ret = self.bridge.send_msg(&val.0, val.1).await?;
        Ok(ret)
    }

    pub async fn sftp_remove(&self, req: types::SftpRemoveRequest) -> Result<Value> {
        let val = self.logic.sftp_remove(req).await?;
        let ret = self.bridge.send_msg(&val.0, val.1).await?;
        Ok(ret)
    }

    pub async fn heartbeat(&self, req: HeartbeatParams) -> Result<Value> {
        let v = self.logic.heartbeat(req, self.port).await?;
        Ok(v)
    }

    pub async fn update_job(&self, req: UpdateJobParams) -> Result<Value> {
        let ret = self.logic.update_job(req).await?;
        Ok(ret)
    }

    pub async fn handle(&self, msg: MsgReqKind) -> Value {
        match msg {
            MsgReqKind::PullJobRequest(v) => self.pull_job(v).await,
            MsgReqKind::HeartbeatRequest(v) => self.heartbeat(v).await,
            MsgReqKind::UpdateJobRequest(v) => self.update_job(v).await,
            _ => todo!(),
        }
        .map_or_else(
            |e| {
                error!("failed handle msg - {e}");
                json!({
                    "error":e.to_string()
                })
            },
            |v| v,
        )
    }
}

pub struct CometOptions {
    pub redis_url: String,
    pub bind_addr: String,
    pub secret: String,
}

pub async fn run(opts: CometOptions, signal: Option<OneSender<()>>) -> Result<()> {
    let redis_client = redis::Client::open(opts.redis_url).context("failed connect to redis")?;
    let port = opts
        .bind_addr
        .parse::<SocketAddr>()
        .context("failed parse bind address")?
        .port();
    let comet = Comet::new(redis_client, port, opts.secret.clone());
    let app = Route::new()
        .at(
            "/dispatch",
            post(
                handler::dispatch
                    .with(bearer_auth(&opts.secret))
                    .data(comet.clone()),
            ),
        )
        .at(
            "runtime/action",
            post(
                handler::runtime_action
                    .with(bearer_auth(&opts.secret))
                    .data(comet.clone()),
            ),
        )
        .at(
            "/file/get/:filename",
            get(handler::get_file
                .with(bearer_auth(&opts.secret))
                .data(comet.clone())),
        )
        .at(
            "/evt/:namespace",
            get(handler::ws
                .with(bearer_auth(&opts.secret))
                .data(comet.clone())),
        )
        .at(
            "/ssh/register/:ip",
            handler::ssh_register
                .with(bearer_auth(&opts.secret))
                .data(comet.clone()),
        )
        .at(
            "/ssh/tunnel",
            handler::proxy_ssh
                .with(bearer_auth(&opts.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/read-dir",
            handler::sftp_read_dir
                .with(bearer_auth(&opts.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/upload",
            handler::sftp_upload
                .with(bearer_auth(&opts.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/remove",
            handler::sftp_remove
                .with(bearer_auth(&opts.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/download",
            handler::sftp_download
                .with(bearer_auth(&opts.secret))
                .data(comet.clone()),
        );
    if let Some(tx) = signal {
        tx.send(()).expect("failed send signal");
    }
    Ok(Server::new(TcpListener::bind(opts.bind_addr))
        .run(app)
        .await?)
}
