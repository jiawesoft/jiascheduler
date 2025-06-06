use poem::{session::Session, web::Data, Endpoint, EndpointExt};
use poem_openapi::{
    param::{Header, Query},
    payload::Json,
    OpenApi,
};
use sea_orm::{ActiveValue::NotSet, Set};

use crate::{
    api::workflow::types::NodeConfig, api_response, local_time, logic, middleware, return_err,
    return_ok, state::AppState,
};

mod types {
    use poem_openapi::{Enum, Object};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use service::logic;
    use std::fmt::Display;

    pub fn default_page() -> u64 {
        1
    }

    pub fn default_page_size() -> u64 {
        20
    }

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

    #[derive(Serialize, Enum, Deserialize, Clone)]
    pub enum NodeType {
        #[serde(rename = "bpmn:startEvent")]
        StartEvent,
    }

    impl Display for NodeType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                NodeType::StartEvent => write!(f, "bpmn:startEvent"),
            }
        }
    }

    #[derive(Serialize, Object, Deserialize, Clone)]
    pub struct Task {
        pub standard: Option<StandardJob>,
        pub custom: Option<CustomJob>,
    }

    impl TryInto<logic::workflow::types::Task> for Task {
        type Error = anyhow::Error;

        fn try_into(self) -> Result<logic::workflow::types::Task, Self::Error> {
            if let Some(std_job) = self.standard {
                Ok(logic::workflow::types::Task::Standard(
                    logic::workflow::types::StandardJob { eid: std_job.eid },
                ))
            } else if let Some(job) = self.custom {
                Ok(logic::workflow::types::Task::Custom(
                    logic::workflow::types::CustomJob {
                        executor_id: job.executor_id,
                        timeout: job.timeout,
                        code: job.code,
                        upload_file: job.upload_file,
                    },
                ))
            } else {
                Ok(logic::workflow::types::Task::None)
            }
        }
    }

    #[derive(Serialize, Enum, Deserialize, Clone)]
    pub enum TaskType {
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "custom")]
        Custom,
    }

    impl Display for TaskType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TaskType::Standard => write!(f, "standard"),
                TaskType::Custom => write!(f, "custom"),
            }
        }
    }

    #[derive(Serialize, Object, Deserialize, Clone, Debug)]
    pub struct CustomJob {
        pub executor_id: u64,
        pub timeout: Option<u64>,
        pub code: String,
        pub upload_file: Option<String>,
    }

    #[derive(Serialize, Object, Deserialize, Clone, Debug)]
    pub struct StandardJob {
        pub eid: String,
    }

    #[derive(Clone, Object, Serialize, Deserialize)]
    pub struct NodeConfig {
        pub id: String,
        pub name: String,
        pub node_type: NodeType,
        pub task_type: TaskType,
        pub task: Task,
        pub data: serde_json::Value,
    }

    impl TryInto<logic::workflow::types::NodeConfig> for NodeConfig {
        type Error = anyhow::Error;
        fn try_into(self) -> Result<logic::workflow::types::NodeConfig, Self::Error> {
            Ok(logic::workflow::types::NodeConfig {
                id: self.id,
                name: self.name,
                node_type: self.node_type.to_string().as_str().try_into()?,
                task_type: self.task_type.to_string().as_str().try_into()?,
                task: self.task.try_into()?,
                data: self.data,
            })
        }
    }

    #[derive(Clone, Object, Serialize, Deserialize)]
    pub struct EdgeConfig {
        pub id: String,
        pub name: String,
        pub source_node_id: String,
        pub target_node_id: String,
        pub data: serde_json::Value,
    }

    impl TryInto<logic::workflow::types::EdgeConfig> for EdgeConfig {
        type Error = anyhow::Error;
        fn try_into(self) -> Result<logic::workflow::types::EdgeConfig, Self::Error> {
            Ok(logic::workflow::types::EdgeConfig {
                id: self.id,
                name: self.name,
                source_node_id: self.source_node_id,
                target_node_id: self.target_node_id,
                data: self.data,
            })
        }
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowVersionReq {
        pub pid: Option<u64>,
        pub name: String,
        pub nodes: Option<Vec<NodeConfig>>,
        pub edges: Option<Vec<EdgeConfig>>,
        pub info: Option<String>,
        pub version: String,
        #[oai(validator(custom = "crate::api::OneOfValidator::new(vec![\"draft\",\"release\"])"))]
        pub version_status: String,
        pub id: Option<u64>,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowVersionResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryWorkflowResp {
        pub total: u64,
        pub list: Vec<WorkflowRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct WorkflowRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub team_name: Option<String>,
        pub team_id: u64,
        pub updated_time: String,
        pub created_user: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryWorkflowVersionResp {
        pub total: u64,
        pub list: Vec<WorkflowVersionRecord>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct WorkflowVersionRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub updated_time: String,
        pub created_user: String,
        pub nodes: Option<Vec<NodeConfig>>,
        pub edges: Option<Vec<EdgeConfig>>,
    }
}

fn set_middleware(ep: impl Endpoint) -> impl Endpoint {
    ep.with(middleware::TeamPermissionMiddleware)
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

        let nodes: Option<Vec<_>> = req
            .nodes
            .map(|v| {
                v.into_iter()
                    .map(|v| TryInto::<logic::workflow::types::NodeConfig>::try_into(v))
                    .collect()
            })
            .transpose()?;
        let edges: Option<Vec<_>> = req
            .edges
            .map(|v| {
                v.into_iter()
                    .map(|v| TryInto::<logic::workflow::types::EdgeConfig>::try_into(v))
                    .collect()
            })
            .transpose()?;
        let ret = svc
            .workflow
            .save_workflow_version(
                req.pid,
                req.id,
                &user_info,
                req.name,
                req.info,
                req.version,
                req.version_status,
                nodes,
                edges,
                team_id,
            )
            .await?;

        return_ok!(types::SaveWorkflowVersionResp { result: ret })
    }

    #[oai(path = "/list", method = "get", transform = "set_middleware")]
    pub async fn query_workflow(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        Query(search_username): Query<Option<String>>,
        Query(default_id): Query<Option<u64>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        #[oai(default)] Query(name): Query<Option<String>>,
    ) -> api_response!(types::QueryWorkflowResp) {
        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };
        let svc = state.service();
        let ret = svc
            .workflow
            .get_workflow_list(
                &user_info,
                search_username.as_deref(),
                default_id,
                team_id,
                name,
                page,
                page_size,
            )
            .await?;
        let list = ret
            .0
            .into_iter()
            .map(|v| types::WorkflowRecord {
                id: v.id,
                name: v.name,
                info: v.info,
                team_name: v.team_name,
                team_id: v.team_id,
                updated_time: local_time!(v.updated_time),
                created_user: v.created_user,
            })
            .collect();

        return_ok!(types::QueryWorkflowResp { total: ret.1, list })
    }

    #[oai(path = "/version/list", method = "get", transform = "set_middleware")]
    pub async fn query_workflow_version(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        Query(username): Query<Option<String>>,
        Query(workflow_id): Query<u64>,
        Query(default_id): Query<Option<u64>>,
        #[oai(validator(custom = "super::OneOfValidator::new(vec![\"draft\",\"release\",])"))]
        Query(version_status): Query<String>,
        #[oai(name = "X-Team-Id")] Header(_team_id): Header<Option<u64>>,
        #[oai(default)] Query(name): Query<Option<String>>,
    ) -> api_response!(types::QueryWorkflowVersionResp) {
        let svc = state.service();
        let ret = svc
            .workflow
            .get_workflow_version_list(
                &user_info,
                name,
                username,
                workflow_id,
                default_id,
                page,
                page_size,
            )
            .await?;
        let list = ret
            .0
            .into_iter()
            .map(|v| types::WorkflowVersionRecord {
                id: v.id,
                name: v.name,
                info: v.info,
                nodes: v.nodes.map(|v| serde_json::from_value(v).unwrap_or(vec![])),
                edges: v.edges.map(|v| serde_json::from_value(v).unwrap_or(vec![])),
                updated_time: local_time!(v.updated_time),
                created_user: v.created_user,
            })
            .collect();

        return_ok!(types::QueryWorkflowVersionResp { total: ret.1, list })
    }
}
