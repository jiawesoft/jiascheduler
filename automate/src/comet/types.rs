use serde::{Deserialize, Serialize};

use crate::bridge::msg::{
    DispatchJobParams, RuntimeActionParams, SftpDownloadChunkParams, SftpDownloadParams,
    SftpDownloadFinishParams, SftpDownloadStatParams, SftpReadDirParams, SftpRemoveParams,
    SftpUploadChunkParams, SftpUploadFinishParams, SftpUploadParams, SftpUploadStartParams,
};
use redis_macros::{FromRedisValue, ToRedisArgs};
use serde_repr::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct DispatchJobRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub dispatch_params: DispatchJobParams,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RuntimeActionRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub action_params: RuntimeActionParams,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SftpReadDirRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpReadDirParams,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SftpUploadRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpUploadParams,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SftpRemoveRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpRemoveParams,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SftpDownloadRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpDownloadParams,
}

/// Chunked upload: start.
#[derive(Serialize, Deserialize, Debug)]
pub struct SftpUploadStartRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpUploadStartParams,
}

/// Chunked upload: one chunk.
#[derive(Serialize, Deserialize, Debug)]
pub struct SftpUploadChunkRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpUploadChunkParams,
}

/// Chunked upload: finish.
#[derive(Serialize, Deserialize, Debug)]
pub struct SftpUploadFinishRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpUploadFinishParams,
}

/// Chunked download: query the size.
#[derive(Serialize, Deserialize, Debug)]
pub struct SftpDownloadStatRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpDownloadStatParams,
}

/// Chunked download: end the session.
#[derive(Serialize, Deserialize, Debug)]
pub struct SftpDownloadFinishRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpDownloadFinishParams,
}

/// Chunked download: one chunk.
#[derive(Serialize, Deserialize, Debug)]
pub struct SftpDownloadChunkRequest {
    pub agent_ip: String,
    pub mac_addr: String,
    pub namespace: String,
    pub params: SftpDownloadChunkParams,
}

#[derive(Serialize, Clone, FromRedisValue, Deserialize, ToRedisArgs)]
pub struct LinkPair {
    pub namespace: String,
    pub comet_addr: String,
}
impl ToString for LinkPair {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

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

#[derive(Deserialize)]
pub struct WebSshQuery {
    pub namespace: String,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SshLoginParams {
    pub cols: u32,
    pub rows: u32,
    pub namespace: String,
    pub ip: String,
    pub mac_addr: String,
    pub connect_options: String,
}
