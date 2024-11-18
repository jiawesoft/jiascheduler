use crate::{entity::executor, local_time, logic, response::ApiStdResponse, return_ok, AppState};
use poem::{session::Session, web::Data, Result};
use poem_openapi::{param::Query, payload::Json, OpenApi};
use sea_orm::{ActiveValue::NotSet, Set};

mod types {
    use poem_openapi::Object;
    use serde::Serialize;

    #[derive(Object, Serialize, Default)]
    pub struct DeleteExecutorReq {
        pub id: u32,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SaveExecutorReq {
        pub id: Option<u64>,
        #[oai(validator(min_length = 1))]
        pub name: String,
        pub command: String,
        pub platform: String,
        pub info: String,
        pub read_code_from_stdin: Option<bool>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SaveExecutorRes {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryExecutorResp {
        pub total: u64,
        pub list: Vec<ExecutorRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct ExecutorRecord {
        pub id: u64,
        pub name: String,
        pub command: String,
        pub platform: String,
        pub info: String,
        pub created_time: String,
        pub updated_time: String,
    }
}

pub struct ExecutorApi;

#[OpenApi(prefix_path = "/executor", tag = super::Tag::Executor)]
impl ExecutorApi {
    #[oai(path = "/delete", method = "post")]
    pub async fn delete_executor(
        &self,
        state: Data<&AppState>,
        Json(req): Json<types::DeleteExecutorReq>,
    ) -> Result<ApiStdResponse<u64>> {
        let svc = state.service();
        let ret = svc.executor.delete_job(req.id).await?;
        return_ok!(ret)
    }

    #[oai(path = "/save", method = "post")]
    pub async fn save_executor(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveExecutorReq>,
    ) -> Result<ApiStdResponse<types::SaveExecutorRes>> {
        let svc = state.service();

        let ret = svc
            .executor
            .save_executor(executor::ActiveModel {
                id: req.id.filter(|v| *v != 0).map_or(NotSet, |v| Set(v)),
                name: Set(req.name),
                command: Set(req.command),
                platform: Set(req.platform),
                info: Set(req.info),
                read_code_from_stdin: Set(req.read_code_from_stdin.map_or(0, |v| match v {
                    true => 1,
                    false => 0,
                })),
                created_user: Set(user_info.username.clone()),
                updated_user: Set(user_info.username.clone()),
                ..Default::default()
            })
            .await?;

        return_ok!(types::SaveExecutorRes {
            result: ret.id.as_ref().to_owned()
        });
    }

    #[oai(path = "/list", method = "get")]
    pub async fn query_executor(
        &self,
        state: Data<&AppState>,
        _session: &Session,

        Query(default_id): Query<Option<u64>>,

        #[oai(
            default = "crate::api::default_page",
            validator(maximum(value = "10000"))
        )]
        Query(page): Query<u64>,
        Query(name): Query<Option<String>>,

        #[oai(
            default = "crate::api::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        _user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::QueryExecutorResp>> {
        let svc = state.service();
        let ret = svc
            .executor
            .query_executor(default_id, name, page - 1, page_size)
            .await?;

        let list: Vec<types::ExecutorRecord> = ret
            .0
            .into_iter()
            .map(|v: executor::Model| types::ExecutorRecord {
                id: v.id,
                name: v.name,
                command: v.command,
                platform: v.platform,
                info: v.info,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();
        return_ok!(types::QueryExecutorResp {
            total: ret.1,
            list: list,
        })
    }
}
