use std::net::IpAddr;

use crate::{
    bridge::msg::{
        AgentOfflineParams, AgentOnlineParams, HeartbeatParams, MsgReqKind, UpdateJobParams,
    },
    bus::Bus,
    get_endpoint, LinkPair,
};
use anyhow::{Ok, Result};
use local_ip_address::linux::local_ip;
use redis::{AsyncCommands, FromRedisValue, RedisResult};

use serde_json::{json, Value};

use super::types::{self};

#[derive(Clone)]
pub struct Logic {
    pub redis_client: redis::Client,
    local_ip: IpAddr,
    bus: Bus,
}

impl Logic {
    pub fn new(redis: redis::Client) -> Self {
        Self {
            local_ip: local_ip().expect("failed get local ip"),
            redis_client: redis.clone(),
            bus: Bus::new(redis),
        }
    }

    pub fn get_agent_key(&self, namespace: impl Into<String>, ip: impl Into<String>) -> String {
        get_endpoint(namespace, ip)
    }

    async fn set_link_pair<T: Into<String>>(
        &self,
        namespace: T,
        agent_ip: T,
        port: u16,
    ) -> Result<()> {
        let mut conn = self.get_async_connection().await?;
        let key = self.get_agent_key(namespace, agent_ip);
        let ret = conn
            .set_ex(
                key,
                types::LinkPair {
                    comet_addr: format!("{}:{}", self.local_ip.to_string(), port),
                },
                10,
            )
            .await?;
        Ok(ret)
    }

    pub async fn get_link_pair<T: Into<String>>(
        &self,
        namespace: T,
        agent_ip: T,
    ) -> Result<(String, types::LinkPair)> {
        let (namespace, agent_ip) = (namespace.into(), agent_ip.into());
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;
        let key = self.get_agent_key(namespace.clone(), agent_ip.clone());
        let val = conn.get(key.clone()).await?;

        if val == redis::Value::Nil {
            anyhow::bail!("Agent {agent_ip}:{namespace} not registered, please deploy first");
        }

        Ok((key.clone(), LinkPair::from_redis_value(&val)?))
    }

    pub async fn get_async_connection(&self) -> RedisResult<redis::aio::MultiplexedConnection> {
        self.redis_client.get_multiplexed_async_connection().await
    }

    pub async fn dispath(&self, req: types::DispatchJobRequest) -> Result<(String, MsgReqKind)> {
        let pair = self.get_link_pair(&req.namespace, &req.agent_ip).await?;
        Ok((pair.0, MsgReqKind::DispatchJobRequest(req.dispatch_params)))
    }

    pub async fn sfpt_read_dir(
        &self,
        req: types::SftpReadDirRequest,
    ) -> Result<(String, MsgReqKind)> {
        let key = self.get_agent_key(&req.namespace, &req.agent_ip);
        let msg = MsgReqKind::SftpReadDirRequest(req.params);
        Ok((key, msg))
    }

    pub async fn sftp_upload(&self, req: types::SftpUploadRequest) -> Result<(String, MsgReqKind)> {
        let key = self.get_agent_key(&req.namespace, &req.agent_ip);
        let msg = MsgReqKind::SftpUploadRequest(req.params);
        Ok((key, msg))
    }

    pub async fn sftp_download(
        &self,
        req: types::SftpDownloadRequest,
    ) -> Result<(String, MsgReqKind)> {
        let key = self.get_agent_key(&req.namespace, &req.agent_ip);
        let msg = MsgReqKind::SftpDownloadRequest(req.params);
        Ok((key, msg))
    }

    pub async fn sftp_remove(&self, req: types::SftpRemoveRequest) -> Result<(String, MsgReqKind)> {
        let key = self.get_agent_key(&req.namespace, &req.agent_ip);
        let msg = MsgReqKind::SftpRemoveRequest(req.params);
        Ok((key, msg))
    }

    pub async fn runtime_action(
        &self,
        req: types::RuntimeActionRequest,
    ) -> Result<(String, MsgReqKind)> {
        let pair = self.get_link_pair(&req.namespace, &req.agent_ip).await?;
        Ok((pair.0, MsgReqKind::RuntimeActionRequest(req.action_params)))
    }

    pub async fn update_job(&self, req: UpdateJobParams) -> Result<Value> {
        self.bus.update_job(req).await?;
        Ok(json!(null))
    }

    pub async fn agent_online(&self, req: AgentOnlineParams) -> Result<Value> {
        self.bus.agent_online(req).await?;
        Ok(json!(null))
    }

    pub async fn agent_offline(&self, req: AgentOfflineParams) -> Result<Value> {
        self.bus.agent_offline(req).await?;
        Ok(json!(null))
    }

    pub async fn heartbeat(&self, v: HeartbeatParams, port: u16) -> Result<Value> {
        self.set_link_pair(&v.namespace, &v.source_ip, port).await?;
        self.bus.heartbeat(v).await?;
        Ok(json!({"data":"heartbeat success"}))
    }
}
