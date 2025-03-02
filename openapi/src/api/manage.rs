use crate::{
    entity::user,
    error::NoPermission,
    local_time,
    logic::{self, role::PERMISSIONS, user::UserLogic},
    response::ApiStdResponse,
    return_err, return_ok, AppState,
};

use anyhow::anyhow;
use poem::{session::Session, web::Data, Result};
use poem_openapi::{param::Query, payload::Json, OpenApi};
use sea_orm::{ActiveValue::NotSet, Set};
use types::PermissionRecord;
pub struct ManageApi;

pub mod types {

    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object, Serialize, Deserialize)]
    pub struct SetRoleResp {
        pub affected: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SetRoleReq {
        pub user_id: String,
        pub role_id: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct AdminUpdateUserInfoReq {
        pub password: Option<String>,
        pub nickname: String, //昵称
        pub avatar: String,   // 头像
        pub email: String,    // 邮箱
        pub gender: String,
        pub introduction: String, // 简介
        pub phone: String,
        pub user_id: String,
        pub role_id: Option<u64>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct AdminUpdateInfoResp {
        pub affected: u64,
    }
    #[derive(Object, Serialize, Default)]
    pub struct AllPermissionResp {
        pub list: Vec<PermissionRecord>,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct PermissionRecord {
        pub name: String,
        pub object: String,
        pub action: String,
        pub key: String,
    }
}

#[OpenApi(prefix_path = "/admin", tag = super::Tag::Admin)]
impl ManageApi {
    #[oai(path = "/user/set-role", method = "post")]
    pub async fn set_role(
        &self,
        user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        state: Data<&AppState>,
        Json(req): Json<types::SetRoleReq>,
    ) -> Result<ApiStdResponse<types::SetRoleResp>> {
        let ok = state.can_manage_user(&user_info.user_id).await?;

        if !ok {
            return Err(NoPermission().into());
        }

        let affected = state
            .service()
            .user
            .set_role(req.user_id, req.role_id)
            .await?;

        return_ok!(types::SetRoleResp { affected })
    }

    #[oai(path = "/user/update-info", method = "post")]
    pub async fn update_info(
        &self,
        sess: &Session,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        // #[oai(name = "TOKEN")] _token: Header<String>,
        Json(req): Json<types::AdminUpdateUserInfoReq>,
    ) -> Result<ApiStdResponse<types::AdminUpdateInfoResp>> {
        let svc = state.service();
        let state_clone = state.clone();

        let ok = state.can_manage_user(&user_info.user_id).await?;

        if !ok {
            return Err(NoPermission().into());
        }

        let affected = svc
            .user
            .update_user(
                user::ActiveModel {
                    user_id: Set(req.user_id),
                    nickname: Set(req.nickname),
                    avatar: Set(req.avatar),
                    email: Set(req.email),
                    phone: Set(req.phone),
                    gender: Set(req.gender),
                    role_id: req.role_id.map_or(NotSet, |v| Set(v)),
                    password: req.password.filter(|v| v != "").map_or(NotSet, |v| Set(v)),
                    introduction: Set(req.introduction),
                    ..Default::default()
                },
                |user_id: String, role: String| async move {
                    if role == "0" {
                        state_clone.delete_role_for_user(&user_id).await?;
                    } else {
                        state_clone.set_role_for_user(&user_id, &role).await?;
                    }

                    Ok(())
                },
            )
            .await?;
        state.load_policy().await?;

        match svc.user.get_user(Some(&user_info.username), None).await? {
            Some(record) => {
                let permissions = state.get_permissions_for_user(&record.user_id).await?;
                sess.set(
                    UserLogic::SESS_KEY,
                    logic::types::UserInfo {
                        username: record.username,
                        nickname: record.nickname,
                        avatar: record.avatar,
                        email: record.email,
                        is_root: record.is_root,
                        role_id: record.role_id,
                        introduction: record.introduction,
                        phone: record.phone,
                        created_time: local_time!(record.created_time),
                        updated_time: local_time!(record.updated_time),
                        user_id: record.user_id,
                        gender: record.gender,
                        permissions,
                        role: record.role.unwrap_or_default(),
                    },
                );
            }
            None => (),
        }

        return_ok!(types::AdminUpdateInfoResp { affected });
    }

    #[oai(path = "/instance/user-server-list", method = "get")]
    pub async fn admin_user_server(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,

        Query(ip): Query<Option<String>>,
        Query(instance_id): Query<Option<String>>,
        Query(instance_group_id): Query<Option<u64>>,
        Query(tag_id): Query<Option<Vec<u64>>>,
        Query(status): Query<Option<u8>>,

        /// only for admin role
        Query(user_id): Query<String>,

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
    ) -> Result<ApiStdResponse<super::instance::types::QueryUserServerResp>> {
        let is_allow = state.can_manage_user(&user_info.user_id).await?;

        if !is_allow {
            return_err!("no permission");
        }

        let svc = state.service();

        let search_user = svc
            .user
            .get_user(None, Some(&user_id))
            .await?
            .ok_or(anyhow!("invalid user_id"))?;

        let user_id = search_user.user_id;

        let can_manage_instance = state.can_manage_instance(&user_id).await?;

        let (list, total) = match tag_id {
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
                        instance_group_id.filter(|&v| v != 0),
                        status,
                        ip.filter(|v| v != ""),
                        Some(tag_id),
                        page - 1,
                        page_size,
                    )
                    .await?;

                (query_result.0, query_result.1)
            }
            _ if can_manage_instance => {
                let query_result = svc
                    .instance
                    .query_admin_server(
                        instance_id.filter(|v| v != ""),
                        instance_group_id.filter(|&v| v != 0),
                        status,
                        ip.filter(|v| v != ""),
                        page - 1,
                        page_size,
                    )
                    .await?;
                (query_result.0, query_result.1)
            }
            _ => {
                let query_result = svc
                    .instance
                    .query_user_server(
                        user_id,
                        instance_id.filter(|v| v != ""),
                        instance_group_id.filter(|&v| v != 0),
                        status,
                        ip.filter(|v| v != ""),
                        page - 1,
                        page_size,
                    )
                    .await?;

                (query_result.0, query_result.1)
            }
        };

        let list = list
            .into_iter()
            .map(|v| super::instance::types::UserServerRecord {
                ip: v.ip,
                info: v.info,
                instance_id: v.instance_id,
                tags: None,
                namespace: v.namespace,
                instance_group_id: v.instance_group_id.unwrap_or_default(),
                instance_group: v.instance_group_name.unwrap_or_default(),
                status: v.status,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();

        return_ok!(super::instance::types::QueryUserServerResp { list, total })
    }

    #[oai(path = "/permission/all", method = "get")]
    pub async fn all_permission(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::AllPermissionResp>> {
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }

        let permission_record = PERMISSIONS
            .iter()
            .map(|v| PermissionRecord {
                name: v.name.to_string(),
                object: v.object.to_string(),
                action: v.action.to_string(),
                key: v.to_string(),
            })
            .collect();

        return_ok!(types::AllPermissionResp {
            list: permission_record
        });
    }
}
