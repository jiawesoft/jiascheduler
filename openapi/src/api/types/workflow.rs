use poem_openapi::{Enum, Object};

use crate::logic;
use serde::{Deserialize, Serialize};

use service::logic::workflow::condition;
use std::fmt::Display;

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
    pub timer_expr: super::job::CustomTimerExpr,
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
