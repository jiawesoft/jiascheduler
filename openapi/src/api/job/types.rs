use std::collections::HashMap;

use automate::scheduler::types;
use poem_openapi::{Enum, Object};

use crate::logic;
use serde::Serialize;
use serde_json::Value;

#[derive(Object, Serialize, Default)]
pub struct SaveJobResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
#[oai(skip_serializing_if_is_none)]
pub struct SaveJobReq {
    pub id: Option<u64>,
    pub eid: Option<String>,
    pub executor_id: u64,
    #[oai(validator(min_length = 1, max_length = 50))]
    pub name: String,
    pub work_user: Option<String>,
    pub work_dir: Option<String>,
    pub timeout: Option<u64>,
    pub max_retry: Option<u8>,
    pub max_parallel: Option<u8>,
    pub code: Option<String>,
    pub info: Option<String>,
    pub bundle_script: Option<Vec<BundleScript>>,
    pub upload_file: Option<String>,
    #[oai(default)]
    pub is_public: Option<bool>,
    pub display_on_dashboard: Option<bool>,
    pub args: Option<HashMap<String, String>>,
    pub completed_callback: Option<CompletedCallbackOpts>,
}

#[derive(Object, Serialize, Default)]
pub struct CompletedCallbackOpts {
    pub trigger_on: CompletedCallbackTriggerType,
    pub url: String,
    pub header: Option<HashMap<String, String>>,
    pub enable: bool,
}

impl From<logic::types::CompletedCallbackOpts> for CompletedCallbackOpts {
    fn from(value: logic::types::CompletedCallbackOpts) -> Self {
        let trigger_on = match value.trigger_on {
            logic::types::CompletedCallbackTriggerType::All => CompletedCallbackTriggerType::All,
            logic::types::CompletedCallbackTriggerType::Error => {
                CompletedCallbackTriggerType::Error
            }
        };
        Self {
            trigger_on,
            url: value.url,
            header: value.header,
            enable: value.enable,
        }
    }
}

impl Into<logic::types::CompletedCallbackOpts> for CompletedCallbackOpts {
    fn into(self) -> logic::types::CompletedCallbackOpts {
        let trigger_on = match self.trigger_on {
            CompletedCallbackTriggerType::All => logic::types::CompletedCallbackTriggerType::All,
            CompletedCallbackTriggerType::Error => {
                logic::types::CompletedCallbackTriggerType::Error
            }
        };
        logic::types::CompletedCallbackOpts {
            trigger_on,
            url: self.url,
            header: self.header,
            enable: self.enable,
        }
    }
}

#[derive(Enum, Serialize, Default)]
pub enum CompletedCallbackTriggerType {
    #[default]
    #[oai(rename = "all")]
    All,
    #[oai(rename = "error")]
    Error,
}

#[derive(Object, Serialize, Default)]
pub struct BundleScript {
    pub eid: String,
    pub name: String,
    pub info: String,
    pub executor_id: u64,
    pub code: String,
    pub cond_expr: String,
}

pub fn default_page() -> u64 {
    1
}

pub fn default_page_size() -> u64 {
    20
}

#[derive(Object, Serialize, Default)]
pub struct QueryJobResp {
    pub total: u64,
    pub list: Vec<JobRecord>,
}

#[derive(Object, Serialize, Default)]
pub struct JobRecord {
    pub id: u64,
    pub eid: String,
    pub executor_id: u64,
    pub executor_name: String,
    pub executor_platform: String,
    pub name: String,
    pub code: String,
    pub info: String,
    pub is_public: bool,
    pub job_type: String,
    pub team_name: Option<String>,
    pub team_id: Option<u64>,
    pub bundle_script: Option<Value>,
    pub tags: Option<Vec<JobTag>>,
    pub display_on_dashboard: bool,
    pub work_dir: String,
    pub work_user: String,
    pub timeout: u64,
    pub max_retry: u8,
    pub max_parallel: u8,
    pub created_user: String,
    pub updated_user: String,
    pub upload_file: String,
    pub args: Option<Value>,
    pub completed_callback: Option<CompletedCallbackOpts>,
    pub created_time: String,
    pub updated_time: String,
}

#[derive(Object, Serialize, Default)]
pub struct JobTag {
    pub id: u64,
    pub tag_name: String,
}

#[derive(Object, Serialize, Default)]
pub struct RunRecord {
    pub id: u64,
    pub executor_id: u64,
    pub executor_name: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub instance_id: String,
    pub bind_ip: String,
    pub bind_namespace: String,
    pub schedule_type: String,
    pub job_type: String,
    pub eid: String,
    pub schedule_id: String,
    pub schedule_snapshot_data: Option<serde_json::Value>,
    pub schedule_name: Option<String>,
    pub schedule_status: String,
    pub run_status: String,
    pub exit_status: String,
    pub exit_code: i32,
    pub dispatch_result: Option<serde_json::Value>,
    pub dispatch_data: Option<serde_json::Value>,
    pub tags: Option<Vec<JobTag>>,
    pub start_time: String,
    pub end_time: String,
    pub next_time: String,
    pub prev_time: String,
    pub updated_user: String,
    pub updated_time: String,
}

