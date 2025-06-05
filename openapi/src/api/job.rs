use std::time::Duration;

use crate::{
    api_response, default_local_time,
    entity::{job, job_bundle_script, job_supervisor},
    error::NoPermission,
    local_time,
    logic::{self, job::types::BundleScriptRecord},
    middleware,
    response::{std_into_error, ApiStdResponse},
    return_err, return_ok, AppState,
};

use service::IdGenerator;

use automate::{scheduler::types::ScheduleType, JobAction};
use poem::{session::Session, web::Data, Endpoint, EndpointExt, Result};
use poem_openapi::{
    param::{Header, Query},
    payload::Json,
    OpenApi,
};
use sea_orm::{ActiveValue::NotSet, Set};
use serde_json::json;
use types::CompletedCallbackOpts;
mod types {
    use std::collections::HashMap;

    use automate::scheduler::types;
    use poem_openapi::{Enum, Object};

    use serde::Serialize;
    use serde_json::Value;

    use crate::logic;

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
                logic::types::CompletedCallbackTriggerType::All => {
                    CompletedCallbackTriggerType::All
                }
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
                CompletedCallbackTriggerType::All => {
                    logic::types::CompletedCallbackTriggerType::All
                }
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
        pub is_online: bool,
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
        pub is_online: bool,
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
        pub schedule_type: Option<String>,
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
    pub struct DeleteRunStatusReq {
        pub eid: String,
        pub instance_id: String,
        pub schedule_type: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct DeleteRunStatusResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct DeleteScheduleHistoryReq {
        pub eid: String,
        pub schedule_id: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct DeleteScheduleHistoryResp {
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
}

fn set_middleware(ep: impl Endpoint) -> impl Endpoint {
    ep.with(middleware::TeamPermissionMiddleware)
}

pub struct JobApi;

#[OpenApi(prefix_path = "/job", tag = super::Tag::Job)]
impl JobApi {
    #[oai(path = "/save", method = "post", transform = "set_middleware")]
    pub async fn save_job(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveJobReq>,
    ) -> Result<ApiStdResponse<types::SaveJobResp>> {
        let ok = state.is_change_forbid(&user_info.user_id).await?;
        if ok {
            return Err(NoPermission().into());
        }
        let svc = state.service();

        if !svc
            .job
            .can_write_job_by_id(&user_info, team_id, req.id)
            .await?
        {
            return Err(NoPermission().into());
        }

        let args = req
            .args
            .map(|v| serde_json::to_value(&v))
            .transpose()
            .map_err(std_into_error)?;

        let completed_callback = if let Some(v) = req.completed_callback {
            let data: logic::types::CompletedCallbackOpts = v.into();
            Set(Some(serde_json::to_value(data).map_err(std_into_error)?))
        } else {
            NotSet
        };

        let (job_type, bundle_script) = match req.bundle_script {
            Some(v) => {
                let list: Vec<BundleScriptRecord> = v
                    .iter()
                    .map(|v| BundleScriptRecord {
                        executor_id: v.executor_id.clone(),
                        eid: v.eid.clone(),
                        name: v.name.clone(),
                        code: v.code.clone(),
                        info: v.info.clone(),
                        cond_expr: v.cond_expr.clone(),
                    })
                    .collect();

                (
                    Set("bundle".to_string()),
                    Set(Some(serde_json::to_value(&list).map_err(std_into_error)?)),
                )
            }
            None => (Set("default".to_string()), NotSet),
        };

        let (eid, id, created_user) = match req.id {
            Some(v) => (NotSet, Set(v), NotSet),
            None => (
                Set(IdGenerator::get_job_eid()),
                NotSet,
                Set(user_info.username.clone()),
            ),
        };

        let ret = svc
            .job
            .save_job(job::ActiveModel {
                id,
                eid,
                executor_id: Set(req.executor_id),
                name: Set(req.name),
                code: Set(req.code.unwrap_or_default()),
                info: Set(req.info.unwrap_or_default()),
                work_dir: Set(req.work_dir.unwrap_or_default()),
                work_user: Set(req.work_user.unwrap_or_default()),
                max_retry: Set(req.max_retry.unwrap_or(1)),
                max_parallel: Set(req.max_parallel.unwrap_or(1)),
                timeout: Set(req.timeout.unwrap_or(60)),
                bundle_script,
                job_type,
                upload_file: Set(req.upload_file.unwrap_or_default()),
                is_public: Set(req.is_public.map_or(0, |v| match v {
                    true => 1,
                    false => 0,
                })),
                display_on_dashboard: Set(req.display_on_dashboard.unwrap_or(false)),
                created_user,
                updated_user: Set(user_info.username.clone()),
                args: Set(args),
                team_id: team_id.map_or(NotSet, |v| Set(v)),
                completed_callback,
                ..Default::default()
            })
            .await?;

        return_ok!(types::SaveJobResp {
            result: ret.id.as_ref().to_owned()
        });
    }

    #[oai(path = "/list", method = "get", transform = "set_middleware")]
    pub async fn query_job(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,

        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        Query(default_id): Query<Option<u64>>,
        Query(default_eid): Query<Option<String>>,
        Query(search_username): Query<Option<String>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(updated_time_range): Query<Option<Vec<String>>>,
        #[oai(default)] Query(name): Query<Option<String>>,
        #[oai(default)] Query(job_type): Query<Option<String>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryJobResp>> {
        let svc = state.service();
        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let default_eid = default_eid.filter(|v| v != "");

        let team_id = svc
            .job
            .get_validate_team_id_by_job_or_default(&user_info, default_eid.as_deref(), team_id)
            .await?;

        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };

        let ret = svc
            .job
            .query_job(
                search_username,
                job_type.filter(|v| v != ""),
                name.filter(|v| v != ""),
                updated_time_range,
                default_id,
                default_eid,
                team_id,
                tag_ids,
                page - 1,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_job_ids(ret.0.iter().map(|v| v.id).collect())
            .await?;

        let list: Vec<types::JobRecord> = ret
            .0
            .into_iter()
            .map(|v| types::JobRecord {
                id: v.id,
                eid: v.eid,
                executor_id: v.executor_id,
                executor_name: v.executor_name,
                executor_platform: v.executor_platform,
                name: v.name,
                code: v.code,
                info: v.info,
                team_id: v.team_id,
                team_name: v.team_name,
                display_on_dashboard: v.display_on_dashboard,
                bundle_script: v.bundle_script,
                is_public: v.is_public == 1,
                job_type: v.job_type,
                created_user: v.created_user,
                updated_user: v.updated_user,
                args: v.args,
                completed_callback: v
                    .completed_callback
                    .map(|v| serde_json::from_value::<logic::types::CompletedCallbackOpts>(v))
                    .transpose()
                    .unwrap_or_default()
                    .map(|v| CompletedCallbackOpts::from(v)),
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.id {
                                Some(types::JobTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                work_dir: v.work_dir,
                work_user: v.work_user,
                timeout: v.timeout,
                max_retry: v.max_retry,
                max_parallel: v.max_parallel,
                upload_file: v.upload_file,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();
        return_ok!(types::QueryJobResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/delete", method = "post", transform = "set_middleware")]
    pub async fn delete_job(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteJobReq>,
    ) -> api_response!(types::DeleteJobResp) {
        let svc = state.service();
        if !svc
            .job
            .can_write_job(&user_info, team_id.clone(), Some(req.eid.clone()))
            .await?
        {
            return_err!("no permission to delete this job");
        }

        let result = svc.job.delete_job(&user_info, req.eid).await?;
        return_ok!(types::DeleteJobResp { result })
    }

    #[oai(path = "/dispatch", method = "post", transform = "set_middleware")]
    pub async fn dispatch(
        &self,
        state: Data<&AppState>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DispatchJobReq>,
        user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::DispatchJobResp>> {
        let svc = state.service();
        let action = req.action.as_str().try_into()?;
        let schedule_type = req.schedule_type.as_str().try_into()?;
        let secret = state.conf.comet_secret.clone();

        if !svc
            .job
            .can_dispatch_job(&user_info, team_id, None, &req.eid)
            .await?
        {
            return Err(NoPermission().into());
        }

        let ret = svc
            .job
            .dispatch_job(
                secret,
                req.endpoints.into_iter().map(|v| v.instance_id).collect(),
                req.eid,
                req.is_sync,
                req.schedule_name,
                schedule_type,
                action,
                req.timer_expr.map(|v| v.into()),
                req.restart_interval.map(|v| Duration::from_secs(v)),
                user_info.username.clone(),
            )
            .await?;
        return_ok!(types::DispatchJobResp { result: ret })
    }

    #[oai(path = "/redispatch", method = "post", transform = "set_middleware")]
    pub async fn redispatch(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::RedispatchJobReq>,
    ) -> Result<ApiStdResponse<types::RedispatchJobResp>> {
        let svc = state.service();
        let action: JobAction = req.action.as_str().try_into()?;

        let schedule_record =
            svc.job
                .get_schedule(&req.schedule_id)
                .await?
                .ok_or(anyhow::anyhow!(
                    "cannot found job schedule by schedule_id: {}",
                    req.schedule_id
                ))?;

        if !svc
            .job
            .can_dispatch_job(
                &user_info,
                team_id,
                Some(&schedule_record.created_user),
                &schedule_record.eid,
            )
            .await?
        {
            return_err!(
                "Rescheduling is not allowed unless you are the task's original scheduler."
            );
        }

        let ret = svc
            .job
            .redispatch_job(
                &req.schedule_id,
                action,
                schedule_record,
                user_info.username.clone(),
            )
            .await?;

        let ret = ret
            .into_iter()
            .map(|v| match v {
                Ok(v) => types::DispatchJobResult {
                    namespace: v.namespace,
                    ip: v.bind_ip,
                    response: v.response,
                    has_err: v.has_err,
                    call_err: v.err,
                },
                Err(_) => unreachable!(),
            })
            .collect();

        return_ok!(ret)
    }

    #[oai(
        path = "/running-status-list",
        method = "get",
        transform = "set_middleware"
    )]
    pub async fn query_running_status_list(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,

        #[oai(default)] Query(bind_ip): Query<Option<String>>,
        #[oai(default)] Query(schedule_name): Query<Option<String>>,
        Query(search_username): Query<Option<String>>,
        #[oai(validator(
            custom = "super::OneOfValidator::new(vec![\"once\",\"timer\",\"flow\",\"daemon\"])"
        ))]
        Query(schedule_type): Query<String>,
        #[oai(validator(custom = "super::OneOfValidator::new(vec![\"bundle\", \"default\"])"))]
        Query(job_type): Query<String>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(updated_time_range): Query<Option<Vec<String>>>,

        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryRunResp>> {
        let svc = state.service();
        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };
        let ret = svc
            .job
            .query_run_list(
                search_username,
                bind_ip.filter(|v| v != ""),
                team_id,
                schedule_name.filter(|v| v != ""),
                Some(schedule_type),
                Some(job_type),
                updated_time_range,
                tag_ids,
                page - 1,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_job_ids(ret.0.iter().map(|v| v.job_id).collect())
            .await?;

        let list: Vec<types::RunRecord> = ret
            .0
            .into_iter()
            .map(|v| types::RunRecord {
                id: v.id,
                instance_id: v.instance_id,
                is_online: v.is_online,
                eid: v.eid,
                executor_id: v.executor_id,
                executor_name: v.executor_name,
                team_id: v.team_id,
                team_name: v.team_name,
                updated_user: v.updated_user,
                updated_time: local_time!(v.updated_time),
                bind_ip: v.bind_ip,
                bind_namespace: v.bind_namespace,
                dispatch_data: v.dispatch_data.map(|mut v| {
                    if let Some(o) = v.as_object_mut() {
                        o.remove("target");
                        v
                    } else {
                        return v;
                    }
                }),
                schedule_type: v.schedule_type,
                schedule_id: v.schedule_id,
                schedule_name: v.schedule_name,
                schedule_status: v.schedule_status,
                schedule_snapshot_data: v.schedule_snapshot_data,
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.job_id {
                                Some(types::JobTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                run_status: v.run_status,
                exit_status: v.exit_status,
                exit_code: v.exit_code,
                job_type: v.job_type,
                dispatch_result: v.dispatch_result,
                start_time: v.start_time.map_or("".to_string(), |t| local_time!(t)),
                end_time: v.end_time.map_or("".to_string(), |t| local_time!(t)),
                next_time: v.next_time.map_or("".to_string(), |t| local_time!(t)),
                prev_time: v.prev_time.map_or("".to_string(), |t| local_time!(t)),
            })
            .collect();
        return_ok!(types::QueryRunResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/schedule-list", method = "get", transform = "set_middleware")]
    pub async fn query_schedule(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Query(search_username): Query<Option<String>>,
        #[oai(validator(
            custom = "super::OneOfValidator::new(vec![\"once\",\"timer\",\"flow\",\"daemon\"])"
        ))]
        Query(schedule_type): Query<Option<String>>,
        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(updated_time_range): Query<Option<Vec<String>>>,

        #[oai(default)] Query(name): Query<Option<String>>,
        #[oai(default)] Query(job_type): Query<String>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryScheduleResp>> {
        let svc = state.service();
        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };
        let ret = svc
            .job
            .query_schedule(
                schedule_type,
                search_username,
                job_type,
                name,
                team_id,
                updated_time_range,
                tag_ids,
                page - 1,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_job_ids(ret.0.iter().map(|v| v.job_id).collect())
            .await?;

        let list: Vec<types::ScheduleRecord> = ret
            .0
            .into_iter()
            .map(|v| types::ScheduleRecord {
                id: v.id,
                eid: v.eid,
                created_user: v.created_user,
                updated_user: v.updated_user,
                team_id: v.team_id,
                team_name: v.team_name,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
                schedule_type: v.schedule_type,
                schedule_id: v.schedule_id,
                name: v.name,
                job_type: v.job_type,
                dispatch_result: v.dispatch_result,
                action: v.action,
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.job_id {
                                Some(types::JobTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                dispatch_data: v.dispatch_data,
                snapshot_data: v.snapshot_data,
            })
            .collect();
        return_ok!(types::QueryScheduleResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/exec-list", method = "get", transform = "set_middleware")]
    pub async fn query_exec(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(default)] Query(bind_namespace): Query<Option<String>>,
        #[oai(default)] Query(bind_ip): Query<Option<String>>,
        #[oai(default)] Query(instance_id): Query<Option<String>>,
        Query(search_username): Query<Option<String>>,
        #[oai(validator(custom = "super::OneOfValidator::new(vec![\"bundle\", \"default\"])"))]
        Query(job_type): Query<String>,
        #[oai(default)] Query(schedule_name): Query<Option<String>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(validator(
            custom = "super::OneOfValidator::new(vec![\"once\",\"timer\",\"flow\",\"daemon\"])"
        ))]
        Query(schedule_type): Query<Option<String>>,
        #[oai(default)] Query(schedule_id): Query<Option<String>>,
        #[oai(default)] Query(eid): Query<Option<String>>,

        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(start_time_range): Query<Option<Vec<String>>>,

        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryExecResp>> {
        let start_time_range = start_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let svc = state.service();

        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };
        let ret = svc
            .job
            .query_exec_history(
                job_type,
                schedule_id.filter(|v| v != ""),
                schedule_type,
                team_id,
                eid,
                schedule_name,
                search_username,
                instance_id.filter(|v| v != ""),
                bind_namespace,
                bind_ip,
                start_time_range,
                tag_ids,
                page - 1,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_job_ids(ret.0.iter().map(|v| v.job_id).collect())
            .await?;

        let list: Vec<types::ExecRecord> = ret
            .0
            .into_iter()
            .map(|v| types::ExecRecord {
                id: v.id,
                schedule_id: v.schedule_id,
                bind_ip: v.ip,
                is_online: v.is_online,
                exit_status: v.exit_status,
                exit_code: v.exit_code,
                job_name: v.job_name,
                output: v.output,
                job_type: v.job_type,
                team_id: v.team_id,
                team_name: v.team_name,
                created_user: v.created_user,
                bundle_script_result: v.bundle_script_result,
                start_time: Some(default_local_time!(v.start_time)),
                end_time: Some(default_local_time!(v.end_time)),
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.job_id {
                                Some(types::JobTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
                schedule_name: v.schedule_name,
            })
            .collect();
        return_ok!(types::QueryExecResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(
        path = "/delete-running-status",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn delete_running_status(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteRunStatusReq>,
        _session: &Session,
    ) -> api_response!(types::DeleteRunStatusResp) {
        let schedule_type: ScheduleType = req.schedule_type.as_str().try_into()?;

        let svc = state.service();
        if !svc
            .job
            .can_write_job(&user_info, team_id.clone(), Some(req.eid.clone()))
            .await?
        {
            return_err!("no permission to delete the running status");
        }

        if svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id.clone())
            .await?
            .is_none()
        {
            return_err!("no permission to delete the running status");
        }

        let result = svc
            .job
            .delete_running_status(&user_info, &req.eid, schedule_type, &req.instance_id)
            .await?;

        return_ok!(types::DeleteRunStatusResp { result })
    }

    #[oai(
        path = "/delete-schedule-history",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn delete_schedule_history(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteScheduleHistoryReq>,
        _session: &Session,
    ) -> api_response!(types::DeleteScheduleHistoryResp) {
        let svc = state.service();
        if !svc
            .job
            .can_write_schedule_by_id(&user_info, team_id.clone(), Some(req.schedule_id.clone()))
            .await?
        {
            return_err!("no permission to delete this schedule history");
        }

        let result = svc
            .job
            .delete_schedule_history(&user_info, &req.eid, &req.schedule_id)
            .await?;

        return_ok!(types::DeleteScheduleHistoryResp { result })
    }

    #[oai(
        path = "/delete-exec-history",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn delete_exec_history(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteExecHistoryReq>,
        _session: &Session,
    ) -> api_response!(types::DeleteExecHistoryResp) {
        let schedule_type: Option<ScheduleType> = req
            .schedule_type
            .map(|v| v.as_str().try_into())
            .transpose()?;

        let svc = state.service();
        let username = if !svc
            .job
            .can_write_job(&user_info, team_id.clone(), None)
            .await?
        {
            Some(user_info.username.clone())
        } else {
            None
        };

        let result = svc
            .job
            .delete_exec_history(
                req.ids,
                req.schedule_id,
                schedule_type,
                req.instance_id,
                req.eid,
                team_id,
                username,
                req.time_range_start,
                req.time_range_end,
            )
            .await?;

        return_ok!(types::DeleteExecHistoryResp { result })
    }

    #[oai(path = "/action", method = "post", transform = "set_middleware")]
    pub async fn action(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::ActionReq>,
    ) -> Result<ApiStdResponse<types::ActionRes>> {
        let svc = state.service();
        let action = req.action.into();
        let ret = svc
            .job
            .action(
                req.schedule_id,
                req.instance_id,
                &user_info,
                team_id,
                action,
            )
            .await?;

        return_ok!(types::ActionRes { result: ret });
    }

    #[oai(
        path = "/save-bundle-script",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn save_bundle_script(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::SaveJobBundleScriptReq>,
    ) -> Result<ApiStdResponse<types::SaveJobBundleScriptResp>> {
        let args = match req.args {
            Some(v) => Some(serde_json::to_value(&v).map_err(std_into_error)?),
            None => None,
        };
        let svc = state.service();

        if !svc
            .job
            .can_write_bundle_script_by_id(&user_info, team_id.clone(), req.id)
            .await?
        {
            return_err!("no permission to delete this job");
        }

        let (eid, id, created_user) = match req.id {
            Some(v) => (NotSet, Set(v), NotSet),
            None => (
                Set(IdGenerator::get_job_bundle_script_uid()),
                NotSet,
                Set(user_info.username.clone()),
            ),
        };

        let ret = svc
            .job
            .save_job_bundle_script(job_bundle_script::ActiveModel {
                id,
                eid,
                executor_id: Set(req.executor_id),
                team_id: team_id.map_or(NotSet, |v| Set(v)),
                name: Set(req.name),
                code: Set(req.code),
                info: Set(req.info),
                created_user,
                updated_user: Set(user_info.username.clone()),
                args: Set(args),
                ..Default::default()
            })
            .await?;

        return_ok!(types::SaveJobBundleScriptResp {
            result: ret.id.as_ref().to_owned()
        });
    }

    #[oai(
        path = "/delete-bundle-script",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn delete_bundle_script(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteJobBundleScriptReq>,
    ) -> Result<ApiStdResponse<u64>> {
        let svc = state.service();
        if !svc
            .job
            .can_write_bundle_script(&user_info, team_id.clone(), Some(req.eid.clone()))
            .await?
        {
            return_err!("no permission to delete this job");
        }

        let ret = svc.job.delete_bundle_script(&user_info, req.eid).await?;
        return_ok!(ret)
    }

    #[oai(
        path = "/bundle-script-list",
        method = "get",
        transform = "set_middleware"
    )]
    pub async fn query_bundle_script_list(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        Query(search_username): Query<Option<String>>,
        #[oai(default)] Query(name): Query<Option<String>>,
        Query(default_eid): Query<Option<String>>,
        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(updated_time_range): Query<Option<Vec<String>>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,

        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryJobBundleScriptResp>> {
        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let svc = state.service();
        let default_eid = default_eid.filter(|v| v != "");
        let team_id = svc
            .job
            .get_default_validate_team_id_by_bundle_script(
                &user_info,
                default_eid.as_deref(),
                team_id,
            )
            .await?;

        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username
        } else {
            team_id.map_or_else(|| Some(user_info.username.clone()), |_| search_username)
        };

        let ret = svc
            .job
            .query_bundle_script(
                search_username,
                team_id,
                default_eid,
                name,
                updated_time_range,
                page - 1,
                page_size,
            )
            .await?;

        let list: Vec<types::JobBundleScriptRecord> = ret
            .0
            .into_iter()
            .map(|v| types::JobBundleScriptRecord {
                id: v.id,
                eid: v.eid,
                executor_id: v.executor_id,
                executor_name: v.executor_name,
                team_id: v.team_id,
                team_name: v.team_name,
                name: v.name,
                code: v.code,
                info: v.info,
                created_user: v.created_user,
                updated_user: v.updated_user,
                args: v.args,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();
        return_ok!(types::QueryJobBundleScriptResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/timer-list", method = "get", transform = "set_middleware")]
    pub async fn query_timer(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(default)] Query(name): Query<Option<String>>,
        #[oai(default)] Query(job_type): Query<Option<String>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Query(search_username): Query<Option<String>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(updated_time_range): Query<Option<Vec<String>>>,

        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,

        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryJobTimerResp>> {
        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let svc = state.service();

        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username.as_ref()
        } else {
            team_id.map_or(Some(&user_info.username), |_| search_username.as_ref())
        };

        let ret = svc
            .job
            .query_job_timer(
                team_id,
                search_username,
                name.filter(|v| v != ""),
                job_type.filter(|v| v != ""),
                updated_time_range,
                tag_ids,
                page - 1,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_job_ids(ret.0.iter().map(|v| v.job_id).collect())
            .await?;

        let list: Vec<types::JobTimerRecord> = ret
            .0
            .into_iter()
            .map(|v| types::JobTimerRecord {
                id: v.id,
                eid: v.eid,
                name: v.name,
                job_name: v.job_name,
                timer_expr: v.timer_expr.map_or(json!("null"), |v| v),
                job_type: v.job_type,
                info: v.info,
                team_id: v.team_id,
                team_name: v.team_name,
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.job_id {
                                Some(types::JobTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                executor_id: v.executor_id,
                executor_name: v.executor_name,
                executor_platform: v.executor_platform,
                created_user: v.created_user,
                updated_user: v.updated_user,
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
            })
            .collect();
        return_ok!(types::QueryJobTimerResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(path = "/save-timer", method = "post", transform = "set_middleware")]
    pub async fn save_timer(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::SaveJobTimerReq>,
    ) -> Result<ApiStdResponse<types::SaveJobTimerResp>> {
        let svc = state.service();

        if !svc
            .job
            .can_write_job(&user_info, team_id, Some(req.eid.clone()))
            .await?
        {
            return Err(NoPermission().into());
        }

        let ret = svc
            .job
            .save_job_timer(crate::entity::job_timer::ActiveModel {
                id: req.id.map_or(NotSet, |v| Set(v)),
                name: Set(req.name),
                eid: Set(req.eid),
                timer_expr: Set(Some(
                    serde_json::to_value(req.timer_expr).map_err(std_into_error)?,
                )),
                job_type: Set(req.job_type),
                info: Set(req.info),
                created_user: req.id.map_or(Set(user_info.username.clone()), |_| NotSet),
                updated_user: Set(user_info.username.clone()),
                ..Default::default()
            })
            .await?;

        return_ok!(types::SaveJobTimerResp {
            result: ret.id.as_ref().to_owned()
        });
    }

    #[oai(path = "/delete-timer", method = "post", transform = "set_middleware")]
    pub async fn delete_timer(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteJobTimerReq>,
    ) -> api_response!(types::DeleteJobTimerResp) {
        let svc = state.service();
        if !svc
            .job
            .can_write_job_timer_by_id(&user_info, team_id.clone(), Some(req.id))
            .await?
        {
            return_err!("no permission to delete this job");
        }
        let result = svc.job.delete_job_timer(&user_info, req.id).await?;
        return_ok!(types::DeleteJobTimerResp { result });
    }

    #[oai(path = "/dashboard", method = "post", transform = "set_middleware")]
    pub async fn get_dashboard(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::GetDashboardReq>,
    ) -> Result<ApiStdResponse<types::GetDashboardResp>> {
        let svc = state.service();

        let job_summary = svc.job.get_summary(&user_info).await?;

        let stats = svc
            .job
            .get_dashboard(&user_info, Some(req.job_type), None)
            .await?;

        let rows = stats
            .into_iter()
            .map(|v| types::JobRunResultStats {
                name: v.name,
                eid: v.eid,
                schedule_name: v.schedule_name,
                results: v
                    .results
                    .into_iter()
                    .map(|v| types::JobRunSummary {
                        eid: v.eid,
                        total: v.total,
                        name: v.name,
                        info: v.info,
                        last_start_time: v.last_start_time,
                        exec_succ_num: v.exec_succ_num,
                        exec_fail_num: v.exec_fail_num,
                        check_succ_num: v.check_succ_num,
                        check_fail_num: v.check_fail_num,
                        eval_fail_num: v.eval_fail_num,
                    })
                    .collect(),
            })
            .collect();

        return_ok!(types::GetDashboardResp {
            rows,
            job_num: job_summary.total,
            running_num: job_summary.running_num,
            exec_succ_num: job_summary.exec_succ_num,
            exec_fail_num: job_summary.exec_fail_num,
        });
    }

    #[oai(
        path = "/supervisor-list",
        method = "get",
        transform = "set_middleware"
    )]
    pub async fn query_job_supervisor(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(default)] Query(name): Query<Option<String>>,
        Query(eid): Query<Option<String>>,
        /// Search based on time range
        #[oai(validator(max_items = 2, min_items = 2))]
        Query(updated_time_range): Query<Option<Vec<String>>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Query(search_username): Query<Option<String>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(default = "types::default_page", validator(maximum(value = "10000")))]
        Query(page): Query<u64>,
        #[oai(
            default = "types::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
    ) -> Result<ApiStdResponse<types::QueryJobSupervisorResp>> {
        let updated_time_range = updated_time_range.map(|v| (v[0].clone(), v[1].clone()));
        let svc = state.service();
        let search_username = if state.can_manage_job(&user_info.user_id).await? {
            search_username.as_ref()
        } else {
            team_id.map_or_else(|| Some(&user_info.username), |_| search_username.as_ref())
        };
        let ret = svc
            .job
            .query_job_supervisor(
                search_username,
                name.filter(|v| v != ""),
                eid,
                team_id,
                updated_time_range,
                tag_ids,
                page - 1,
                page_size,
            )
            .await?;

        let tag_records = svc
            .tag
            .get_all_tag_bind_by_job_ids(ret.0.iter().map(|v| v.job_id).collect())
            .await?;

        let list: Vec<types::JobSupervisorRecord> = ret
            .0
            .into_iter()
            .map(|v| types::JobSupervisorRecord {
                id: v.id,
                name: v.name,
                job_name: v.job_name,
                eid: v.eid,
                info: v.info,
                executor_id: v.executor_id,
                team_id: v.team_id,
                team_name: v.team_name,
                created_user: v.created_user,
                updated_user: v.updated_user,
                tags: Some(
                    tag_records
                        .iter()
                        .filter_map(|tb| {
                            if tb.resource_id == v.job_id {
                                Some(types::JobTag {
                                    id: tb.tag_id,
                                    tag_name: tb.tag_name.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                ),
                created_time: local_time!(v.created_time),
                updated_time: local_time!(v.updated_time),
                executor_name: v.executor_name,
                restart_interval: v.restart_interval,
                executor_platform: v.executor_platform,
            })
            .collect();
        return_ok!(types::QueryJobSupervisorResp {
            total: ret.1,
            list: list,
        })
    }

    #[oai(
        path = "/save-supervisor",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn save_job_supervisor(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::SaveJobSupervisorReq>,
    ) -> api_response!(types::SaveJobSupervisorResp) {
        let svc = state.service();

        if !svc
            .job
            .can_write_job(&user_info, team_id, Some(req.eid.clone()))
            .await?
        {
            return Err(NoPermission().into());
        }

        let ret = svc
            .job
            .save_job_supervisor(job_supervisor::ActiveModel {
                id: req.id.map_or(NotSet, |v| Set(v)),
                name: Set(req.name),
                eid: Set(req.eid),
                restart_interval: Set({
                    if req.restart_interval == 0 {
                        1
                    } else {
                        req.restart_interval
                    }
                }),
                info: Set(req.info),
                created_user: req.id.map_or(Set(user_info.username.clone()), |_| NotSet),
                updated_user: Set(user_info.username.clone()),
                ..Default::default()
            })
            .await?;

        return_ok!(types::SaveJobSupervisorResp {
            result: ret.id.as_ref().to_owned()
        });
    }

    #[oai(
        path = "/delete-supervisor",
        method = "post",
        transform = "set_middleware"
    )]
    pub async fn delete_supervisor(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Json(req): Json<types::DeleteJobSupervisorReq>,
    ) -> api_response!(types::DeleteJobSupervisorResp) {
        let svc = state.service();
        if !svc
            .job
            .can_write_job_supervisor_by_id(&user_info, team_id.clone(), Some(req.id))
            .await?
        {
            return_err!("no permission to delete this job supervisor");
        }

        let result = svc.job.delete_job_supervisor(&user_info, req.id).await?;
        return_ok!(types::DeleteJobSupervisorResp { result });
    }
}
