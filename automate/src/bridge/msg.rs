use std::{collections::HashMap, time::Duration};

use anyhow::Error;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::Sender;

use crate::{
    comet::handler::SecretHeader,
    scheduler::types::{
        BaseJob, BundleOutput, JobAction, RunStatus, RuntimeAction, ScheduleStatus, ScheduleType,
    },
    ssh::AuthData,
};

pub enum MsgState {
    Completed(Value),
    Err(Error),
}

#[derive(Clone)]
pub struct TransactionMsg {
    pub tx: Sender<MsgState>,
    pub id: u64,
}

impl TransactionMsg {
    pub fn new(tx: Sender<MsgState>, id: u64) -> Self {
        Self { tx, id }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpReadDirParams {
    pub user: String,
    pub auth_data: AuthData,
    pub ip: String,
    pub port: u16,
    pub dir: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpUploadParams {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
    #[serde(with = "crate::bridge::base64_bytes")]
    pub data: Vec<u8>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpDownloadParams {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpRemoveParams {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub remove_type: String,
    pub filepath: String,
}

/// Starts a chunked upload.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpUploadStartParams {
    pub session_id: String,
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
    /// Total remote file size, used for verification only.
    pub total_size: u64,
}

/// A single upload chunk.
///
/// Every chunk carries its own connection parameters, so the agent does not have
/// to keep per session state to accept a chunk and one failing chunk cannot
/// affect the others.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpUploadChunkParams {
    pub session_id: String,
    pub seq: u64,
    /// Offset of this chunk inside the file.
    pub offset: u64,
    /// Chunk payload (base64 encoded).
    #[serde(with = "crate::bridge::base64_bytes")]
    pub data: Vec<u8>,
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
}

/// Finishes a chunked upload.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpUploadFinishParams {
    pub session_id: String,
    pub total_size: u64,
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
}

/// Download: query the remote file size.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpDownloadStatParams {
    /// Session id used to reuse an already established SSH/SFTP connection.
    pub session_id: String,
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
}

/// Download: fetch the requested range.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpDownloadChunkParams {
    pub session_id: String,
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub auth_data: AuthData,
    pub filepath: String,
    pub offset: u64,
    pub len: u32,
}

/// Ends a download session and releases the remote handle.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SftpDownloadFinishParams {
    pub session_id: String,
}


#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub enum MsgReqKind {
    DispatchJobRequest(DispatchJobParams),
    RuntimeActionRequest(RuntimeActionParams),
    PullJobRequest(Value),
    SftpReadDirRequest(SftpReadDirParams),
    SftpUploadRequest(SftpUploadParams),
    SftpDownloadRequest(SftpDownloadParams),
    SftpRemoveRequest(SftpRemoveParams),
    SftpUploadStartRequest(SftpUploadStartParams),
    SftpUploadChunkRequest(SftpUploadChunkParams),
    SftpUploadFinishRequest(SftpUploadFinishParams),
    SftpDownloadStatRequest(SftpDownloadStatParams),
    SftpDownloadChunkRequest(SftpDownloadChunkParams),
    SftpDownloadFinishRequest(SftpDownloadFinishParams),
    Auth(AuthParams),
    UpdateJobRequest(UpdateJobParams),
    HeartbeatRequest(HeartbeatParams),
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub enum MsgKind {
    Response(Value),
    Request(MsgReqKind),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Msg {
    pub id: u64,
    pub data: MsgKind,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct TimerExpr {
    pub timezone: String,
    pub expr: String,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct DispatchJobParams {
    pub base_job: BaseJob,
    pub schedule_id: String,
    pub instance_id: Option<String>,
    #[serde(default)]
    pub run_id: String,
    pub fields: Option<serde_json::Value>,
    pub timer_expr: Option<TimerExpr>,
    pub restart_interval: Option<Duration>,
    pub is_sync: bool,
    pub created_user: String,
    pub action: JobAction,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct RuntimeActionParams {
    pub eid: String,
    pub fields: Option<HashMap<String, serde_json::Value>>,
    pub is_sync: bool,
    pub created_user: String,
    pub action: RuntimeAction,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct HeartbeatParams {
    pub namespace: String,
    pub mac_addr: String,
    pub source_ip: String,
}

impl HeartbeatParams {
    pub fn get_endpoint(&self) -> String {
        if self.namespace != "" {
            format!("{}:{}", self.namespace, self.source_ip)
        } else {
            format!("{}", self.source_ip)
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct UpdateJobParams {
    pub schedule_id: String,
    pub schedule_type: Option<ScheduleType>,
    pub base_job: BaseJob,
    pub fields: Option<serde_json::Value>,
    pub instance_id: String,
    pub bind_ip: String,
    pub bind_namespace: String,
    pub run_status: Option<RunStatus>,
    pub schedule_status: Option<ScheduleStatus>,
    pub exit_code: Option<i32>,
    pub exit_status: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub created_user: String,
    pub bundle_output: Option<Vec<BundleOutputParams>>,
    pub run_id: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub prev_time: Option<DateTime<Utc>>,
    pub next_time: Option<DateTime<Utc>>,
    pub is_timeout: bool,
}

impl UpdateJobParams {
    pub fn bundle_output2json(bundle_output: Option<Vec<BundleOutputParams>>) -> Option<String> {
        match bundle_output {
            Some(v) => Some(
                serde_json::to_string(&v).expect("failed convert bundle_output to json string"),
            ),
            None => None,
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct BundleOutputParams {
    pub eid: String,
    pub exit_code: Option<i32>,
    pub exit_status: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

impl BundleOutputParams {
    pub fn parse(value: &BundleOutput) -> Option<Vec<BundleOutputParams>> {
        match value {
            BundleOutput::Output(_) => None,
            BundleOutput::Bundle(v) => Some(
                v.iter()
                    .map(|v| BundleOutputParams {
                        eid: v.0.to_owned(),
                        exit_code: {
                            if v.1.status.success() {
                                v.1.status.code()
                            } else {
                                v.1.status.code().or(Some(9))
                            }
                        },
                        exit_status: Some(v.1.status.to_string()),
                        stdout: Some(String::from_utf8_lossy(&v.1.stdout).to_string()),
                        stderr: Some(String::from_utf8_lossy(&v.1.stderr).to_string()),
                    })
                    .collect::<Vec<BundleOutputParams>>(),
            ),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct AuthParams {
    pub agent_ip: String,
    pub secret: String,
    pub is_initialized: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentOnlineParams {
    pub agent_ip: String,
    pub namespace: String,
    pub mac_addr: String,
    pub is_initialized: bool,
    pub secret_header: SecretHeader,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentOfflineParams {
    pub agent_ip: String,
    pub mac_addr: String,
}
