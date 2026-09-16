use poem_openapi::OpenApi;
use sea_orm::{ActiveValue::NotSet, Set};

use crate::api_response;
use crate::{
    AppState, entity::instance, error::NoPermission, local_time, logic, response::ApiStdResponse,
    return_ok,
};
use entity::instance_group;
use poem::{Result, session::Session, web::Data};
use poem_openapi::param::Query;
use poem_openapi::payload::Json;

pub mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object, Serialize, Default)]
    pub struct QueryInstanceResp {
        pub total: u64,
        pub list: Vec<InstanceRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct InstanceRecord {
        pub id: u64,
        pub instance_id: String,
        pub ip: String,
        pub namespace: String,
        pub instance_group: String,
        pub sys_user: String,
        pub sys_users: Vec<SysUser>,
        pub info: String,
        pub status: i8,
        pub role_id: u64,
        pub role_name: String,
        pub instance_group_id: u64,
        pub ssh_port: Option<u16>,
        pub ssh_user: Option<String>,
        pub ssh_auth_type: Option<String>,
        pub created_time: String,
        pub updated_time: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct GrantedUserReq {
        pub user_id: Vec<String>,
        pub instance_ids: Option<Vec<String>>,
        pub instance_group_ids: Option<Vec<i64>>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct GrantedUserResp {}

    #[derive(Object, Serialize, Default)]
    pub struct UserServerReq {
        pub ips: Option<Vec<String>>,
        pub instance_ids: Option<Vec<String>>,
        pub instance_group_id: Option<u64>,
        pub tag_id: Option<Vec<u64>>,
        pub status: Option<u8>,

        #[oai(
            default = "crate::api::default_page_size",
            validator(maximum(value = "10000"))
        )]
        pub page_size: u64,
        #[oai(
            default = "crate::api::default_page",
            validator(maximum(value = "10000"))
        )]
        pub page: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryUserServerResp {
        pub total: u64,
        pub list: Vec<UserServerRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct Tag {
        pub tag_id: u64,
        pub tag_name: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct UserServerRecord {
        pub instance_id: String,
        pub ip: String,
        pub namespace: String,
        pub instance_group_id: u64,
        pub instance_group: String,
        pub status: i8,
        pub info: String,
        pub tags: Option<Vec<Tag>>,
        pub ssh_port: Option<u16>,
        pub sys_user: Option<String>,
        pub sys_users: Vec<SysUser>,
        pub ssh_user: Option<String>,
        pub ssh_auth_type: Option<String>,
        pub created_time: String,
        pub updated_time: String,
    }

    pub fn default_instance_status() -> u8 {
        1
    }

    /// An ssh login user of an instance.
    ///
    /// It is persisted in the `sys_users` json column of the `instance` table.
    /// Sensitive data (password / key content) is never returned by the API,
    /// only the `auth_type`, `key_path` and `is_default` are readable; when
    /// saving, leaving `password`/`key_content` empty keeps the stored value.
    #[derive(Object, Serialize, Deserialize, Clone, Debug, Default)]
    pub struct SysUser {
        pub auth_type: String,
        pub username: String,
        pub key_path: Option<String>,
        pub key_content: Option<String>,
        pub password: Option<String>,
        /// Whether this user is the default login user of the instance.
        #[oai(default)]
        pub is_default: bool,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceReq {
        pub id: Option<u64>,
        pub ip: String,
        pub namespace: String,
        pub instance_group_id: Option<u64>,
        pub info: Option<String>,
        pub status: i8,
        pub sys_users: Vec<SysUser>,
        pub sys_user: Option<String>,
        pub password: Option<String>,
        pub ssh_port: Option<u16>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceResp {
        pub result: u32,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceGroupReq {
        pub id: Option<u64>,
        pub name: String,
        pub info: String,
        pub instance_ids: Option<Vec<u64>>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceGroupResp {
        pub result: u32,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryInstanceGroupResp {
        pub total: u64,
        pub list: Vec<InstanceGroupRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct InstanceGroupRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub created_time: String,
        pub updated_time: String,
        pub created_user: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct DeleteInstanceGroupReq {
        pub id: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct DeleteInstanceGroupResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct GetInstanceStatsResp {
        pub instance_online_num: u64,
        pub instance_offline_num: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceStatusReq {
        pub status: bool,
        pub instance_ids: Vec<u64>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceStatusResp {
        pub result: u64,
    }
}

pub struct InstanceApi;

pub(crate) fn ssh_auth_type_str(auth: &entity::instance::SshAuthData) -> String {
    match auth {
        entity::instance::SshAuthData::Password(_) => "password".to_string(),
        entity::instance::SshAuthData::KeyPath(_) => "key_path".to_string(),
        entity::instance::SshAuthData::KeyContent(_) => "key_content".to_string(),
    }
}

/// Resolve the user name of one of the instance login users.
///
/// The explicitly selected user wins, otherwise the default login user of the
/// instance (the `sys_user` column) is used.
pub(crate) fn pick_sys_user(
    instance_record: &logic::types::UserServer,
    selected: Option<&str>,
) -> Option<String> {
    selected
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| instance_record.sys_user.clone().filter(|v| v.trim() != ""))
}

/// Convert the auth data stored on an instance into the wire format.
pub(crate) fn to_wire_auth(auth: &entity::instance::SshAuthData) -> automate::ssh::AuthData {
    match auth {
        entity::instance::SshAuthData::Password(v) => automate::ssh::AuthData::Password(v.clone()),
        entity::instance::SshAuthData::KeyPath(v) => automate::ssh::AuthData::KeyPath(v.clone()),
        entity::instance::SshAuthData::KeyContent(v) => {
            automate::ssh::AuthData::KeyContent(v.clone())
        }
    }
}

/// Decrypt a stored secret (password or key content).
///
/// Instances that were configured before key content was encrypted still hold
/// the plaintext key in the column, so the raw value is accepted as a fallback
/// when decryption fails.
pub(crate) fn decrypt_secret(state: &AppState, value: String) -> anyhow::Result<String> {
    match state.decrypt(value.clone()) {
        Ok(v) => Ok(v),
        Err(_) => Ok(value),
    }
}

/// Resolve the ssh auth data of one configured login user.
///
/// Returns `None` when the user is not part of the `sys_users` list, so the
/// caller can fall back to the legacy columns or the agent-reported account.
pub(crate) fn resolve_user_auth_of(
    sys_users: &Option<serde_json::Value>,
    user: &str,
    decrypt: impl Fn(String) -> anyhow::Result<String>,
) -> anyhow::Result<Option<(String, automate::ssh::AuthData)>> {
    use entity::instance::SshAuthData;

    let Some(found) = parse_sys_users(sys_users)
        .into_iter()
        .find(|v| v.user == user)
    else {
        return Ok(None);
    };

    let auth_data = match found.auth_data {
        Some(SshAuthData::Password(p)) => automate::ssh::AuthData::Password(decrypt(p)?),
        // the inline key content is encrypted on the instance as well
        Some(SshAuthData::KeyContent(v)) => automate::ssh::AuthData::KeyContent(decrypt(v)?),
        Some(v) => to_wire_auth(&v),
        None => anyhow::bail!("Notice: the login user {user} has no auth data"),
    };

    Ok(Some((found.user, auth_data)))
}

/// Resolve the ssh auth data of an instance login user.
///
/// This is shared by the terminal login and the sftp file manager so both use
/// the very same account. The resolution order is:
///
/// 1. an entry of the instance `sys_users` list
/// 2. the account reported by the agent (used by the terminal when the agent
///    account is the effective login account)
/// 3. the legacy `sys_user` / `password` columns
pub(crate) fn resolve_user_auth(
    state: &AppState,
    instance_record: &logic::types::UserServer,
    selected: Option<&str>,
) -> anyhow::Result<(String, automate::ssh::AuthData)> {
    let user = pick_sys_user(instance_record, selected).unwrap_or_default();

    if let Some(found) =
        resolve_user_auth_of(&instance_record.sys_users, &user, |v| decrypt_secret(state, v))?
    {
        return Ok(found);
    }

    // the agent reported account, mirroring the terminal resolution
    if let Some(register_data) = instance_record.register_data.as_ref() {
        let agent_user = register_data.ssh_user.clone().filter(|v| v.trim() != "");
        if agent_user.as_deref() == Some(user.as_str()) {
            if let Some(auth_data) = register_data.auth_data.as_ref() {
                return Ok((user, to_wire_auth(auth_data)));
            }
        }
    }

    if user.trim() == "" {
        anyhow::bail!("Notice: no login user configured for this instance");
    }

    let password = instance_record
        .password
        .clone()
        .filter(|v| v != "")
        .ok_or_else(|| anyhow::anyhow!("Notice: no password configured for {user}"))?;

    Ok((
        user,
        automate::ssh::AuthData::Password(state.decrypt(password)?),
    ))
}

/// Resolve the ssh port used to reach an instance.
pub(crate) fn pick_ssh_port(
    instance_record: &logic::types::UserServer,
    selected: Option<u16>,
) -> u16 {
    selected
        .or(instance_record.ssh_port)
        .filter(|&v| v != 0)
        .unwrap_or(22)
}

/// Parse the `sys_users` json column into typed login users.
pub(crate) fn parse_sys_users(
    sys_users: &Option<serde_json::Value>,
) -> Vec<entity::instance::SysUser> {
    sys_users
        .as_ref()
        .and_then(|v| serde_json::from_value::<Vec<entity::instance::SysUser>>(v.clone()).ok())
        .unwrap_or_default()
}

/// Convert a stored login user into an api record.
///
/// Password and key content are secrets that must never leave the server, so
/// only the auth type is returned. `KeyPath` is only meaningful for the agent
/// side, it is not editable from the console.
pub(crate) fn sys_user_to_record(
    user: entity::instance::SysUser,
    default_user: Option<&str>,
) -> types::SysUser {
    let auth_type = user
        .auth_data
        .as_ref()
        .map(ssh_auth_type_str)
        .unwrap_or_else(|| "password".to_string());

    types::SysUser {
        auth_type,
        is_default: user.is_default || default_user.is_some_and(|v| v == user.user),
        username: user.user,
        key_path: None,
        key_content: None,
        password: None,
    }
}

/// Build a stored login user from the api request, encrypting passwords.
///
/// When the plaintext password/key content is left empty the value already
/// stored for the same user is kept, which matches the behaviour of the
/// instance password field.
fn build_sys_user(
    state: &AppState,
    req: &types::SysUser,
    old: Option<&entity::instance::SysUser>,
    is_default: bool,
) -> Result<Option<entity::instance::SysUser>, poem::Error> {
    let user = req.username.trim().to_string();
    if user.is_empty() {
        return Ok(None);
    }

    let old_auth = old.and_then(|v| v.auth_data.clone());
    let old_content = || match old_auth.clone() {
        Some(entity::instance::SshAuthData::KeyContent(v)) => Some(v),
        _ => None,
    };
    let old_key_path = || match old_auth.clone() {
        Some(entity::instance::SshAuthData::KeyPath(v)) => Some(v),
        _ => None,
    };

    let auth_data = match req.auth_type.as_str() {
        // KeyPath is only meaningful for the account reported by the agent, it
        // can no longer be configured from the console
        "key_path" => {
            return Err(anyhow::anyhow!(
                "Notice: the key file path is only supported for the account reported by the agent"
            )
            .into());
        }
        "key_content" => {
            // the key content is a secret as well, it is encrypted with the
            // same key as the password before being stored
            let content = match req.key_content.clone().filter(|v| v.trim() != "") {
                Some(v) => state.encrypt(v)?,
                None => old_content().unwrap_or_default(),
            };
            entity::instance::SshAuthData::KeyContent(content)
        }
        _ => {
            let new_password = req.password.clone().filter(|v| v.trim() != "");
            // a legacy user that was configured with a key file path keeps it
            // when the client saves the record without entering a new secret
            if new_password.is_none() {
                if let Some(content) = old_content() {
                    entity::instance::SshAuthData::KeyContent(content)
                } else if let Some(path) = old_key_path() {
                    entity::instance::SshAuthData::KeyPath(path)
                } else {
                    entity::instance::SshAuthData::Password(String::new())
                }
            } else {
                entity::instance::SshAuthData::Password(
                    state.encrypt(new_password.unwrap_or_default())?,
                )
            }
        }
    };

    Ok(Some(entity::instance::SysUser {
        user,
        auth_data: Some(auth_data),
        is_default,
    }))
}

/// Convert the requested login users into the value stored in `sys_users`.
///
/// The returned string is the default login user, which is written to the
/// `sys_user` column so that both places always agree.
pub(crate) fn build_sys_users(
    state: &AppState,
    req: &[types::SysUser],
    old: &[entity::instance::SysUser],
    current_default: Option<&str>,
) -> Result<(Vec<entity::instance::SysUser>, String), poem::Error> {
    let names: Vec<String> = req
        .iter()
        .map(|v| v.username.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();

    // explicit choice first, then the user already stored as default, finally
    // fall back to the first configured user
    let default_user = req
        .iter()
        .find(|v| v.is_default)
        .map(|v| v.username.trim().to_string())
        .filter(|v| names.contains(v))
        .or_else(|| {
            current_default
                .map(|v| v.to_string())
                .filter(|v| names.contains(v))
        })
        .or_else(|| names.first().cloned())
        .unwrap_or_default();

    let mut users = Vec::new();
    for item in req {
        let user = item.username.trim();
        if user.is_empty() {
            continue;
        }
        let old_user = old.iter().find(|v| v.user == user);
        let is_default = user == default_user;
        if let Some(v) = build_sys_user(state, item, old_user, is_default)? {
            users.push(v);
        }
    }

    Ok((users, default_user))
}

#[OpenApi(prefix_path = "/instance", tag = super::Tag::Instance)]
impl InstanceApi {
    #[oai(path = "/list", method = "get")]
    pub async fn query_instance(
        &self,
        state: Data<&AppState>,
        _session: &Session,

        Query(ip): Query<Option<String>>,
        Query(status): Query<Option<u8>>,
        Query(role_id): Query<Option<u64>>,
        Query(ignore_role_id): Query<Option<u64>>,
        #[oai(
            default = "crate::api::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        #[oai(
            default = "crate::api::default_page",
            validator(maximum(value = "10000"))
        )]
        Query(page): Query<u64>,
        user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::QueryInstanceResp>> {
        let svc = state.service();
        if !state.can_manage_instance(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }

        let ret = match role_id {
            Some(role_id) if role_id > 0 && !svc.role.is_admin(role_id).await? => {
                svc.instance
                    .query_instance_by_role_id(
                        ip.filter(|v| v != ""),
                        status,
                        role_id,
                        ignore_role_id.filter(|&v| v != 0),
                        page - 1,
                        page_size,
                    )
                    .await?
            }
            _ => {
                svc.instance
                    .query_instance(
                        ip.filter(|v| v != ""),
                        status,
                        ignore_role_id.filter(|&v| v != 0),
                        page - 1,
                        page_size,
                    )
                    .await?
            }
        };

        let list: Vec<types::InstanceRecord> = ret
            .0
            .into_iter()
            .map(|v| types::InstanceRecord {
                id: v.id,
                instance_id: v.instance_id,
                ip: v.ip,
                role_id: v.role_id.unwrap_or_default(),
                role_name: v.role_name.unwrap_or_default(),
                instance_group: v.instance_group.unwrap_or_default(),
                instance_group_id: v.instance_group_id,
                namespace: v.namespace,
                status: v.status,
                updated_time: local_time!(v.updated_time),
                sys_users: parse_sys_users(&v.sys_users)
                    .into_iter()
                    .map(|u| sys_user_to_record(u, Some(&v.sys_user)))
                    .collect(),
                sys_user: v.sys_user,
                info: v.info,
                ssh_port: Some(v.ssh_port),
                ssh_user: v.register_data.as_ref().and_then(|r| r.ssh_user.clone()),
                ssh_auth_type: v
                    .register_data
                    .as_ref()
                    .and_then(|r| r.auth_data.as_ref().map(ssh_auth_type_str)),
                created_time: local_time!(v.created_time),
            })
            .collect();
        return_ok!(types::QueryInstanceResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/grant", method = "post")]
    pub async fn grant(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::GrantedUserReq>,
    ) -> Result<ApiStdResponse<types::GrantedUserResp>> {
        let svc = state.service();
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }
        svc.instance
            .granted_user(req.user_id, req.instance_ids, req.instance_group_ids)
            .await?;
        return_ok!(types::GrantedUserResp {})
    }

    #[oai(path = "/user-server-list", method = "post")]
    pub async fn user_server(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::UserServerReq>,
    ) -> Result<ApiStdResponse<types::QueryUserServerResp>> {
        let svc = state.service();
        let user_id = user_info.user_id.clone();

        let can_manage_instance = state.can_manage_instance(&user_id).await?;

        let (list, total) = match req.tag_id {
            Some(tag_id) if tag_id.len() > 0 => {
                let query_result = svc
                    .instance
                    .query_server_by_tag(
                        None.or_else(|| {
                            if can_manage_instance {
                                Some(user_id)
                            } else {
                                None
                            }
                        }),
                        req.instance_group_id.filter(|&v| v != 0),
                        req.status,
                        req.ips.clone(),
                        req.instance_ids.clone(),
                        Some(tag_id),
                        req.page - 1,
                        req.page_size,
                    )
                    .await?;

                (query_result.0, query_result.1)
            }
            _ if can_manage_instance => {
                let query_result = svc
                    .instance
                    .query_admin_server(
                        req.instance_ids.clone(),
                        req.instance_group_id.filter(|&v| v != 0),
                        req.status,
                        req.ips.clone(),
                        req.page - 1,
                        req.page_size,
                    )
                    .await?;
                (query_result.0, query_result.1)
            }
            _ => {
                let query_result = svc
                    .instance
                    .query_user_server(
                        user_id,
                        req.instance_ids.clone(),
                        req.instance_group_id.filter(|&v| v != 0),
                        req.status,
                        req.ips.clone(),
                        req.page - 1,
                        req.page_size,
                    )
                    .await?;

                (query_result.0, query_result.1)
            }
        };

        let list = list
            .into_iter()
            .map(|v| types::UserServerRecord {
                instance_id: v.instance_id,
                ip: v.ip,
                info: v.info,
                tags: None,
                namespace: v.namespace,
                instance_group_id: v.instance_group_id.unwrap_or_default(),
                instance_group: v.instance_group_name.unwrap_or_default(),
                status: v.status,
                ssh_port: v.ssh_port,
                sys_users: parse_sys_users(&v.sys_users)
                    .into_iter()
                    .map(|u| sys_user_to_record(u, v.sys_user.as_deref()))
                    .collect(),
                sys_user: v.sys_user,
                ssh_user: v.register_data.as_ref().and_then(|r| r.ssh_user.clone()),
                ssh_auth_type: v
                    .register_data
                    .as_ref()
                    .and_then(|r| r.auth_data.as_ref().map(ssh_auth_type_str)),
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();

        return_ok!(types::QueryUserServerResp { list, total })
    }

    #[oai(path = "/save", method = "post")]
    pub async fn save_instance(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveInstanceReq>,
    ) -> Result<ApiStdResponse<types::SaveInstanceResp>> {
        let svc = state.service();
        if !state.can_manage_instance(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }

        // load the current record so that secrets left blank by the client and
        // the user marked as default are preserved
        let current = match req.id.filter(|&v| v != 0) {
            Some(id) => svc
                .instance
                .find_by_id(id)
                .await?
                .map(|v| (parse_sys_users(&v.sys_users), v.sys_user)),
            None => None,
        };
        let is_update = current.is_some();
        let (old_sys_users, current_default) = current.unwrap_or_default();

        let (sys_users, default_user) = build_sys_users(
            &state,
            &req.sys_users,
            &old_sys_users,
            Some(current_default.as_str()).filter(|v| !v.is_empty()),
        )?;

        // a request that omits the field keeps the stored list, an explicit
        // empty list clears it
        let sys_users = if sys_users.is_empty() && req.sys_users.is_empty() && is_update {
            NotSet
        } else if sys_users.is_empty() {
            Set(None)
        } else {
            Set(Some(
                serde_json::to_value(&sys_users).map_err(anyhow::Error::from)?,
            ))
        };

        let password = req
            .password
            .clone()
            .filter(|v| v.trim() != "")
            .map(|v| state.encrypt(v))
            .transpose()?
            .map_or(NotSet, |v| Set(v));

        svc.instance
            .save_instance(instance::ActiveModel {
                id: req.id.filter(|&v| v != 0).map_or(NotSet, |v| Set(v)),
                ip: Set(req.ip),
                namespace: Set(req.namespace),
                instance_group_id: req.instance_group_id.map_or(NotSet, |v| Set(v)),
                info: req.info.map_or(NotSet, |v| Set(v)),
                status: Set(req.status),
                // the default login user kept in sync with sys_users; a legacy
                // client that only sends sys_user is still honoured
                sys_user: if default_user.trim() == "" {
                    req.sys_user
                        .clone()
                        .filter(|v| v.trim() != "")
                        .map_or(NotSet, |v| Set(v))
                } else {
                    Set(default_user.clone())
                },
                sys_users,
                password,
                ssh_port: req.ssh_port.filter(|&v| v != 0).map_or(NotSet, |v| Set(v)),
                ..Default::default()
            })
            .await?;
        return_ok!(types::SaveInstanceResp { result: 0 })
    }

    #[oai(path = "/set_status", method = "post")]
    pub async fn set_instance_status(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveInstanceStatusReq>,
    ) -> api_response!(types::SaveInstanceStatusResp) {
        if !state.can_manage_instance(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }
        let result = state
            .service()
            .instance
            .set_status(state.clone(), &user_info, req.instance_ids, req.status)
            .await?;

        return_ok!(types::SaveInstanceStatusResp { result })
    }

    #[oai(path = "/group/save", method = "post")]
    pub async fn save_group(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveInstanceGroupReq>,
    ) -> Result<ApiStdResponse<types::SaveInstanceGroupResp>> {
        let svc = state.service();
        if !state.can_manage_instance(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }
        svc.instance
            .save_group(instance_group::ActiveModel {
                id: req.id.filter(|&v| v != 0).map_or(NotSet, |v| Set(v)),
                name: Set(req.name),
                info: Set(req.info),
                created_user: Set(user_info.username.to_string()),
                ..Default::default()
            })
            .await?;
        return_ok!(types::SaveInstanceGroupResp { result: 0 })
    }

    #[oai(path = "/group/list", method = "get")]
    pub async fn query_group(
        &self,
        state: Data<&AppState>,
        _session: &Session,

        Query(name): Query<Option<String>>,
        Query(role_id): Query<Option<u64>>,
        Query(ignore_role_id): Query<Option<u64>>,

        #[oai(
            default = "crate::api::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        #[oai(
            default = "crate::api::default_page",
            validator(maximum(value = "10000"))
        )]
        Query(page): Query<u64>,
        user_info: Data<&logic::types::UserInfo>,
    ) -> api_response!(types::QueryInstanceGroupResp) {
        let svc = state.service();
        if !state.can_manage_instance(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }

        let ret = if let Some(role_id) = role_id {
            svc.instance
                .query_group_by_role_id(
                    name.filter(|v| v != ""),
                    role_id,
                    ignore_role_id.filter(|&v| v != 0),
                    page - 1,
                    page_size,
                )
                .await?
        } else {
            svc.instance
                .query_group(
                    name.filter(|v| v != ""),
                    ignore_role_id.filter(|&v| v != 0),
                    page - 1,
                    page_size,
                )
                .await?
        };

        let list = ret
            .0
            .into_iter()
            .map(|v| types::InstanceGroupRecord {
                id: v.id,
                name: v.name,
                info: v.info,
                created_user: v.created_user,
                updated_time: local_time!(v.updated_time),
                created_time: local_time!(v.created_time),
            })
            .collect();
        return_ok!(types::QueryInstanceGroupResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/group/delete", method = "post")]
    pub async fn delete_group(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::DeleteInstanceGroupReq>,
    ) -> api_response!(types::DeleteInstanceGroupResp) {
        let svc = state.service();
        if !state.can_manage_instance(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }
        let ret = svc.instance.delete_group(req.id).await?;
        return_ok!(types::DeleteInstanceGroupResp { result: ret })
    }

    #[oai(path = "/instance-stats", method = "post")]
    pub async fn get_instance_stats(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::GetInstanceStatsResp>> {
        let svc = state.service();
        let can_manage_instance = state.can_manage_instance(&user_info.user_id).await?;
        let (online_num, offline_num) = if can_manage_instance {
            (
                svc.instance
                    .query_admin_server(None, None, Some(1), None, 0, 1)
                    .await?
                    .1,
                svc.instance
                    .query_admin_server(None, None, Some(0), None, 0, 1)
                    .await?
                    .1,
            )
        } else {
            (
                svc.instance
                    .query_user_server(user_info.user_id.clone(), None, None, Some(1), None, 0, 1)
                    .await?
                    .1,
                svc.instance
                    .query_user_server(user_info.user_id.clone(), None, None, Some(0), None, 0, 1)
                    .await?
                    .1,
            )
        };
        return_ok!(types::GetInstanceStatsResp {
            instance_online_num: online_num,
            instance_offline_num: offline_num,
        });
    }
}
