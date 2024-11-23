use std::{collections::HashMap, sync::Arc};

use anyhow::Ok;
use futures::SinkExt;

use handler::SecretHeader;
use poem::web::websocket::WebSocketStream;
use serde_json::{json, Value};
use tokio::sync::{mpsc::Sender, Mutex};
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
        let mac_address = secret_header.mac_address.clone();
        let key = get_endpoint(ip.clone(), mac_address.clone());
        info!("{ip}:{namespace}:{} online", secret_header.mac_address);

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
