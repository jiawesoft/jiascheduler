use automate::scheduler::types::SshConnectOption;
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use serde_repr::*;
use service::logic::types::UserServer;

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
    pub cols: u32,
    pub rows: u32,
}

#[derive(Object, Serialize, Deserialize)]
pub struct CreateSessionReq {
    pub instance_id: String,
    pub user_source: Option<String>,
    pub auth_type: Option<String>,
    pub password: Option<String>,
    pub key_content: Option<String>,
    pub port: Option<u16>,
    pub sys_user: Option<String>,
}

#[derive(Object, Serialize, Deserialize)]
pub struct CreateSessionResp {
    pub session_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct TerminalSession {
    pub connect_opts: SshConnectOption,
    pub created_username: String,
    pub instance: UserServer,
}

#[derive(Object, Serialize, Deserialize)]
pub struct GetTerminalSessionResp {
    pub connect_opts: serde_json::Value,
    pub created_username: String,
    pub instance: serde_json::Value,
}
