use anyhow::{Context, Result};
use entity::workflow_timer;
use poem::{web::Data, Endpoint, EndpointExt};
use poem_openapi::{
    param::{Header, Query},
    payload::Json,
    OpenApi,
};
use sea_orm::{ActiveValue::NotSet, ActiveValue::Set};
use service::logic::workflow::{
    timer::WorkflowTimerTask,
    types::{EdgeConfig, NodeConfig},
    WorkflowLogic,
};

use crate::{
    api::workflow::types::UserVariables, api_response, local_time, logic, middleware, return_err,
    return_ok, state::AppState,
};

mod types {
    use poem_openapi::{Enum, Object};
    use serde::{Deserialize, Serialize};
    use service::logic::{self, workflow::condition};
    use std::fmt::Display;

    pub fn default_page() -> u64 {
        1
    }

    pub fn default_page_size() -> u64 {
        20
    }

    #[derive(Object, Serialize, Deserialize, Default)]
    pub struct UserVariables {
        pub name: String,
        pub val: String,
        pub info: String,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowReq {
        pub id: Option<u64>,
        pub name: String,
        pub info: Option<String>,
        pub nodes: Option<Vec<NodeConfig>>,
        pub edges: Option<Vec<EdgeConfig>>,
        pub user_variables: Option<Vec<UserVariables>>,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct SaveWorkflowResp {
        pub result: u64,
    }

    #[derive(Serialize, Enum, Deserialize, Clone)]
    pub enum NodeType {
        #[oai(rename = "bpmn:startEvent")]
        #[serde(rename = "bpmn:startEvent")]
        StartEvent,
        #[oai(rename = "bpmn:serviceTask")]
        #[serde(rename = "bpmn:serviceTask")]
        ServiceTask,
        #[oai(rename = "bpmn:endEvent")]
        #[serde(rename = "bpmn:endEvent")]
        EndEvent,
        #[oai(rename = "bpmn:exclusiveGateway")]
        #[serde(rename = "bpmn:exclusiveGateway")]
        ExclusiveGateway,
    }

    impl Display for NodeType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                NodeType::StartEvent => write!(f, "bpmn:startEvent"),
                NodeType::ServiceTask => write!(f, "bpmn:serviceTask"),
                NodeType::EndEvent => write!(f, "bpmn:endEvent"),
                NodeType::ExclusiveGateway => write!(f, "bpmn:exclusiveGateway"),
            }
        }
    }

    impl TryFrom<&str> for NodeType {
        type Error = anyhow::Error;
        fn try_from(value: &str) -> Result<Self, Self::Error> {
            match value {
                "bpmn:startEvent" => Ok(NodeType::StartEvent),
                "bpmn:serviceTask" => Ok(NodeType::ServiceTask),
                "bpmn:endEvent" => Ok(NodeType::EndEvent),
                "bpmn:exclusiveGateway" => Ok(NodeType::ExclusiveGateway),
                _ => Err(anyhow::anyhow!("Invalid node type")),
            }
        }
    }

    #[derive(Serialize, Object, Deserialize, Clone)]
    pub struct Task {
        pub standard: Option<StandardJob>,
        pub custom: Option<CustomJob>,
    }

    impl TryFrom<logic::workflow::types::Task> for Task {
        type Error = anyhow::Error;
        fn try_from(value: logic::workflow::types::Task) -> Result<Self, Self::Error> {
            Ok(match value {
                logic::workflow::types::Task::Standard(std_job) => Self {
                    standard: Some(StandardJob {
                        eid: std_job.eid,
                        formal_args: std_job
                            .formal_args
                            .iter()
                            .map(|v| WorkflowJobArgs {
                                name: v.name.clone(),
                                val: v.val.clone(),
                                val_type: v.val_type.clone(),
                                info: v.info.clone(),
                            })
                            .collect(),
                        target: std_job.target,
                    }),
                    custom: None,
                },
                logic::workflow::types::Task::Custom(custom_job) => Self {
                    standard: None,
                    custom: Some(CustomJob {
                        executor_id: custom_job.executor_id,
                        timeout: custom_job.timeout,
                        code: custom_job.code,
                        upload_file: custom_job.upload_file,
                        target: custom_job.target,
                        formal_args: custom_job
                            .formal_args
                            .iter()
                            .map(|v| WorkflowJobArgs {
                                name: v.name.clone(),
                                val: v.val.clone(),
                                val_type: v.val_type.clone(),
                                info: v.info.clone(),
                            })
                            .collect(),
                    }),
                },
                logic::workflow::types::Task::None => Self {
                    standard: None,
                    custom: None,
                },
            })
        }
    }

    impl TryInto<logic::workflow::types::Task> for Task {
        type Error = anyhow::Error;

        fn try_into(self) -> Result<logic::workflow::types::Task, Self::Error> {
            if let Some(std_job) = self.standard {
                Ok(logic::workflow::types::Task::Standard(
                    logic::workflow::types::StandardJob {
                        eid: std_job.eid,
                        formal_args: std_job
                            .formal_args
                            .iter()
                            .map(|v| logic::workflow::types::WorkflowJobArgs {
                                name: v.name.clone(),
                                val: v.val.clone(),
                                val_type: v.val_type.clone(),
                                info: v.info.clone(),
                            })
                            .collect(),
                        target: std_job.target,
                    },
                ))
            } else if let Some(job) = self.custom {
                Ok(logic::workflow::types::Task::Custom(
                    logic::workflow::types::CustomJob {
                        executor_id: job.executor_id,
                        timeout: job.timeout,
                        code: job.code,
                        upload_file: job.upload_file,
                        target: job.target,
                        formal_args: job
                            .formal_args
                            .iter()
                            .map(|v| logic::workflow::types::WorkflowJobArgs {
                                name: v.name.clone(),
                                val: v.val.clone(),
                                val_type: v.val_type.clone(),
                                info: v.info.clone(),
                            })
                            .collect(),
                    },
                ))
            } else {
                Ok(logic::workflow::types::Task::None)
            }
        }
    }

    #[derive(Serialize, Enum, Deserialize, Clone, PartialEq)]
    pub enum TaskType {
        #[oai(rename = "standard")]
        #[serde(rename = "standard")]
        Standard,
        #[oai(rename = "custom")]
        #[serde(rename = "custom")]
        Custom,
        #[oai(rename = "none")]
        #[serde(rename = "none")]
        None,
    }

    impl Display for TaskType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TaskType::Standard => write!(f, "standard"),
                TaskType::Custom => write!(f, "custom"),
                TaskType::None => write!(f, "none"),
            }
        }
    }

    impl TryFrom<&str> for TaskType {
        type Error = anyhow::Error;
        fn try_from(value: &str) -> Result<Self, Self::Error> {
            match value {
                "standard" => Ok(TaskType::Standard),
                "custom" => Ok(TaskType::Custom),
                "none" => Ok(TaskType::None),
                _ => Err(anyhow::anyhow!("Invalid task type")),
            }
        }
    }

    #[derive(Default, Object, Serialize, Deserialize, Clone, Debug)]
    pub struct WorkflowJobArgs {
        pub name: String,
        pub val: String,
        pub val_type: String,
        pub info: Option<String>,
    }

    #[derive(Serialize, Object, Deserialize, Clone, Debug)]
    pub struct CustomJob {
        pub executor_id: u64,
        pub timeout: Option<u64>,
        pub code: String,
        pub upload_file: Option<String>,
        pub target: Option<Vec<String>>,
        pub formal_args: Vec<WorkflowJobArgs>,
    }

    #[derive(Serialize, Object, Deserialize, Clone, Debug)]
    pub struct StandardJob {
        pub eid: String,
        pub formal_args: Vec<WorkflowJobArgs>,
        pub target: Option<Vec<String>>,
    }

    #[derive(Clone, Object, Serialize, Deserialize)]
    pub struct NodeConfig {
        pub id: String,
        pub name: String,
        pub node_type: NodeType,
        pub task_type: TaskType,
        pub is_join_all: bool,
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
                is_join_all: self.is_join_all,
                data: self.data,
            })
        }
    }

    impl TryFrom<logic::workflow::types::NodeConfig> for NodeConfig {
        type Error = anyhow::Error;
        fn try_from(value: logic::workflow::types::NodeConfig) -> Result<Self, Self::Error> {
            Ok(NodeConfig {
                id: value.id,
                name: value.name,
                is_join_all: value.is_join_all,
                node_type: value.node_type.to_string().as_str().try_into()?,
                task_type: value.task_type.to_string().as_str().try_into()?,
                task: value.task.try_into()?,
                data: value.data,
            })
        }
    }

    #[derive(Clone, Object, Serialize, Deserialize)]
    pub struct EdgeConfig {
        pub id: String,
        pub name: String,
        pub condition: Option<Condition>,
        pub source_node_id: String,
        pub target_node_id: String,
        pub data: serde_json::Value,
    }

    #[derive(Serialize, Enum, Deserialize, Clone, PartialEq)]
    pub enum ConditionValType {
        #[oai(rename = "user_variables")]
        #[serde(rename = "user_variables")]
        UserVariables,
        #[oai(rename = "custom")]
        #[serde(rename = "custom")]
        Custom,
        #[oai(rename = "exit_code")]
        #[serde(rename = "exit_code")]
        ExitCode,
        #[oai(rename = "output")]
        #[serde(rename = "output")]
        Output,
    }

    impl From<condition::ConditionValType> for ConditionValType {
        fn from(value: condition::ConditionValType) -> Self {
            match value {
                condition::ConditionValType::UserVariables => ConditionValType::UserVariables,
                condition::ConditionValType::Custom => ConditionValType::Custom,
                condition::ConditionValType::ExitCode => ConditionValType::ExitCode,
                condition::ConditionValType::Output => ConditionValType::Output,
            }
        }
    }

    impl Into<condition::ConditionValType> for ConditionValType {
        fn into(self) -> condition::ConditionValType {
            match self {
                ConditionValType::UserVariables => condition::ConditionValType::UserVariables,
                ConditionValType::Custom => condition::ConditionValType::Custom,
                ConditionValType::ExitCode => condition::ConditionValType::ExitCode,
                ConditionValType::Output => condition::ConditionValType::Output,
            }
        }
    }

    #[derive(Serialize, Object, Deserialize, Clone, PartialEq)]
    pub struct ConditionVal {
        pub val_type: ConditionValType,
        pub val: String,
    }

    #[derive(Clone, Object, Serialize, Deserialize)]
    pub struct Rule {
        pub name: String,
        pub left_val: ConditionVal,
        pub op: String,
        pub right_val: ConditionVal,
    }

    #[derive(Clone, Object, Serialize, Deserialize)]
    pub struct Condition {
        pub rules: Vec<Rule>,
        pub expr: String,
        pub logical_op: String,
    }

    impl TryInto<logic::workflow::types::EdgeConfig> for EdgeConfig {
        type Error = anyhow::Error;
        fn try_into(self) -> Result<logic::workflow::types::EdgeConfig, Self::Error> {
            Ok(logic::workflow::types::EdgeConfig {
                id: self.id,
                name: self.name,
                condition: self.condition.map_or(None, |v| {
                    Some(condition::Condition {
                        expr: v.expr.clone(),
                        logical_op: v.logical_op,
                        rules: v
                            .rules
                            .iter()
                            .map(|r| condition::Rule {
                                name: r.name.clone(),
                                left_val: condition::ConditionVal {
                                    val_type: r.left_val.val_type.clone().into(),
                                    val: r.left_val.val.to_string(),
                                },
                                op: r.op.to_string(),
                                right_val: condition::ConditionVal {
                                    val_type: r.right_val.val_type.clone().into(),
                                    val: r.right_val.val.to_string(),
                                },
                            })
                            .collect(),
                    })
                }),
                source_node_id: self.source_node_id,
                target_node_id: self.target_node_id,
                data: self.data,
            })
        }
    }

    impl TryFrom<logic::workflow::types::EdgeConfig> for EdgeConfig {
        type Error = anyhow::Error;
        fn try_from(value: logic::workflow::types::EdgeConfig) -> Result<Self, Self::Error> {
            Ok(EdgeConfig {
                id: value.id,
                name: value.name,
                condition: value.condition.map_or(None, |v| {
                    Some(Condition {
                        expr: v.expr.clone(),
                        logical_op: v.logical_op.clone(),
                        rules: v
                            .rules
                            .iter()
                            .map(|r| Rule {
                                name: r.name.clone(),
                                left_val: ConditionVal {
                                    val_type: r.left_val.val_type.clone().into(),
                                    val: r.left_val.val.to_string(),
                                },
                                op: r.op.clone(),
                                right_val: ConditionVal {
                                    val_type: r.right_val.val_type.clone().into(),
                                    val: r.right_val.val.to_string(),
                                },
                            })
                            .collect(),
                    })
                }),
                source_node_id: value.source_node_id,
                target_node_id: value.target_node_id,
                data: value.data,
            })
        }
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct ReleaseWorkflowVersionReq {
        pub workflow_id: u64,
        pub version: String,
        pub version_info: Option<String>,
        pub nodes: Option<Vec<NodeConfig>>,
        pub edges: Option<Vec<EdgeConfig>>,
        pub user_variables: Option<Vec<UserVariables>>,
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

    #[derive(Object, Serialize, Deserialize, Default)]
    pub struct ResourceTag {
        pub id: u64,
        pub tag_name: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct WorkflowRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub tags: Option<Vec<ResourceTag>>,
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
        pub workflow_id: u64,
        pub version: String,
        pub version_info: String,
        pub created_time: String,
        pub created_user: String,
        pub nodes: Option<Vec<NodeConfig>>,
        pub edges: Option<Vec<EdgeConfig>>,
        pub user_variables: Option<Vec<UserVariables>>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct GetWorkflowDetailResp {
        pub workflow_id: u64,
        pub version_id: Option<u64>,
        pub workflow_name: String,
        pub workflow_info: String,
        pub version: Option<String>,
        pub version_info: Option<String>,
        pub updated_time: String,
        pub created_user: String,
        pub nodes: Option<Vec<NodeConfig>>,
        pub edges: Option<Vec<EdgeConfig>>,
        pub user_variables: Option<Vec<UserVariables>>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct StartProcessReq {
        pub workflow_id: u64,
        pub version_id: u64,
        pub process_name: String,
        pub process_args: Option<WorkflowProcessArgs>,
    }

    #[derive(Default, Serialize, Deserialize, Object)]
    pub struct WorkflowNodeArgs {
        pub node_id: String,
        pub target: Option<Vec<String>>,
        pub args: Option<Vec<WorkflowJobArgs>>,
    }

    #[derive(Default, Serialize, Deserialize, Object)]
    pub struct WorkflowProcessArgs {
        pub default_target: Option<Vec<String>>,
        pub user_variables: Option<Vec<UserVariables>>,
        pub nodes: Option<Vec<WorkflowNodeArgs>>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct StartProcessResp {
        pub process_id: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct GetWorkflowProcessDetailResp {
        pub process_id: String,
        pub process_name: String,
        pub created_user: String,
        pub current_run_id: String,
        pub current_node_id: String,
        pub current_node_status: String,
        pub process_status: String,
        pub origin_nodes: Option<serde_json::Value>,
        pub origin_edges: Option<serde_json::Value>,
        pub process_args: Option<serde_json::Value>,
        pub completed_nodes: Vec<WorkflowProcessCompletedNode>,
        pub completed_edges: Vec<WorkflowProcessCompletedEdge>,
    }

    #[derive(Default, Object, Serialize, Deserialize, Clone)]
    pub struct WorkflowProcessCompletedNode {
        pub base: WorkflowProcessNodeRecord,
        pub tasks: Vec<WorkflowProcessNodeTaskRecord>,
    }

    #[derive(Default, Object, Serialize, Deserialize, Clone)]
    pub struct WorkflowProcessNodeRecord {
        pub id: u64,
        pub process_id: String,
        pub run_id: String,
        pub node_id: String,
        pub node_status: String,
        pub node_args: Option<serde_json::Value>,
        pub created_user: String,
        pub created_time: String,
        pub updated_time: String,
    }

    #[derive(Default, Object, Serialize, Deserialize, Clone)]
    pub struct WorkflowProcessNodeTaskRecord {
        pub id: u64,
        pub process_id: String,
        pub node_id: String,
        pub run_id: String,
        pub task_status: String,
        pub bind_ip: String,
        pub exit_code: i64,
        pub exit_status: String,
        pub output: String,
        pub restart_num: i64,
        pub dispatch_result: Option<serde_json::Value>,
        pub created_user: String,
        pub created_time: String,
        pub updated_time: String,
    }

    #[derive(Default, Object, Serialize, Deserialize, Clone)]
    pub struct WorkflowProcessCompletedEdge {
        pub base: WorkflowProcessEdgeRecord,
    }

    #[derive(Default, Object, Serialize, Deserialize, Clone)]
    pub struct WorkflowProcessEdgeRecord {
        pub id: u64,
        pub process_id: String,
        pub run_id: String,
        pub edge_id: String,
        pub edge_type: String,
        pub eval_val: String,
        pub props: Option<serde_json::Value>,
        pub source_node_id: String,
        pub target_node_id: String,
        pub created_user: String,
        pub created_time: String,
    }
    #[derive(Default, Object, Serialize)]
    pub struct WorkflowProcessRecord {
        pub workflow_id: u64,
        pub workflow_name: String,
        pub timer_id: u64,
        pub timer_name: Option<String>,
        pub version_id: u64,
        pub version: String,
        pub process_id: String,
        pub process_name: String,
        pub created_user: String,
        pub current_run_id: String,
        pub current_node_name: String,
        pub current_node_id: String,
        pub current_node_status: String,
        pub process_status: String,
        pub team_name: Option<String>,
        pub team_id: Option<u64>,
        pub tags: Option<Vec<ResourceTag>>,
        pub created_time: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryWorkflowProcessResp {
        pub total: u64,
        pub list: Vec<WorkflowProcessRecord>,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteWorkflowReq {
        pub workflow_id: u64,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteWorkflowResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteProcessReq {
        pub workflow_id: Option<u64>,
        pub process_id: Option<String>,
        pub is_soft: Option<bool>,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteProcessResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteVersionReq {
        pub workflow_id: u64,
        pub version_id: u64,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteVersionResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SaveWorkflowTimerReq {
        pub id: Option<u64>,
        pub name: String,
        pub workflow_id: u64,
        pub version_id: u64,
        pub timer_expr: CustomTimerExpr,
        pub info: Option<String>,
        pub process_args: Option<WorkflowProcessArgs>,
    }

    impl From<logic::workflow::types::WorkflowProcessArgs> for WorkflowProcessArgs {
        fn from(value: logic::workflow::types::WorkflowProcessArgs) -> Self {
            Self {
                default_target: value.default_target,
                user_variables: value.user_variables.map(|v| {
                    v.into_iter()
                        .map(|v| UserVariables {
                            name: v.name,
                            val: v.val,
                            info: v.info,
                        })
                        .collect()
                }),
                nodes: value.nodes.map_or(None, |v| {
                    Some(
                        v.into_iter()
                            .map(|v| WorkflowNodeArgs {
                                node_id: v.node_id,
                                target: v.target,
                                args: v.args.map(|v| {
                                    v.into_iter()
                                        .map(|v| WorkflowJobArgs {
                                            name: v.name,
                                            val: v.val,
                                            val_type: v.val_type,
                                            info: v.info,
                                        })
                                        .collect()
                                }),
                            })
                            .collect(),
                    )
                }),
            }
        }
    }

    impl Into<logic::workflow::types::WorkflowProcessArgs> for WorkflowProcessArgs {
        fn into(self) -> logic::workflow::types::WorkflowProcessArgs {
            logic::workflow::types::WorkflowProcessArgs {
                default_target: self.default_target,
                nodes: self.nodes.map_or(None, |v| {
                    Some(
                        v.into_iter()
                            .map(|v| logic::workflow::types::WorkflowNodeArgs {
                                node_id: v.node_id,
                                target: v.target,
                                args: v.args.map(|v| {
                                    v.into_iter()
                                        .map(|v| logic::workflow::types::WorkflowJobArgs {
                                            name: v.name,
                                            val: v.val,
                                            val_type: v.val_type,
                                            info: v.info,
                                        })
                                        .collect()
                                }),
                            })
                            .collect(),
                    )
                }),
                user_variables: self.user_variables.map(|v| {
                    v.into_iter()
                        .map(|v| logic::workflow::types::UserVariables {
                            name: v.name,
                            val: v.val,
                            info: v.info,
                        })
                        .collect()
                }),
            }
        }
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct CustomTimerExpr {
        pub timezone: String,
        pub expr: String,
    }

    #[derive(Object, Deserialize, Serialize, Default)]
    pub struct SaveWorkflowTimerResp {
        pub result: u64,
        pub next_exec_times: Vec<String>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryWorkflowTimerResp {
        pub total: u64,
        pub list: Vec<WorkflowTimerRecord>,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct WorkflowTimerRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub workflow_id: u64,
        pub workflow_name: String,
        pub version_id: u64,
        pub timer_expr: serde_json::Value,
        pub schedule_guid: String,
        pub startup_error: String,
        pub is_active: bool,
        pub tags: Option<Vec<ResourceTag>>,
        pub team_id: u64,
        pub team_name: Option<String>,
        pub process_args: Option<WorkflowProcessArgs>,
        pub created_user: String,
        pub updated_user: String,
        pub created_time: String,
        pub updated_time: String,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteTimerReq {
        pub id: u64,
        pub is_soft: Option<bool>,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DeleteTimerResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct ScheduleTimerReq {
        pub id: u64,
        pub action: String,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct ScheduleTimerResp {
        pub result: String,
    }
}

fn set_middleware(ep: impl Endpoint) -> impl Endpoint {
    ep.with(middleware::TeamPermissionMiddleware)
}
pub struct WorkflowApi;

#[OpenApi(prefix_path = "/workflow", tag = super::Tag::Workflow)]
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

        let nodes: Option<Vec<logic::workflow::types::NodeConfig>> = req
            .nodes
            .map(|v| v.into_iter().map(|v| v.try_into()).collect())
            .transpose()?;
        let edges: Option<Vec<logic::workflow::types::EdgeConfig>> = req
            .edges
            .map(|v| v.into_iter().map(|v| v.try_into()).collect())
            .transpose()?;

        let ret = svc
            .workflow
            .save_workflow(
                req.id,
                &user_info,
                req.name,
                req.info,
                team_id,
                nodes,
                edges,
                req.user_variables.map(|v| {
                    v.iter()
                        .map(|v| logic::workflow::types::UserVariables {
                            name: v.name.to_string(),
                            val: v.val.to_string(),
                            info: v.info.to_string(),
                        })
                        .collect()
                }),
            )
            .await?;

        return_ok!(types::SaveWorkflowResp { result: ret })
    }

    #[oai(path = "/release", method = "post")]
    pub async fn release_version(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::ReleaseWorkflowVersionReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::SaveWorkflowVersionResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, Some(req.workflow_id))
            .await?
        {
            return_err!("no permission");
        }

        let nodes: Option<Vec<logic::workflow::types::NodeConfig>> = req
            .nodes
            .map(|v| v.into_iter().map(|v| v.try_into()).collect())
            .transpose()?;
        let edges: Option<Vec<logic::workflow::types::EdgeConfig>> = req
            .edges
            .map(|v| v.into_iter().map(|v| v.try_into()).collect())
            .transpose()?;
        let ret = svc
            .workflow
            .release_version(
                req.workflow_id,
                &user_info,
                req.version,
                req.version_info,
                nodes,
                edges,
                req.user_variables.map(|v| {
                    v.into_iter()
                        .map(|v| logic::workflow::types::UserVariables {
                            name: v.name,
                            val: v.val,
                            info: v.info,
                        })
                        .collect()
                }),
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
        Query(tag_ids): Query<Option<Vec<u64>>>,
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
                tag_ids,
                name,
                page,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_resource_ids(
                ret.0.iter().map(|v| v.id).collect(),
                logic::types::ResourceType::Workflow,
            )
            .await?;

        let list = ret
            .0
            .into_iter()
            .map(|v| types::WorkflowRecord {
                id: v.id,
                name: v.name,
                info: v.info,
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.id {
                                Some(types::ResourceTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
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
        Query(id): Query<Option<u64>>,
        #[oai(name = "X-Team-Id")] Header(_team_id): Header<Option<u64>>,
        #[oai(default)] Query(version): Query<Option<String>>,
    ) -> api_response!(types::QueryWorkflowVersionResp) {
        let svc = state.service();
        let ret = svc
            .workflow
            .get_workflow_version_list(
                &user_info,
                version,
                username,
                workflow_id,
                id,
                page,
                page_size,
            )
            .await?;
        let list = ret
            .0
            .into_iter()
            .map::<Result<types::WorkflowVersionRecord>, _>(|v| {
                let nodes = v
                    .nodes
                    .map(|v| serde_json::from_value::<Vec<NodeConfig>>(v))
                    .transpose()?
                    .map(|v| {
                        v.into_iter()
                            .map(|v| types::NodeConfig::try_from(v))
                            .collect()
                    })
                    .transpose()?;
                let edges = v
                    .edges
                    .map(|v| serde_json::from_value::<Vec<EdgeConfig>>(v))
                    .transpose()?
                    .map(|v| {
                        v.into_iter()
                            .map(|v| types::EdgeConfig::try_from(v))
                            .collect()
                    })
                    .transpose()?;

                let user_variables = v
                    .user_variables
                    .map(|v| {
                        serde_json::from_value::<Vec<logic::workflow::types::UserVariables>>(v)
                    })
                    .transpose()
                    .context("failed convert user variables data")?
                    .map(|v| {
                        v.into_iter()
                            .map(|v| UserVariables {
                                name: v.name,
                                val: v.val,
                                info: v.info,
                            })
                            .collect()
                    });

                Ok(types::WorkflowVersionRecord {
                    id: v.id,
                    workflow_id: v.workflow_id,
                    version: v.version,
                    version_info: v.version_info,
                    nodes,
                    edges,
                    created_time: local_time!(v.created_time),
                    created_user: v.created_user,
                    user_variables,
                })
            })
            .collect::<Result<_>>()?;

        return_ok!(types::QueryWorkflowVersionResp { total: ret.1, list })
    }

    #[oai(path = "/detail", method = "get", transform = "set_middleware")]
    pub async fn get_workflow_detail(
        &self,
        state: Data<&AppState>,
        _user_info: Data<&logic::types::UserInfo>,
        Query(workflow_id): Query<u64>,
        Query(version_id): Query<Option<u64>>,
        #[oai(name = "X-Team-Id")] Header(_team_id): Header<Option<u64>>,
    ) -> api_response!(types::GetWorkflowDetailResp) {
        let svc = state.service();
        let ret = svc
            .workflow
            .get_workflow_detail(workflow_id, version_id)
            .await?;
        let nodes = ret
            .nodes
            .map(|v| serde_json::from_value::<Vec<logic::workflow::types::NodeConfig>>(v))
            .transpose()
            .context("failed convert node data")?
            .map(|v| v.into_iter().map(|v| v.try_into()).collect())
            .transpose()?;

        let edges = ret
            .edges
            .map(|v| serde_json::from_value::<Vec<logic::workflow::types::EdgeConfig>>(v))
            .transpose()
            .context("failed convert node data")?
            .map(|v| v.into_iter().map(|v| v.try_into()).collect())
            .transpose()?;

        let user_variables = ret
            .user_variables
            .map(|v| serde_json::from_value::<Vec<logic::workflow::types::UserVariables>>(v))
            .transpose()
            .context("failed convert user variables data")?
            .map(|v| {
                v.into_iter()
                    .map(|v| UserVariables {
                        name: v.name,
                        val: v.val,
                        info: v.info,
                    })
                    .collect()
            });

        return_ok!(types::GetWorkflowDetailResp {
            workflow_id: ret.workflow_id,
            version_id: ret.version_id,
            version: ret.version,
            version_info: ret.version_info,
            workflow_name: ret.workflow_name,
            workflow_info: ret.workflow_info,
            updated_time: local_time!(ret.updated_time),
            created_user: ret.created_user,
            nodes,
            edges,
            user_variables,
        })
    }

    #[oai(path = "/process/detail", method = "get")]
    pub async fn get_process_detail(
        &self,
        state: Data<&AppState>,
        _user_info: Data<&logic::types::UserInfo>,
        Query(process_id): Query<String>,
    ) -> api_response!(types::GetWorkflowProcessDetailResp) {
        let svc = state.service();
        let process_detail = svc.workflow.get_process_detail(process_id.clone()).await?;

        let resp = types::GetWorkflowProcessDetailResp {
            process_id,
            process_name: process_detail.process_name,
            created_user: process_detail.created_user,
            current_run_id: process_detail.current_run_id,
            current_node_id: process_detail.current_node_id,
            current_node_status: process_detail.current_node_status,
            process_status: process_detail.process_status,
            origin_nodes: process_detail.origin_nodes,
            origin_edges: process_detail.origin_edges,
            process_args: process_detail.process_args,
            completed_nodes: process_detail
                .completed_nodes
                .into_iter()
                .map(|v| types::WorkflowProcessCompletedNode {
                    base: types::WorkflowProcessNodeRecord {
                        id: v.base.id,
                        process_id: v.base.process_id,
                        run_id: v.base.run_id,
                        node_id: v.base.node_id,
                        node_status: v.base.node_status,
                        node_args: v.base.node_args,
                        created_user: v.base.created_user,
                        created_time: local_time!(v.base.created_time),
                        updated_time: local_time!(v.base.updated_time),
                    },
                    tasks: v
                        .tasks
                        .into_iter()
                        .map(|v| types::WorkflowProcessNodeTaskRecord {
                            id: v.id,
                            process_id: v.process_id,
                            node_id: v.node_id,
                            run_id: v.run_id,
                            task_status: v.task_status,
                            bind_ip: v.bind_ip,
                            exit_code: v.exit_code,
                            exit_status: v.exit_status,
                            output: v.output,
                            restart_num: v.restart_num,
                            dispatch_result: v.dispatch_result,
                            created_user: v.created_user,
                            created_time: local_time!(v.created_time),
                            updated_time: local_time!((v.updated_time)),
                        })
                        .collect(),
                })
                .collect(),
            completed_edges: process_detail
                .completed_edges
                .into_iter()
                .map(|v| types::WorkflowProcessCompletedEdge {
                    base: types::WorkflowProcessEdgeRecord {
                        id: v.base.id,
                        process_id: v.base.process_id,
                        run_id: v.base.run_id,
                        edge_id: v.base.edge_id,
                        edge_type: v.base.edge_type,
                        eval_val: v.base.eval_val,
                        props: v.base.props,
                        source_node_id: v.base.source_node_id,
                        target_node_id: v.base.target_node_id,
                        created_user: v.base.created_user,
                        created_time: local_time!(v.base.created_time),
                    },
                })
                .collect(),
        };

        return_ok!(resp)
    }

    #[oai(path = "/process/list", method = "get")]
    pub async fn query_process(
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
        Query(process_name): Query<Option<String>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::QueryWorkflowProcessResp) {
        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };

        let svc = state.service();
        let ret = svc
            .workflow
            .get_workflow_process_list(
                &user_info,
                search_username.as_deref(),
                default_id,
                team_id,
                tag_ids,
                process_name,
                page,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_resource_ids(
                ret.0.iter().map(|v| v.workflow_id).collect(),
                logic::types::ResourceType::Workflow,
            )
            .await?;

        let list = ret
            .0
            .into_iter()
            .map(|v| {
                let nodes: Option<Vec<NodeConfig>> = v
                    .workflow_nodes
                    .map_or(None, |v| Some(serde_json::from_value(v)))
                    .transpose()
                    .unwrap_or(None);

                let current_node = nodes
                    .iter()
                    .find_map(|arr| arr.iter().find(|node| node.id == v.current_node_id));

                types::WorkflowProcessRecord {
                    workflow_id: v.workflow_id,
                    workflow_name: v.workflow_name,
                    version_id: v.version_id,
                    version: v.version,
                    timer_id: v.timer_id,
                    timer_name: v.timer_name,
                    process_id: v.process_id,
                    process_name: v.process_name,
                    created_user: v.created_user,
                    current_run_id: v.current_run_id,
                    current_node_id: v.current_node_id,
                    current_node_name: current_node
                        .map_or("".to_string(), |node| node.name.to_string()),
                    current_node_status: v.current_node_status,
                    process_status: v.process_status,
                    tags: Some(
                        tag_records
                            .iter()
                            .filter_map(|tb| {
                                if tb.resource_id == v.workflow_id {
                                    Some(types::ResourceTag {
                                        id: tb.tag_id,
                                        tag_name: tb.tag_name.clone(),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    ),
                    team_id: v.team_id,
                    team_name: v.team_name,
                    created_time: local_time!(v.created_time),
                }
            })
            .collect();

        return_ok!(types::QueryWorkflowProcessResp { total: ret.1, list })
    }

    #[oai(path = "/start-process", method = "post")]
    pub async fn start_process(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::StartProcessReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::StartProcessResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, Some(req.workflow_id))
            .await?
        {
            return_err!("no permission");
        }

        let process_args = req.process_args.map(|v| v.into());

        let process_id = svc
            .workflow
            .start_process(
                &user_info,
                req.workflow_id,
                req.version_id,
                None,
                req.process_name,
                process_args,
            )
            .await?;
        return_ok!(types::StartProcessResp { process_id })
    }

    #[oai(path = "/delete", method = "post")]
    async fn delete_workflow(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::DeleteWorkflowReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::DeleteWorkflowResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, Some(req.workflow_id))
            .await?
        {
            return_err!("no permission");
        }

        let ret = svc
            .workflow
            .delete_workflow(&user_info, req.workflow_id)
            .await?;
        return_ok!(types::DeleteWorkflowResp { result: ret })
    }

    #[oai(path = "/delete-process", method = "post")]
    async fn delete_process(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::DeleteProcessReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::DeleteProcessResp) {
        let svc = state.service();
        let username = if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, req.workflow_id)
            .await?
            || team_id == Some(0)
            || team_id == None
        {
            Some(user_info.username.clone())
        } else {
            None
        };

        let ret = svc
            .workflow
            .delete_process(
                &user_info,
                username,
                req.workflow_id,
                req.process_id,
                team_id,
                None,
            )
            .await?;
        return_ok!(types::DeleteProcessResp { result: ret })
    }

    #[oai(path = "/delete-version", method = "post")]
    async fn delete_version(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::DeleteVersionReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::DeleteVersionResp) {
        let svc = state.service();

        if !svc
            .workflow
            .can_write_workflow(&user_info, team_id, Some(req.workflow_id))
            .await?
        {
            return_err!("no permission");
        }

        let ret = svc
            .workflow
            .delete_version(&user_info, req.workflow_id, req.version_id)
            .await?;
        return_ok!(types::DeleteVersionResp { result: ret })
    }

    #[oai(path = "/timer/save", method = "post")]
    pub async fn save_timer(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveWorkflowTimerReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::SaveWorkflowTimerResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_timer(&user_info, team_id, req.id)
            .await?
        {
            return_err!("no permission");
        }

        let next_exec_times =
            WorkflowLogic::check_timer_expr(&req.timer_expr.timezone, &req.timer_expr.expr)?;

        let process_args: Option<logic::workflow::types::WorkflowProcessArgs> =
            req.process_args.map(|v| v.into());

        let ret = svc
            .workflow
            .save_timer(
                &user_info,
                workflow_timer::ActiveModel {
                    id: req.id.map_or(NotSet, |v| Set(v)),
                    workflow_id: Set(req.workflow_id),
                    version_id: Set(req.version_id),
                    name: Set(req.name),
                    info: Set(req.info.unwrap_or_default()),
                    timer_expr: Set(serde_json::to_value(&req.timer_expr)
                        .expect("failed encode timer_expr to json")),
                    created_user: req.id.map_or(Set(user_info.username.clone()), |_| NotSet),
                    updated_user: Set(user_info.username.clone()),
                    process_args: process_args.map_or(NotSet, |v| {
                        Set(Some(
                            serde_json::to_value(&v).expect("failed encode process args to json"),
                        ))
                    }),
                    ..Default::default()
                },
            )
            .await?;
        return_ok!(types::SaveWorkflowTimerResp {
            result: ret,
            next_exec_times,
        })
    }

    #[oai(path = "/timer/list", method = "get")]
    pub async fn query_timer(
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
        Query(updated_time_range): Query<Option<Vec<String>>>,
        Query(search_username): Query<Option<String>>,
        Query(workflow_name): Query<Option<String>>,
        Query(name): Query<Option<String>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::QueryWorkflowTimerResp) {
        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };

        let svc = state.service();

        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));

        let ret = svc
            .workflow
            .get_timer_list(
                team_id,
                search_username.as_ref(),
                name,
                workflow_name,
                updated_time_range,
                tag_ids,
                page,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_resource_ids(
                ret.0.iter().map(|v| v.id).collect(),
                logic::types::ResourceType::Workflow,
            )
            .await?;

        let list = ret
            .0
            .into_iter()
            .map(|v| types::WorkflowTimerRecord {
                id: v.id,
                name: v.name,
                team_name: v.team_name,
                team_id: v.team_id,
                workflow_name: v.workflow_name,
                created_user: v.created_user,
                info: v.info,
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.id {
                                Some(types::ResourceTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                workflow_id: v.workflow_id,
                version_id: v.version_id,
                timer_expr: v.timer_expr,
                schedule_guid: v.schedule_guid,
                is_active: v.is_active,
                startup_error: v.startup_error,
                updated_user: v.updated_user,
                process_args: v
                    .process_args
                    .map(serde_json::from_value)
                    .transpose()
                    .expect("failed parse process args"),
                updated_time: local_time!(v.updated_time),
                created_time: local_time!(v.created_time),
            })
            .collect();

        return_ok!(types::QueryWorkflowTimerResp { total: ret.1, list })
    }

    #[oai(path = "/timer/delete", method = "post")]
    async fn delete_timer(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::DeleteTimerReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::DeleteTimerResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_timer(&user_info, team_id, Some(req.id))
            .await?
        {
            return_err!("no permission");
        }

        let ret = svc.workflow.delete_timer(&user_info, req.id).await?;
        return_ok!(types::DeleteTimerResp { result: ret })
    }

    #[oai(path = "/timer/schedule", method = "post")]
    async fn schedule_timer(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::ScheduleTimerReq>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::ScheduleTimerResp) {
        let svc = state.service();
        if !svc
            .workflow
            .can_write_timer(&user_info, team_id, Some(req.id))
            .await?
        {
            return_err!("no permission");
        }

        let ret = match req.action.as_ref() {
            "start_timer" => {
                svc.workflow
                    .send_timer_msg(WorkflowTimerTask::StartTimer(req.id))
                    .await?
            }
            "stop_timer" => {
                svc.workflow
                    .send_timer_msg(WorkflowTimerTask::StopTimer(req.id))
                    .await?
            }
            _ => return_err!(format!("not support {}", req.action)),
        };

        return_ok!(types::ScheduleTimerResp { result: ret });
    }
}
