use sea_orm::{FromQueryResult, prelude::DateTimeLocal};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Display;

#[derive(Serialize, Deserialize, Clone)]
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

impl TryFrom<&str> for NodeType {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "bpmn:startEvent" => Ok(NodeType::StartEvent),
            _ => Err(anyhow::anyhow!("Invalid node type")),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Task {
    #[serde(rename = "standard")]
    Standard(StandardJob),
    #[serde(rename = "custom")]
    Custom(CustomJob),
    #[serde(rename = "none")]
    None,
}

#[derive(Serialize, Deserialize, Clone)]
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

impl TryFrom<&str> for TaskType {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "standard" => Ok(TaskType::Standard),
            "custom" => Ok(TaskType::Custom),
            _ => Err(anyhow::anyhow!("Invalid task type")),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomJob {
    pub executor_id: u64,
    pub timeout: Option<u64>,
    pub code: String,
    pub upload_file: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StandardJob {
    pub eid: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub task_type: TaskType,
    pub task: Task,
    pub data: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    pub id: String,
    pub name: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct WorkflowModel {
    pub id: u64,
    pub pid: u64,
    pub name: String,
    pub info: String,
    pub team_id: u64,
    pub team_name: Option<String>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}
