use anyhow::{Result, anyhow};
use entity::{workflow_process_edge, workflow_process_node, workflow_process_node_task};
use redis_macros::{FromRedisValue, ToRedisArgs};
use sea_orm::{FromQueryResult, prelude::DateTimeLocal};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::logic::workflow::condition;
#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub enum NodeType {
    #[default]
    #[serde(rename = "bpmn:startEvent")]
    StartEvent,
    #[serde(rename = "bpmn:serviceTask")]
    ServiceTask,
    #[serde(rename = "bpmn:endEvent")]
    EndEvent,
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

#[derive(Default, Serialize, Deserialize, Clone)]
pub enum Task {
    #[serde(rename = "standard")]
    Standard(StandardJob),
    #[serde(rename = "custom")]
    Custom(CustomJob),
    #[serde(rename = "none")]
    #[default]
    None,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskType {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "none")]
    #[default]
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

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub enum NodeStatus {
    #[serde(rename = "prepare")]
    #[default]
    Prepare,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "end")]
    End,
    // #[serde(rename = "stop")]
    // Stop,
}

impl Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Prepare => write!(f, "prepare"),
            NodeStatus::Running => write!(f, "running"),
            NodeStatus::End => write!(f, "end"),
            // NodeStatus::Stop => write!(f, "stop"),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub enum ProcessStatus {
    #[default]
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "end")]
    End,
}

impl Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::End => write!(f, "end"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomJob {
    pub executor_id: u64,
    pub timeout: Option<u64>,
    pub code: String,
    pub formal_args: Vec<WorkflowJobArgs>,
    pub upload_file: Option<String>,
    pub target: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StandardJob {
    pub eid: String,
    pub formal_args: Vec<WorkflowJobArgs>,
    pub target: Option<Vec<String>>,
}

#[derive(Default, Clone, Serialize, Deserialize, FromRedisValue, ToRedisArgs)]
pub struct NodeConfig {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub task_type: TaskType,
    #[serde(default)]
    pub is_join_all: bool,
    pub task: Task,
    pub data: serde_json::Value,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct UserVariables {
    pub name: String,
    pub val: String,
    pub info: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    pub id: String,
    pub name: String,
    pub condition: Option<condition::Condition>,
    pub source_node_id: String,
    pub target_node_id: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct WorkflowModel {
    pub id: u64,
    pub name: String,
    pub info: String,
    pub team_id: u64,
    pub nodes: Option<serde_json::Value>,
    pub edges: Option<serde_json::Value>,
    pub user_variables: Option<serde_json::Value>,
    pub team_name: Option<String>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct WorkflowProcessModel {
    pub id: u64,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub timer_id: u64,
    pub timer_name: Option<String>,
    pub process_id: String,
    pub process_name: String,
    pub workflow_id: u64,
    pub workflow_name: String,
    pub workflow_nodes: Option<serde_json::Value>,
    pub version_id: u64,
    pub version: String,
    pub process_args: Option<serde_json::Value>,
    pub process_status: String,
    pub current_run_id: String,
    pub current_node_id: String,
    pub current_node_status: String,
    pub created_user: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Default)]
pub struct WorkflowVersionDetailModel {
    pub workflow_id: u64,
    pub version_id: Option<u64>,
    pub workflow_name: String,
    pub workflow_info: String,
    pub nodes: Option<serde_json::Value>,
    pub edges: Option<serde_json::Value>,
    pub user_variables: Option<serde_json::Value>,
    pub team_id: u64,
    pub version: Option<String>,
    pub version_info: Option<String>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct WorkflowJobArgs {
    pub name: String,
    pub val: String,
    pub val_type: String,
    pub info: Option<String>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct WorkflowNodeActualArgs {
    pub formal: Vec<WorkflowJobArgs>,
    pub args: Option<serde_json::Value>,
    pub code: String,
    pub target: Vec<String>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct WorkflowNodeArgs {
    pub node_id: String,
    pub target: Option<Vec<String>>,
    pub args: Option<Vec<WorkflowJobArgs>>,
}

#[derive(Default, Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone)]
pub struct WorkflowProcessArgs {
    pub default_target: Option<Vec<String>>,
    pub nodes: Option<Vec<WorkflowNodeArgs>>,
    pub user_variables: Option<Vec<UserVariables>>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct WorkflowProcessDetail {
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

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct WorkflowProcessCompletedNode {
    pub base: workflow_process_node::Model,
    pub tasks: Vec<workflow_process_node_task::Model>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct WorkflowProcessCompletedEdge {
    pub base: workflow_process_edge::Model,
}

#[derive(Default, Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone)]
pub struct WorkflowNode {
    pub created_user: String,
    pub process_id: String,
    pub run_id: String,
    pub origin_nodes: Vec<NodeConfig>,
    pub origin_edges: Vec<EdgeConfig>,
    pub process_args: Option<WorkflowProcessArgs>,
    pub flow_depth: u32,
    pub actual_args: Option<WorkflowNodeActualArgs>,
    pub reached_edge: Option<EdgeConfig>,
    pub current_node: NodeConfig,
}

impl WorkflowNode {
    pub fn get_next_node(&self) -> Option<&NodeConfig> {
        let Some(edge) = self.get_next_edge() else {
            return None;
        };
        self.origin_nodes
            .iter()
            .find(|&v| v.id == edge.target_node_id)
    }

    pub fn get_next_edge(&self) -> Option<&EdgeConfig> {
        self.origin_edges
            .iter()
            .find(|&v| v.source_node_id == self.current_node.id)
    }

    pub fn get_next_edges(&self) -> Vec<&EdgeConfig> {
        self.origin_edges
            .iter()
            .filter(|&v| v.source_node_id == self.current_node.id)
            .collect::<Vec<&EdgeConfig>>()
    }

    pub fn get_prev_edges(&self) -> Vec<&EdgeConfig> {
        self.origin_edges
            .iter()
            .filter(|&v| v.target_node_id == self.current_node.id)
            .collect::<Vec<&EdgeConfig>>()
    }

    pub fn get_next_node_by_edge(&self, edge: &EdgeConfig) -> Option<&NodeConfig> {
        self.origin_nodes
            .iter()
            .find(|&v| v.id == edge.target_node_id)
    }

    pub fn get_next_nodes(&self) -> Result<Vec<(&EdgeConfig, &NodeConfig)>> {
        let next_edges = self.get_next_edges();

        let data = next_edges
            .into_iter()
            .map::<Result<(&EdgeConfig, &NodeConfig)>, _>(|edge| {
                let next_nodes = self.get_next_node_by_edge(edge);
                let data = next_nodes.map_or(Err(anyhow!("cannot found next edge")), |node| {
                    Ok((edge, node))
                });
                data
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(data)
    }
}
