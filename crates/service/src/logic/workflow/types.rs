use std::fmt::Display;

use automate::DispatchJobParams;
use sea_orm::prelude::DateTimeLocal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Serialize, Deserialize, Clone)]
pub enum Task {
    #[serde(rename = "job")]
    Job(String),
    #[serde(rename = "custom")]
    Custom(CustomJob),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum TaskType {
    #[serde(rename = "job")]
    Job,
    #[serde(rename = "custom")]
    Custom,
}

impl Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::Job => write!(f, "job"),
            TaskType::Custom => write!(f, "custom"),
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

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub task_type: TaskType,
    pub task: Task,
}
