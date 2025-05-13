use std::{collections::HashMap, fmt, process::Output};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Copy)]
pub enum JobAction {
    Exec,
    Kill,
    StartTimer,
    StopTimer,
    StartSupervising,
    RestartSupervising,
    StopSupervising,
}

impl TryFrom<&str> for JobAction {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let action = match value {
            "exec" => JobAction::Exec,
            "kill" => JobAction::Kill,
            "start_timer" => JobAction::StartTimer,
            "stop_timer" => JobAction::StopTimer,
            "start_supervising" => JobAction::StartSupervising,
            "stop_supervising" => JobAction::StopSupervising,
            "restart_supervising" => JobAction::RestartSupervising,
            _ => return Err(anyhow!("invalid job action {value}")),
        };

        Ok(action)
    }
}

impl fmt::Display for JobAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobAction::Exec => write!(f, "exec"),
            JobAction::Kill => write!(f, "kill"),
            JobAction::StartTimer => write!(f, "start_timer"),
            JobAction::StopTimer => write!(f, "stop_timer"),
            JobAction::StartSupervising => write!(f, "start_supervising"),
            JobAction::RestartSupervising => write!(f, "restart_supervising"),
            JobAction::StopSupervising => write!(f, "stop_supervising"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum RuntimeAction {
    Kill,
    StopTimer,
    StartSupervising,
    RestartSupervising,
    StopSupervising,
}

impl fmt::Display for RuntimeAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeAction::Kill => write!(f, "kill"),
            RuntimeAction::StopTimer => write!(f, "stop_timer"),
            RuntimeAction::StartSupervising => write!(f, "start_supervising"),
            RuntimeAction::RestartSupervising => write!(f, "restart_supervising"),
            RuntimeAction::StopSupervising => write!(f, "stop_supervising"),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub enum RunStatus {
    #[default]
    Prepare,
    Running,
    Stop,
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunStatus::Prepare => write!(f, "prepare"),
            RunStatus::Running => write!(f, "running"),
            RunStatus::Stop => write!(f, "stop"),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub enum ScheduleStatus {
    #[default]
    Prepare,
    Supervising,
    Unsupervised,
    Scheduling,
    Unscheduled,
}

impl fmt::Display for ScheduleStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScheduleStatus::Prepare => write!(f, "prepare"),
            ScheduleStatus::Scheduling => write!(f, "scheduling"),
            ScheduleStatus::Unscheduled => write!(f, "unscheduled"),
            ScheduleStatus::Supervising => write!(f, "supervising"),
            ScheduleStatus::Unsupervised => write!(f, "unsupervised"),
        }
    }
}

#[derive(Default, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BaseJob {
    pub eid: String,
    pub cmd_name: String,
    pub code: String,
    pub bundle_script: Option<Vec<BundleScript>>,
    pub args: Vec<String>,
    pub upload_file: Option<UploadFile>,
    pub read_code_from_stdin: bool,
    pub timeout: u64,
    pub work_dir: Option<String>,
    pub work_user: Option<String>,
    pub max_retry: Option<u8>,
    pub max_parallel: Option<u32>,
}

impl BaseJob {
    /// remove upload_file and return pure job
    pub fn to_pure_job(&self) -> BaseJob {
        BaseJob {
            eid: self.eid.clone(),
            cmd_name: self.cmd_name.clone(),
            code: self.code.clone(),
            bundle_script: self.bundle_script.clone(),
            args: self.args.clone(),
            upload_file: None,
            read_code_from_stdin: self.read_code_from_stdin,
            timeout: self.timeout,
            work_dir: self.work_dir.clone(),
            work_user: self.work_user.clone(),
            max_retry: self.max_retry,
            max_parallel: self.max_parallel,
        }
    }
}

#[derive(Default, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BundleScript {
    pub eid: String,
    pub cmd_name: String,
    pub args: Vec<String>,
    pub code: String,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct UploadFile {
    pub filename: String,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, PartialEq, Deserialize, Default, Clone)]
pub enum ScheduleType {
    #[default]
    Once,
    Timer,
    Flow,
    Daemon,
}

impl TryFrom<&str> for ScheduleType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let schedule_type = match value {
            "once" => ScheduleType::Once,
            "flow" => ScheduleType::Flow,
            "timer" => ScheduleType::Timer,
            "daemon" => ScheduleType::Daemon,
            _ => return Err(anyhow!("invalid schedule type").into()),
        };
        Ok(schedule_type)
    }
}

impl fmt::Display for ScheduleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScheduleType::Once => write!(f, "once"),
            ScheduleType::Timer => write!(f, "timer"),
            ScheduleType::Flow => write!(f, "flow"),
            ScheduleType::Daemon => write!(f, "daemon"),
        }
    }
}

pub enum BundleOutput {
    Output(Output),
    Bundle(HashMap<String, Output>),
}

impl BundleOutput {
    pub fn get_exit_status(&self) -> Option<String> {
        match self {
            BundleOutput::Output(v) => Some(v.status.to_string()),
            BundleOutput::Bundle(_) => None,
        }
    }

    pub fn get_exit_code(&self) -> Option<i32> {
        match self {
            BundleOutput::Output(v) => {
                if v.status.success() {
                    v.status.code()
                } else {
                    // killed, return 9
                    v.status.code().or(Some(9))
                }
            }
            BundleOutput::Bundle(_) => None,
        }
    }

    pub fn get_stdout(&self) -> Option<String> {
        match self {
            BundleOutput::Output(v) => Some(String::from_utf8_lossy(&v.stdout).to_string()),
            BundleOutput::Bundle(_) => None,
        }
    }

    pub fn get_stderr(&self) -> Option<String> {
        match self {
            BundleOutput::Output(v) => Some(String::from_utf8_lossy(&v.stderr).to_string()),
            BundleOutput::Bundle(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshConnectionOption {
    pub user: String,
    pub password: String,
    pub port: u16,
}

impl SshConnectionOption {
    pub fn build(
        user: Option<String>,
        password: Option<String>,
        port: Option<u16>,
    ) -> Option<SshConnectionOption> {
        if let (Some(user), Some(password), Some(port)) = (user, password, port) {
            Some(SshConnectionOption {
                user,
                password,
                port,
            })
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssignUserOption {
    pub username: String,
    pub password: String,
}

impl AssignUserOption {
    pub fn build(username: Option<String>, password: Option<String>) -> Option<AssignUserOption> {
        if let (Some(username), Some(password)) = (username, password) {
            Some(AssignUserOption { username, password })
        } else {
            None
        }
    }
}