#[derive(Object, Serialize, Default)]
pub struct QueryRunResp {
    pub total: u64,
    pub list: Vec<RunRecord>,
}

#[derive(Object, Serialize, Default)]
pub struct Endpoint {
    pub instance_id: String,
}

#[derive(Object, Serialize, Default)]
pub struct DispatchJobReq {
    pub schedule_name: String,
    pub schedule_type: String,
    pub endpoints: Vec<Endpoint>,
    pub eid: String,
    pub timer_expr: Option<TimerExpr>,
    pub restart_interval: Option<u64>,
    pub is_sync: bool,
    pub action: String,
}

#[derive(Object, Serialize, Default)]
pub struct DispatchJobResp {
    pub result: u64,
}

pub type RedispatchJobResp = Vec<DispatchJobResult>;

#[derive(Object, Serialize, Default)]
pub struct DispatchJobResult {
    pub namespace: String,
    pub ip: String,
    pub response: serde_json::Value,
    pub has_err: bool,
    pub call_err: Option<String>,
}

#[derive(Object, Serialize, Default)]
pub struct RedispatchJobReq {
    pub schedule_id: String,
    pub action: String,
}
#[derive(Object, Serialize, Default)]
pub struct DeleteJobReq {
    pub eid: String,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteJobResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
pub struct ScheduleRecord {
    pub id: u64,
    pub schedule_id: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub name: String,
    pub eid: String,
    pub job_type: String,
    pub dispatch_result: Option<Value>,
    pub schedule_type: String,
    pub action: String,
    pub dispatch_data: Option<Value>,
    pub snapshot_data: Option<Value>,
    pub tags: Option<Vec<JobTag>>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: String,
    pub updated_time: String,
}

#[derive(Object, Serialize, Default)]
pub struct QueryScheduleResp {
    pub total: u64,
    pub list: Vec<ScheduleRecord>,
}

#[derive(Object, Serialize, Default)]
pub struct ExecRecord {
    pub id: u64,
    pub job_name: String,
    pub schedule_id: String,
    pub bind_ip: String,
    pub job_type: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub bundle_script_result: Option<serde_json::Value>,
    pub exit_status: String,
    pub exit_code: i64,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub tags: Option<Vec<JobTag>>,
    pub output: String,
    pub created_user: String,
    pub created_time: String,
    pub updated_time: String,
    pub schedule_name: String,
}

#[derive(Object, Serialize, Default)]
pub struct QueryExecResp {
    pub total: u64,
    pub list: Vec<ExecRecord>,
}

#[derive(Serialize, Default, Enum)]
pub enum JobAction {
    #[default]
    #[oai(rename = "exec")]
    Exec,
    #[oai(rename = "kill")]
    Kill,
    #[oai(rename = "start_timer")]
    StartTimer,
    #[oai(rename = "stop_timer")]
    StopTimer,
    #[oai(rename = "start_supervising")]
    StartSupervising,
    #[oai(rename = "stop_supervising")]
    StopSupervising,
}

impl Into<types::JobAction> for JobAction {
    fn into(self) -> types::JobAction {
        match self {
            JobAction::Exec => types::JobAction::Exec,
            JobAction::Kill => types::JobAction::Kill,
            JobAction::StartTimer => types::JobAction::StartTimer,
            JobAction::StopTimer => types::JobAction::StopTimer,
            JobAction::StartSupervising => types::JobAction::StartSupervising,
            JobAction::StopSupervising => types::JobAction::StopSupervising,
        }
    }
}

#[test]
fn test() {
    let m = JobAction::Exec;
    let s = serde_json::to_string(&m).unwrap();
    println!("{}", s);
}

#[derive(Object, Serialize, Default)]
pub struct ActionReq {
    pub action: JobAction,
    pub instance_id: String,
    pub schedule_id: String,
}

#[derive(Object, Serialize, Default)]
pub struct ActionRes {
    pub result: Value,
}

#[derive(Object, Serialize, Default)]
#[oai(skip_serializing_if_is_none)]
pub struct SaveJobBundleScriptReq {
    pub id: Option<u64>,
    pub eid: Option<String>,
    pub executor_id: u64,
    #[oai(validator(min_length = 1))]
    pub name: String,
    pub code: String,
    pub info: String,
    pub args: Option<HashMap<String, String>>,
}

#[derive(Object, Serialize, Default)]
pub struct SaveJobBundleScriptResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteJobBundleScriptReq {
    pub eid: String,
}

#[derive(Object, Serialize, Default)]
pub struct JobBundleScriptRecord {
    pub id: u64,
    pub eid: String,
    pub executor_id: u64,
    pub executor_name: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub name: String,
    pub code: String,
    pub info: String,
    pub created_user: String,
    pub updated_user: String,
    pub args: Option<Value>,
    pub created_time: String,
    pub updated_time: String,
}

#[derive(Object, Serialize, Default)]
pub struct QueryJobBundleScriptResp {
    pub total: u64,
    pub list: Vec<JobBundleScriptRecord>,
}

#[derive(Object, Serialize, Default)]
pub struct JobTimerRecord {
    pub id: u64,
    pub eid: String,
    pub name: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub job_name: String,
    pub job_type: String,
    pub executor_id: u64,
    pub executor_name: String,
    pub executor_platform: String,
    pub timer_expr: serde_json::Value,
    pub info: String,
    pub tags: Option<Vec<JobTag>>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: String,
    pub updated_time: String,
}

#[derive(Object, Serialize, Default)]
pub struct QueryJobTimerResp {
    pub total: u64,
    pub list: Vec<JobTimerRecord>,
}

#[derive(Object, Serialize, Default)]
#[oai(skip_serializing_if_is_none)]
pub struct SaveJobTimerReq {
    pub id: Option<u64>,
    pub eid: String,
    pub job_type: String,
    #[oai(validator(min_length = 1, max_length = 50))]
    pub name: String,
    pub timer_expr: TimerExpr,
    pub info: String,
}

#[derive(Object, Serialize, Default)]
pub struct TimerExpr {
    pub second: String,
    pub minute: String,
    pub hour: String,
    pub day_of_month: String,
    pub month: String,
    pub year: String,
}

impl From<String> for TimerExpr {
    fn from(value: String) -> Self {
        let vec: Vec<&str> = value.split(" ").collect();
        Self {
            second: vec.get(0).map_or("1".to_string(), |&v| v.to_string()),
            minute: vec.get(1).map_or("1".to_string(), |&v| v.to_string()),
            hour: vec.get(2).map_or("1".to_string(), |&v| v.to_string()),
            day_of_month: vec.get(3).map_or("1".to_string(), |&v| v.to_string()),
            month: vec.get(4).map_or("1".to_string(), |&v| v.to_string()),
            year: vec.get(5).map_or("1".to_string(), |&v| v.to_string()),
        }
    }
}

impl Into<String> for TimerExpr {
    fn into(self) -> String {
        format!(
            "{} {} {} {} {} {}",
            self.second, self.minute, self.hour, self.day_of_month, self.month, self.year
        )
    }
}

#[derive(Object, Serialize, Default)]
pub struct SaveJobTimerResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
pub struct GetDashboardReq {
    // pub eid: String,
    pub job_type: String,
    pub filter_schedule_history: Vec<FilterScheduleHistoryRule>,
}

#[derive(Object, Serialize, Default)]
pub struct FilterScheduleHistoryRule {
    eid: String,
    schedule_id: String,
}
#[derive(Object, Serialize, Default)]
pub struct GetDashboardResp {
    pub job_num: u64,
    pub running_num: u64,
    pub exec_succ_num: u64,
    pub exec_fail_num: u64,
    pub rows: Vec<JobRunResultStats>,
}

#[derive(Object, Serialize, Default)]
pub struct JobRunResultStats {
    pub name: String,
    pub eid: String,
    pub schedule_name: String,
    pub results: Vec<JobRunSummary>,
}

#[derive(Object, Serialize, Default)]
pub struct JobRunSummary {
    pub eid: String,
    pub total: i64,
    pub name: String,
    pub info: String,
    pub last_start_time: String,
    pub exec_succ_num: i64,
    pub exec_fail_num: i64,
    pub check_succ_num: i64,
    pub check_fail_num: i64,
    pub eval_fail_num: i64,
}

#[derive(Object, Serialize, Default)]
pub struct JobSupervisorRecord {
    pub id: u64,
    pub name: String,
    pub job_name: String,
    pub eid: String,
    pub executor_id: u64,
    pub executor_name: String,
    pub executor_platform: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub restart_interval: u64,
    pub info: String,
    pub tags: Option<Vec<JobTag>>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: String,
    pub updated_time: String,
}

#[derive(Object, Serialize, Default)]
pub struct QueryJobSupervisorResp {
    pub total: u64,
    pub list: Vec<JobSupervisorRecord>,
}

#[derive(Object, Serialize, Default)]
#[oai(skip_serializing_if_is_none)]
pub struct SaveJobSupervisorReq {
    pub id: Option<u64>,
    pub eid: String,
    pub restart_interval: u64,
    #[oai(validator(min_length = 1, max_length = 50))]
    pub name: String,
    #[oai(validator(min_length = 0, max_length = 500))]
    pub info: String,
}

#[derive(Object, Serialize, Default)]
pub struct SaveJobSupervisorResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteExecHistoryReq {
    pub eid: Option<String>,
    pub schedule_id: Option<String>,
    pub ids: Option<Vec<u64>>,
    pub instance_id: Option<String>,
    pub time_range_start: Option<String>,
    pub time_range_end: Option<String>,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteExecHistoryResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteJobSupervisorReq {
    pub id: u64,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteJobSupervisorResp {
    pub result: u64,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteJobTimerReq {
    pub id: u64,
}

#[derive(Object, Serialize, Default)]
pub struct DeleteJobTimerResp {
    pub result: u64,
}
