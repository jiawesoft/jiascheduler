use poem_openapi::OpenApi;
use sea_orm::{ActiveValue::NotSet, Set};

use crate::api_response;
use crate::entity::instance_group;
use crate::{
    entity::instance, error::NoPermission, local_time, logic, response::ApiStdResponse, return_ok,
    AppState,
};
use poem::{session::Session, web::Data, Result};
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
        pub ip: String,
        pub namespace: String,
        pub instance_group: String,
        pub sys_user: String,
        pub info: String,
        pub status: i8,
        pub role_id: u64,
        pub role_name: String,
        pub instance_group_id: u64,
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
    pub struct QueryUserServerResp {
        pub total: u64,
        pub list: Vec<UserServerRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct UserServerRecord {
        pub id: u64,
        pub ip: String,
        pub namespace: String,
        pub instance_group_id: u64,
        pub instance_group: String,
        pub status: i8,
        pub info: String,
        pub tag_key: Option<String>,
        pub tag_val: Option<String>,
        pub created_time: String,
        pub updated_time: String,
    }

    pub fn default_instance_status() -> u8 {
        1
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveInstanceReq {
        pub id: Option<u64>,
        pub ip: String,
        pub namespace: String,
        pub instance_group_id: Option<u64>,
        pub info: Option<String>,
        pub status: i8,
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
}

pub struct InstanceApi;

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
                ip: v.ip,
                role_id: v.role_id.unwrap_or_default(),
                role_name: v.role_name.unwrap_or_default(),
                instance_group: v.instance_group.unwrap_or_default(),
                instance_group_id: v.instance_group_id,
                namespace: v.namespace,
                status: v.status,
                updated_time: local_time!(v.updated_time),
                sys_user: v.sys_user,
                info: v.info,
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

    #[oai(path = "/user-server-list", method = "get")]
    pub async fn user_server(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,

        Query(ip): Query<Option<String>>,
        Query(instance_group_id): Query<Option<u64>>,
        Query(tag_id): Query<Option<Vec<u64>>>,
        Query(status): Query<Option<u8>>,

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
    ) -> Result<ApiStdResponse<types::QueryUserServerResp>> {
        let svc = state.service();
        let user_id = user_info.user_id.clone();

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
            .map(|v| types::UserServerRecord {
                id: v.id,
                ip: v.ip,
                info: v.info,
                tag_key: v.tag_key,
                tag_val: v.tag_val,
                namespace: v.namespace,
                instance_group_id: v.instance_group_id.unwrap_or_default(),
                instance_group: v.instance_group_name.unwrap_or_default(),
                status: v.status,
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
                sys_user: req
                    .sys_user
                    .filter(|v| v.trim() != "")
                    .map_or(NotSet, |v| Set(v)),
                password,
                ssh_port: req.ssh_port.filter(|&v| v != 0).map_or(NotSet, |v| Set(v)),
                ..Default::default()
            })
            .await?;
        return_ok!(types::SaveInstanceResp { result: 0 })
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
                    .query_admin_server(None, Some(1), None, 0, 1)
                    .await?
                    .1,
                svc.instance
                    .query_admin_server(None, Some(0), None, 0, 1)
                    .await?
                    .1,
            )
        } else {
            (
                svc.instance
                    .query_user_server(user_info.user_id.clone(), None, Some(1), None, 0, 1)
                    .await?
                    .1,
                svc.instance
                    .query_user_server(user_info.user_id.clone(), None, Some(0), None, 0, 1)
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
