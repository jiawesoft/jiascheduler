use poem::{session::Session, web::Data, Result};
use poem_openapi::{param::Query, payload::Json, OpenApi};
use sea_orm::{ActiveValue::NotSet, Set};

use crate::{
    api_response, entity::role, error::NoPermission, local_time, logic, response::ApiStdResponse,
    return_err, return_ok, AppState,
};

pub struct RoleApi;

mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object, Serialize, Deserialize)]
    pub struct UpdateResult {
        pub affected: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveRoleReq {
        pub id: Option<u64>,
        pub name: String,
        pub info: String,
        pub user_ids: Option<Vec<String>>,
        pub permissions: Option<Vec<String>>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryRoleResp {
        pub total: u64,
        pub list: Vec<RoleRecord>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct RoleRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub user_total: i64,
        pub permissions: Vec<String>,
        pub created_time: String,
        pub created_user: String,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SetUserReq {
        pub role_id: u64,
        pub user_ids: Option<Vec<String>>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct DeleteRoleReq {
        pub role_id: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct BindInsanceReq {
        pub role_id: u64,
        pub instance_group_ids: Option<Vec<u64>>,
        pub instance_ids: Option<Vec<String>>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct BindInsanceResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct UnbindInsanceReq {
        pub role_id: u64,
        pub instance_group_ids: Option<Vec<u64>>,
        pub instance_ids: Option<Vec<u64>>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct UnbindInsanceResp {
        pub result: u64,
    }
}

#[OpenApi(prefix_path = "/role", tag = super::Tag::Role)]
impl RoleApi {
    #[oai(path = "/save", method = "post")]
    pub async fn save_role(
        &self,
        user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        state: Data<&AppState>,
        Json(req): Json<types::SaveRoleReq>,
    ) -> Result<ApiStdResponse<types::UpdateResult>> {
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }

        if req.id == Some(1) {
            return_err!("Dont't allow modify admin role");
        }

        let affected = state
            .service()
            .role
            .save_role(
                role::ActiveModel {
                    id: req.id.filter(|&v| v != 0).map_or(NotSet, |v| Set(v)),
                    name: Set(req.name),
                    info: Set(req.info),
                    created_user: Set(user_info.username.clone()),
                    ..Default::default()
                },
                req.user_ids,
            )
            .await?;
        if let Some(permissions) = req.permissions {
            state.set_permissions(affected, permissions).await?;
        }

        return_ok!(types::UpdateResult { affected })
    }

    #[oai(path = "/set-user", method = "post")]
    pub async fn set_user(
        &self,
        user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        state: Data<&AppState>,
        Json(req): Json<types::SetUserReq>,
    ) -> Result<ApiStdResponse<types::UpdateResult>> {
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }
        let state_clone = state.clone();
        let affected = state
            .service()
            .role
            .set_user(
                req.role_id,
                req.user_ids,
                |user_id: String, role: String| async move {
                    state_clone.set_role_for_user(&user_id, &role).await?;
                    Ok(())
                },
            )
            .await?;
        state.load_policy().await?;
        return_ok!(types::UpdateResult { affected })
    }

    #[oai(path = "/delete", method = "post")]
    pub async fn delete_role(
        &self,
        _user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        state: Data<&AppState>,
        Json(req): Json<types::SetUserReq>,
    ) -> Result<ApiStdResponse<types::UpdateResult>> {
        let affected = state.service().role.delete_role(req.role_id).await?;
        return_ok!(types::UpdateResult { affected })
    }

    #[oai(path = "/list", method = "get")]
    pub async fn query_role(
        &self,
        state: Data<&AppState>,
        _session: &Session,

        Query(id): Query<Option<u64>>,
        Query(default_id): Query<Option<u64>>,
        Query(name): Query<Option<String>>,
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
    ) -> Result<ApiStdResponse<types::QueryRoleResp>> {
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }

        let svc = state.service();
        let ret = svc
            .role
            .query_role(
                name.filter(|v| v != ""),
                None,
                id.filter(|&v| v > 0),
                default_id.filter(|&v| v > 0),
                page - 1,
                page_size,
            )
            .await?;

        let role_count = svc.user.count_by_role().await?;

        let mut list: Vec<types::RoleRecord> = Vec::new();
        for v in ret.0 {
            let permissions = state.get_permissions_for_role(v.id).await?;
            list.push(types::RoleRecord {
                id: v.id,
                created_time: local_time!(v.created_time),
                user_total: role_count.get_by_role_id(v.id).map_or(0, |v| v.total),
                name: v.name,
                info: v.info,
                permissions,
                created_user: v.created_user,
            });
        }

        return_ok!(types::QueryRoleResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/bind-instance", method = "post")]
    pub async fn bind_instance(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::BindInsanceReq>,
    ) -> api_response!(types::BindInsanceResp) {
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }

        let svc = state.service();
        let ret = svc
            .role
            .bind_instance(
                req.role_id,
                req.instance_group_ids.filter(|v| v.len() > 0),
                req.instance_ids.filter(|v| v.len() > 0),
            )
            .await?;
        return_ok!(types::BindInsanceResp { result: ret })
    }

    #[oai(path = "/unbind-instance", method = "post")]
    pub async fn unbind_instance(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::UnbindInsanceReq>,
    ) -> api_response!(types::UnbindInsanceResp) {
        let ok = state.can_manage_user(&user_info.user_id).await?;
        if !ok {
            return Err(NoPermission().into());
        }

        let svc = state.service();
        let ret = svc
            .role
            .unbind_instance(
                req.role_id,
                req.instance_group_ids.filter(|v| v.len() > 0),
                req.instance_ids.filter(|v| v.len() > 0),
            )
            .await?;
        return_ok!(types::UnbindInsanceResp { result: ret })
    }
}
