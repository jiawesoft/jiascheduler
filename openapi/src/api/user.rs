use crate::{
    entity::user,
    error::NoPermission,
    local_time,
    logic::{self, user::UserLogic},
    response::ApiStdResponse,
    return_ok, AppState,
};

pub struct UserApi;

use poem::{session::Session, web::Data, Result};
use poem_openapi::{param::Query, payload::Json, OpenApi};
use sea_orm::{ActiveValue::NotSet, Set};

pub mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object)]
    pub struct LoginReq {
        pub username: String,
        pub password: String,
    }

    #[derive(Serialize, Object, Default)]
    pub struct Logined {
        pub token: String,
    }

    #[derive(Object, Serialize, Deserialize, Default)]
    pub struct UserInfo {
        pub username: String,
        pub nickname: String,
        pub avatar: String,
        pub email: String,
        pub gender: String,
        pub is_root: bool,
        pub introduction: String,
        pub phone: String,
        pub user_id: String,
        pub role: String,
        pub role_id: u64,
        pub permissions: Vec<String>,
        pub updated_time: String,
        pub created_time: String,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct RegistryReq {
        pub username: String,
        pub nickname: String,
        pub gender: String,
        pub phone: Option<String>,
        pub email: String,
        pub avatar: Option<String>,
        pub password: String,
        pub introduction: Option<String>,
        pub role_id: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct RegistryResponse {
        pub result: u64,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct UpdateUserInfoReq {
        pub password: Option<String>,
        pub nickname: String,
        pub avatar: String,
        pub email: String,
        pub gender: String,
        pub introduction: String,
        pub phone: String,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct UpdateInfoResp {
        pub affected: u64,
    }

    #[derive(Object, Serialize)]
    pub struct QueryUserResp {
        pub total: u64,
        pub list: Vec<UserRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct UserRecord {
        pub user_id: String,
        pub username: String,
        pub nickname: String,
        pub role: String,
        pub role_id: u64,
        pub avatar: String,
        pub email: String,
        pub phone: String,
        pub gender: String,
        pub introduction: String,
        pub created_time: String,
        pub updated_time: String,
    }
}

#[OpenApi(prefix_path = "/user", tag = super::Tag::User)]
impl UserApi {
    #[oai(path = "/login", method = "post")]
    pub async fn login(
        &self,
        session: &Session,
        state: Data<&AppState>,
        // #[oai(name = "TOKEN")] _token: Header<String>,
        Json(login_req): Json<types::LoginReq>,
    ) -> Result<ApiStdResponse<types::Logined>> {
        let svc = state.service();
        let login_user = svc
            .user
            .valid_user(&login_req.username, &login_req.password)
            .await?;

        let permissions = state.get_permissions_for_user(&login_user.user_id).await?;

        session.set(
            UserLogic::SESS_KEY,
            logic::types::UserInfo {
                username: login_req.username,
                nickname: login_user.nickname,
                avatar: login_user.avatar,
                email: login_user.email,
                role_id: login_user.role_id,
                is_root: login_user.is_root,
                introduction: login_user.introduction,
                phone: login_user.phone,
                created_time: local_time!(login_user.created_time),
                updated_time: local_time!(login_user.updated_time),
                user_id: login_user.user_id,
                gender: login_user.gender,
                permissions,
                role: login_user.role.unwrap_or_default(),
            },
        );

        return_ok!(types::Logined {
            token: "success".into(),
        });
    }

    #[oai(path = "/logout", method = "post")]
    pub async fn logout(
        &self,
        sess: &Session,
        // #[oai(name = "TOKEN")] _token: Header<String>,
    ) -> Result<ApiStdResponse<bool>> {
        sess.clear();
        return_ok!(true);
    }

    #[oai(path = "/register", method = "post")]
    pub async fn register(
        &self,
        _session: &Session,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::RegistryReq>,
    ) -> Result<ApiStdResponse<types::RegistryResponse>> {
        let svc = state.service();

        let ok = state.can_manage_user(&user_info.user_id).await?;

        if !ok {
            return Err(NoPermission().into());
        }

        let v = svc
            .user
            .create_user(user::Model {
                username: req.username,
                nickname: req.nickname,
                password: req.password,
                avatar: req.avatar.unwrap_or_default(),
                email: req.email,
                phone: req.phone.unwrap_or_default(),
                gender: req.gender,
                role_id: req.role_id,
                introduction: req.introduction.unwrap_or_default(),
                ..Default::default()
            })
            .await?;

        return_ok!(types::RegistryResponse { result: v })
    }

    #[oai(path = "/info", method = "post")]
    pub async fn get_user(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::UserInfo>> {
        let permissions = state.get_permissions_for_user(&user_info.user_id).await?;
        return_ok!(types::UserInfo {
            username: user_info.username.clone(),
            nickname: user_info.nickname.clone(),
            permissions,
            role_id: user_info.role_id,
            is_root: user_info.is_root,
            avatar: user_info.avatar.clone(),
            email: user_info.email.clone(),
            introduction: user_info.introduction.clone(),
            phone: user_info.phone.clone(),
            user_id: user_info.user_id.clone(),
            role: user_info.role.clone(),
            updated_time: user_info.updated_time.clone(),
            created_time: user_info.created_time.clone(),
            gender: user_info.gender.clone(),
        })
    }

    #[oai(path = "/update-info", method = "post")]
    pub async fn update_info(
        &self,
        sess: &Session,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        // #[oai(name = "TOKEN")] _token: Header<String>,
        Json(req): Json<types::UpdateUserInfoReq>,
    ) -> Result<ApiStdResponse<types::UpdateInfoResp>> {
        let svc = state.service();
        let user_id = user_info.user_id.clone();

        let affected = svc
            .user
            .update_user(
                user::ActiveModel {
                    user_id: Set(user_id),
                    nickname: Set(req.nickname),
                    avatar: Set(req.avatar),
                    email: Set(req.email),
                    phone: Set(req.phone),
                    gender: Set(req.gender),
                    password: req.password.filter(|v| v != "").map_or(NotSet, |v| Set(v)),
                    introduction: Set(req.introduction),
                    ..Default::default()
                },
                |_, _| async { Ok(()) },
            )
            .await?;

        let permissions = state.get_permissions_for_user(&user_info.user_id).await?;
        match svc.user.get_user(Some(&user_info.username), None).await? {
            Some(record) => {
                sess.set(
                    UserLogic::SESS_KEY,
                    logic::types::UserInfo {
                        username: record.username,
                        nickname: record.nickname,
                        avatar: record.avatar,
                        email: record.email,
                        role_id: record.role_id,
                        is_root: record.is_root,
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

        return_ok!(types::UpdateInfoResp { affected });
    }

    #[oai(path = "/list", method = "get")]
    pub async fn query_user(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        Query(role_id): Query<Option<u64>>,
        Query(user_id): Query<Option<Vec<String>>>,
        Query(ignore_role_id): Query<Option<u64>>,
        Query(keyword): Query<Option<String>>,
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
        _user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::QueryUserResp>> {
        let svc = state.service();
        let ret = svc
            .user
            .query_user(
                user_id.filter(|v| v.len() > 0),
                None,
                None,
                None,
                role_id,
                ignore_role_id.filter(|&v| v != 0),
                keyword.filter(|v| v != ""),
                page - 1,
                page_size,
            )
            .await?;
        let list: Vec<types::UserRecord> = ret
            .0
            .into_iter()
            .map(|v| types::UserRecord {
                user_id: v.user_id,
                username: v.username,
                nickname: v.nickname,
                role: v.role.unwrap_or_default(),
                role_id: v.role_id,
                avatar: v.avatar,
                email: v.email,
                phone: v.phone,
                gender: v.gender,
                introduction: v.introduction,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();

        return_ok!(types::QueryUserResp {
            total: ret.1,
            list: list,
        })
    }
}
