use std::time::Duration;

use crate::{
    api_response, default_local_time,
    entity::{job, job_bundle_script, job_supervisor},
    error::NoPermission,
    local_time,
    logic::{self, job::types::BundleScriptRecord},
    middleware,
    response::{std_into_error, ApiStdResponse},
    return_err, return_ok, AppState, IdGenerator,
};

use super::types;
use automate::JobAction;
use poem::{session::Session, web::Data, Endpoint, EndpointExt, Result};
use poem_openapi::{
    param::{Header, Query},
    payload::Json,
    OpenApi,
};
use sea_orm::{ActiveValue::NotSet, Set};
use serde_json::json;

fn set_middleware(ep: impl Endpoint) -> impl Endpoint {
    ep.with(middleware::TeamPermissionMiddleware)
}

pub struct JobApi;

#[OpenApi(prefix_path = "/job", tag = crate::api::Tag::Job)]
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

        if let Some(job_id) = req.id {
            if !svc
                .job
                .can_write_job_by_id(&user_info, team_id, job_id)
                .await?
            {
                return Err(NoPermission().into());
            }
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

        let (eid, id) = match req.id {
            Some(v) => (NotSet, Set(v)),
            None => (Set(IdGenerator::get_job_eid()), NotSet),
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
                created_user: Set(user_info.username.clone()),
                updated_user: Set(user_info.username.clone()),
                args: Set(args),
                team_id: Set(team_id.unwrap_or_default()),
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
                    .map(|v| types::CompletedCallbackOpts::from(v)),
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
        let username = if !svc
            .team
            .can_delete_job(team_id.clone(), &user_info.user_id)
            .await?
        {
            if team_id.is_none() {
                Some(user_info.username.clone())
            } else {
                return_err!("no permission to delete this job");
            }
        } else {
            None
        };

        let result = svc.job.delete_job(username, req.eid).await?;
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

    #[oai(path = "/run-list", method = "get", transform = "set_middleware")]
    pub async fn query_run_list(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        user_info: Data<&logic::types::UserInfo>,

        #[oai(default)] Query(bind_ip): Query<Option<String>>,
        #[oai(default)] Query(schedule_name): Query<Option<String>>,
        Query(search_username): Query<Option<String>>,
        #[oai(validator(
            custom = "crate::api::OneOfValidator::new(vec![\"once\",\"timer\",\"flow\",\"daemon\"])"
        ))]
        Query(schedule_type): Query<String>,
        #[oai(validator(
            custom = "crate::api::OneOfValidator::new(vec![\"bundle\", \"default\"])"
        ))]
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
            custom = "crate::api::OneOfValidator::new(vec![\"once\",\"timer\",\"flow\",\"daemon\"])"
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
        #[oai(validator(
            custom = "crate::api::OneOfValidator::new(vec![\"bundle\", \"default\"])"
        ))]
        Query(job_type): Query<String>,
        #[oai(default)] Query(schedule_name): Query<Option<String>>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
        Query(tag_ids): Query<Option<Vec<u64>>>,
        #[oai(validator(
            custom = "crate::api::OneOfValidator::new(vec![\"once\",\"timer\",\"flow\",\"daemon\"])"
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
        let svc = state.service();
        let username = if !svc
            .team
            .can_delete_job(team_id.clone(), &user_info.user_id)
            .await?
        {
            if team_id.is_none() {
                Some(user_info.username.clone())
            } else {
                return_err!("no permission to delete job execution history");
            }
        } else {
            None
        };

        let result = svc
            .job
            .delete_exec_history(
                req.ids,
                req.schedule_id,
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

        if let Some(bundle_script_id) = req.id {
            if !svc
                .job
                .can_write_bundle_script_by_id(&user_info, team_id, bundle_script_id)
                .await?
            {
                return Err(NoPermission().into());
            }
        }

        let (eid, id) = match req.id {
            Some(v) => (NotSet, Set(v)),
            None => (Set(IdGenerator::get_job_bundle_script_uid()), NotSet),
        };

        let ret = svc
            .job
            .save_job_bundle_script(job_bundle_script::ActiveModel {
                id,
                eid,
                executor_id: Set(req.executor_id),
                team_id: Set(team_id.unwrap_or_default()),
                name: Set(req.name),
                code: Set(req.code),
                info: Set(req.info),
                created_user: Set(user_info.username.clone()),
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
        let username = if !svc
            .team
            .can_delete_job(team_id.clone(), &user_info.user_id)
            .await?
        {
            if team_id.is_none() {
                Some(user_info.username.clone())
            } else {
                return_err!("no permission to delete the bundle script");
            }
        } else {
            None
        };

        let ret = svc.job.delete_bundle_script(username, req.eid).await?;
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
            team_id.map_or_else(|| Some(&user_info.username), |_| search_username.as_ref())
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

        if !svc.job.can_write_job(&user_info, team_id, &req.eid).await? {
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
                created_user: Set(user_info.username.clone()),
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
        let username = if !svc
            .team
            .can_delete_job(team_id.clone(), &user_info.user_id)
            .await?
        {
            if team_id.is_none() {
                Some(user_info.username.clone())
            } else {
                return_err!("no permission to delete this job timer");
            }
        } else {
            None
        };
        let result = svc.job.delete_job_timer(username, req.id).await?;
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

        if !svc.job.can_write_job(&user_info, team_id, &req.eid).await? {
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
                created_user: Set(user_info.username.clone()),
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
        let username = if !svc
            .team
            .can_delete_job(team_id.clone(), &user_info.user_id)
            .await?
        {
            if team_id.is_none() {
                Some(user_info.username.clone())
            } else {
                return_err!("no permission to delete this job supervisor");
            }
        } else {
            None
        };

        let result = svc.job.delete_job_supervisor(username, req.id).await?;
        return_ok!(types::DeleteJobSupervisorResp { result });
    }
}
