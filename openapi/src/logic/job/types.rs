use automate::DispatchJobParams;
use sea_orm::{prelude::DateTimeUtc, FromQueryResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct RunStatusRelatedScheduleJobModel {
    pub id: u64,
    pub executor_id: u64,
    pub executor_name: String,
    pub instance_id: String,
    pub bind_ip: String,
    pub bind_namespace: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub schedule_type: String,
    pub job_type: String,
    pub eid: String,
    pub schedule_snapshot_data: Option<serde_json::Value>,
    pub schedule_id: String,
    pub schedule_status: String,
    pub schedule_name: Option<String>,
    pub run_status: String,
    pub exit_status: String,
    pub exit_code: i32,
    pub dispatch_data: Option<serde_json::Value>,
    pub dispatch_result: Option<serde_json::Value>,
    pub start_time: Option<DateTimeUtc>,
    pub end_time: Option<DateTimeUtc>,
    pub next_time: Option<DateTimeUtc>,
    pub prev_time: Option<DateTimeUtc>,
    pub updated_user: String,
    pub updated_time: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct ExecHistoryRelatedScheduleModel {
    pub id: u64,
    pub schedule_id: String,
    pub ip: String,
    pub namespace: String,
    pub job_type: String,
    pub output: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub bundle_script_result: Option<serde_json::Value>,
    pub created_user: String,
    pub exit_code: i64,
    pub exit_status: String,
    pub start_time: Option<DateTimeUtc>,
    pub end_time: Option<DateTimeUtc>,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
    pub schedule_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct JobRelatedExecutorModel {
    pub id: u64,
    pub eid: String,
    pub executor_id: u64,
    pub executor_name: String,
    pub executor_platform: String,
    pub executor_command: String,
    pub job_type: String,
    pub name: String,
    pub code: String,
    pub info: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub bundle_script: Option<serde_json::Value>,
    pub work_dir: String,
    pub work_user: String,
    pub upload_file: String,
    pub max_retry: u8,
    pub max_parallel: u8,
    pub timeout: u64,
    pub is_public: i8,
    pub created_user: String,
    pub updated_user: String,
    pub display_on_dashboard: bool,
    pub args: Option<serde_json::Value>,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct BundleScriptRelatedExecutorModel {
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
    pub args: Option<serde_json::Value>,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct JobTimerRelatedJobModel {
    pub id: u64,
    pub eid: String,
    pub name: String,
    pub job_name: String,
    pub job_type: String,
    pub executor_id: u64,
    pub executor_name: String,
    pub executor_platform: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub timer_expr: Option<serde_json::Value>,
    pub info: String,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchTarget {
    pub ip: String,
    pub namespace: String,
    pub mac_addr: String,
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchData {
    pub target: Vec<DispatchTarget>,
    pub params: DispatchJobParams,
}

impl TryFrom<Value> for DispatchData {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let data = serde_json::from_value(value)?;
        Ok(data)
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct ComputedBundleOutput {
    pub eid: String,
    pub expr: String,
    pub exit_code: Option<i32>,
    pub exit_status: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub result: bool,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct BundleScriptResult {
    pub name: String,
    pub eid: String,
    pub cond_expr: String,
    pub exit_code: Option<i32>,
    pub exit_status: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub eval_err: Option<String>,
    pub result: bool,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct BundleScriptRecord {
    pub eid: String,
    pub name: String,
    pub code: String,
    pub executor_id: u64,
    pub info: String,
    pub cond_expr: String,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct DispatchResult {
    pub namespace: String,
    pub bind_ip: String,
    pub instance_id: String,
    pub response: serde_json::Value,
    pub has_err: bool,
    pub err: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct JobRunResultStats {
    pub name: String,
    pub eid: String,
    pub schedule_name: String,
    pub last_start_time: String,
    pub results: Vec<RunResultSummary>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct RunResultSummary {
    pub name: String,
    pub eid: String,
    pub total: i64,
    pub info: String,
    pub last_start_time: String,
    pub exec_succ_num: i64,
    pub exec_fail_num: i64,
    pub check_succ_num: i64,
    pub check_fail_num: i64,
    pub eval_fail_num: i64,
}

#[derive(Default)]
pub struct JobStatSummary {
    pub total: u64,
    pub running_num: u64,
    pub exec_succ_num: u64,
    pub exec_fail_num: u64,
}

#[derive(Default)]
pub struct InstanceStatSummary {
    pub online: u64,
    pub offline: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct JobSupervisorRelatedJobModel {
    pub id: u64,
    pub name: String,
    pub job_name: String,
    pub restart_interval: u64,
    pub executor_id: u64,
    pub executor_name: String,
    pub executor_platform: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub eid: String,
    pub info: String,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct TeamMemberModel {
    pub user_id: String,
    pub username: String,
    pub is_admin: bool,
    pub created_time: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct ScheduleJobTeamModel {
    pub id: u64,
    pub schedule_id: String,
    pub name: String,
    pub job_type: String,
    pub eid: String,
    pub team_id: Option<u64>,
    pub team_name: Option<String>,
    pub dispatch_result: Option<serde_json::Value>,
    pub schedule_type: String,
    pub action: String,
    pub dispatch_data: Option<serde_json::Value>,
    pub snapshot_data: Option<serde_json::Value>,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
}
