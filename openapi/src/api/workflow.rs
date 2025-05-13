use poem::{session::Session, web::Data};
use poem_openapi::{
    param::{Header, Query},
    payload::Json,
    OpenApi,
};
use sea_orm::{ActiveValue::NotSet, Set};

use crate::{api_response, local_time, logic, return_err, return_ok, state::AppState};

mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowReq {
        pub name: String,
        pub info: Option<String>,
        pub version: String,
        pub id: Option<u64>,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowResp {
        pub result: u64,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowVersionReq {
        pub pid: Option<u64>,
        pub name: String,
        pub nodes: Option<serde_json::Value>,
        pub edges: Option<serde_json::Value>,
        pub info: Option<String>,
        pub version: String,
        #[oai(validator(custom = "crate::api::OneOfValidator::new(vec![\"draft\",\"release\"])"))]
        pub save_type: String,
        pub id: Option<u64>,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowVersionResp {
        pub result: u64,
    }
}

pub struct WorkflowApi;

#[OpenApi(prefix_path = "/workflow", tag = super::Tag::Team)]
impl WorkflowApi {
    #[oai(path = "/save", method = "post")]
    pub async fn save_workflow(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveWorkflowReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::SaveWorkflowResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, req.id)
            .await?
        {
            return_err!("no permission");
        }

        let ret = svc
            .workflow
            .save_workflow(req.id, &user_info, req.name, req.info, team_id)
            .await?;

        return_ok!(types::SaveWorkflowResp { result: ret })
    }

    #[oai(path = "/version/save", method = "post")]
    pub async fn save_workflow_version(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveWorkflowVersionReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::SaveWorkflowVersionResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, req.id)
            .await?
        {
            return_err!("no permission");
        }

        let ret = svc
            .workflow
            .save_workflow_version(
                req.pid,
                req.id,
                &user_info,
                req.name,
                req.info,
                req.version,
                req.save_type,
                req.nodes,
                req.edges,
                team_id,
            )
            .await?;

        return_ok!(types::SaveWorkflowVersionResp { result: ret })
    }
}
