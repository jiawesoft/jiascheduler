use std::fmt::Display;

use sea_orm::{prelude::DateTimeLocal, FromQueryResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct UserInfo {
    pub username: String,     // 用户名
    pub nickname: String,     //昵称
    pub avatar: String,       // 头像
    pub email: String,        // 邮箱
    pub introduction: String, // 简介
    pub phone: String,
    pub gender: String,
    pub user_id: String,
    pub is_root: bool,
    pub role: String,
    pub role_id: u64,
    pub permissions: Vec<String>,
    pub created_time: String,
    pub updated_time: String,
}

#[derive(Clone, Serialize, Deserialize, Default, FromQueryResult)]
pub struct UserRecord {
    pub id: u64,
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub is_root: bool,
    pub role_id: u64,
    pub salt: String,
    pub password: String,
    pub avatar: String,
    pub email: String,
    pub phone: String,
    pub gender: String,
    pub role: Option<String>,
    pub introduction: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Clone, Serialize, Deserialize, Default, FromQueryResult)]
pub struct UserServer {
    pub id: u64,
    pub ip: String,
    pub instance_id: String,
    pub mac_addr: String,
    pub info: String,
    pub namespace: String,
    pub sys_user: Option<String>,
    pub ssh_port: Option<u16>,
    pub password: Option<String>,
    pub instance_group_id: Option<u64>,
    pub instance_group_name: Option<String>,
    pub tag_id: Option<u64>,
    pub status: i8,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Clone, Serialize, Deserialize, Default, FromQueryResult)]
pub struct UserRoleCount {
    pub role_id: u64,
    pub total: i64,
}

pub struct UserRoleCountList(pub Vec<UserRoleCount>);

impl UserRoleCountList {
    pub fn get_by_role_id(&self, role_id: u64) -> Option<&UserRoleCount> {
        self.0.iter().find(|&v| v.role_id == role_id)
    }
}

#[derive(Clone, Serialize, Deserialize, Default, FromQueryResult)]
pub struct InstanceRecord {
    pub id: u64,
    pub instance_id: String,
    pub ip: String,
    pub namespace: String,
    pub info: String,
    pub status: i8,
    pub sys_user: String,
    pub password: String,
    pub role_id: Option<u64>,
    pub role_name: Option<String>,
    pub instance_group: Option<String>,
    pub instance_group_id: u64,
    pub ssh_port: u16,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct VersionRecord {
    pub name: String,
    pub info: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Permission {
    pub name: &'static str,
    pub object: &'static str,
    pub action: &'static str,
}

impl Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}_{}", self.object, self.action).as_str())
    }
}

#[derive(Clone, Serialize, Deserialize, Default, FromQueryResult)]
pub struct TeamMemberCount {
    pub team_id: u64,
    pub total: i64,
}

pub struct TeamMemberCountList(pub Vec<TeamMemberCount>);

impl TeamMemberCountList {
    pub fn get_by_team_id(&self, team_id: u64) -> Option<&TeamMemberCount> {
        self.0.iter().find(|&v| v.team_id == team_id)
    }
}

#[derive(Clone, Serialize, Deserialize, Default, FromQueryResult)]
pub struct TeamRecord {
    pub id: u64,
    pub name: String,
    pub info: String,
    pub is_admin: Option<bool>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Job,
    Instance,
}

impl Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::Job => write!(f, "job"),
            ResourceType::Instance => write!(f, "instance"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, FromQueryResult)]
pub struct TagCount {
    pub tag_id: u64,
    pub tag_name: String,
    pub total: i64,
}
