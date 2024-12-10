use std::path::PathBuf;

use anyhow::{anyhow, Result};

use automate::{
    bridge::msg::{BundleOutputParams, UpdateJobParams},
    scheduler::types::{BundleScript, RunStatus, ScheduleStatus, ScheduleType, UploadFile},
    JobAction,
};

use evalexpr::eval_boolean;

use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait, Set,
};

use sea_query::OnConflict;

use serde_json::{json, Value};
use tokio::fs;
use tracing::error;

use crate::{
    entity::{self, executor, instance, job, job_running_status, job_schedule_history, prelude::*},
    file_name,
    logic::{executor::ExecutorLogic, job::types::DispatchResult},
    utils, IdGenerator,
};

use super::{
    types::{BundleScriptRecord, BundleScriptResult, DispatchData, DispatchTarget},
    JobLogic,
};

#[test]
fn test_hello() {
    match eval_boolean("$v=10;true") {
        Ok(v) => println!("-----------------{}", v),
        Err(e) => println!("-----------------{}", e),
    }
}

impl<'a> JobLogic<'a> {
    pub async fn compute_bundle_output() {}

    pub fn eval(
        &self,
        record: Vec<BundleScriptRecord>,
        output: Vec<BundleOutputParams>,
    ) -> Vec<BundleScriptResult> {
        record
            .iter()
            .map(|v| {
                for val in output.iter() {
                    if v.eid == val.eid {
                        let (result, eval_err) = match eval_boolean(&format!(
                            "$v={}; {}",
                            val.stdout.clone().unwrap_or_default().clone(),
                            v.cond_expr.clone(),
                        )) {
                            Ok(v) => (v, None),
                            Err(e) => (false, Some(e.to_string())),
                        };

                        return BundleScriptResult {
                            name: v.name.clone(),
                            eid: v.eid.clone(),
                            cond_expr: v.cond_expr.clone(),
                            exit_code: val.exit_code,
                            exit_status: val.exit_status.clone(),
                            stdout: val.stdout.clone(),
                            stderr: val.stderr.clone(),
                            eval_err,
                            result,
                        };
                    }
                }
                return BundleScriptResult {
                    name: v.name.clone(),
                    eid: v.eid.clone(),
                    cond_expr: v.cond_expr.clone(),
                    ..Default::default()
                };
            })
            .collect()
    }

    pub async fn update_job_status(&self, params: UpdateJobParams) -> Result<u64> {
        let mut update_values = vec![
            (
                job_running_status::Column::ScheduleId,
                params.schedule_id.clone().into(),
            ),
            (
                job_running_status::Column::UpdatedUser,
                params.created_user.clone().into(),
            ),
        ];

        params.start_time.clone().inspect(|v| {
            update_values.push((job_running_status::Column::StartTime, (*v).into()));
            update_values.push((job_running_status::Column::EndTime, params.end_time.into()));
        });

        params.prev_time.clone().inspect(|v| {
            update_values.push((job_running_status::Column::PrevTime, (*v).into()));
            update_values.push((
                job_running_status::Column::NextTime,
                params.next_time.into(),
            ));
        });

        params.schedule_status.clone().inspect(|v| {
            if *v == ScheduleStatus::Unscheduled {
                update_values.push((
                    job_running_status::Column::NextTime,
                    params.next_time.into(),
                ));
            }
        });

        if let Some(run_status) = params.run_status.clone() {
            update_values.push((
                job_running_status::Column::RunStatus,
                run_status.to_string().into(),
            ))
        }

        if let Some(ref exit_status) = params.exit_status {
            update_values.push((job_running_status::Column::ExitStatus, exit_status.into()))
        }

        if let Some(exit_code) = params.exit_code {
            update_values.push((job_running_status::Column::ExitCode, exit_code.into()))
        }

        if let Some(schedule_status) = params.schedule_status.clone() {
            update_values.push((
                job_running_status::Column::ScheduleStatus,
                schedule_status.to_string().into(),
            ))
        }
        // if let Some(prev_time) = params.prev_time {
        //     update_values.push((job_running_status::Column::PrevTime, prev_time.into()))
        // }

        // if let Some(next_time) = params.next_time {
        //     update_values.push((job_running_status::Column::NextTime, next_time.into()))
        // }

        let schedule_type = params
            .schedule_type
            .clone()
            .map_or_else(|| NotSet, |v| Set(v.to_string()));
        let run_status = params
            .run_status
            .clone()
            .map_or_else(|| NotSet, |v| Set(v.to_string()));
        let schedule_status = params
            .schedule_status
            .clone()
            .map_or_else(|| NotSet, |v| Set(v.to_string()));

        let active_model = JobRunningStatus::insert(job_running_status::ActiveModel {
            schedule_type,
            eid: Set(params.base_job.eid.clone()),
            instance_id: Set(params.instance_id.clone()),
            schedule_id: Set(params.schedule_id.clone()),
            schedule_status,
            run_status,
            start_time: Set(params.start_time),
            job_type: Set(params
                .base_job
                .bundle_script
                .map_or("default".to_string(), |_v| "bundle".to_string())),
            prev_time: Set(params.prev_time),
            updated_user: Set(params.created_user.clone()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                job_running_status::Column::Eid,
                job_running_status::Column::ScheduleType,
                job_running_status::Column::InstanceId,
            ])
            .values(update_values)
            .to_owned(),
        );

        let ret = active_model.exec(&self.ctx.db).await?;

        match params.run_status {
            Some(RunStatus::Stop) => {
                let (bundle_script_result, job_type) = if params.bundle_output.is_some() {
                    let schedule_record = self
                        .get_schedule(params.schedule_id.clone())
                        .await?
                        .ok_or(anyhow::format_err!(
                            "cannot get schedule record {}",
                            params.schedule_id
                        ))?;
                    let job_record: job::Model = serde_json::from_value(
                        schedule_record
                            .snapshot_data
                            .ok_or(anyhow::format_err!("cannot get snapshot_data"))?,
                    )?;

                    let bundle_script: Vec<BundleScriptRecord> = serde_json::from_value(
                        job_record
                            .bundle_script
                            .ok_or(anyhow::format_err!("cannot get bundle_sciprt"))?,
                    )?;
                    let val = serde_json::to_value(
                        &self.eval(bundle_script, params.bundle_output.unwrap()),
                    )?;
                    (Set(Some(val)), Set("bundle".to_string()))
                } else {
                    (NotSet, Set("default".to_string()))
                };

                let output = params.stdout.unwrap_or_default();
                let output = params
                    .stderr
                    .map_or(output.clone(), |v| format!("{v}\n{output}"));

                let ret = JobExecHistory::insert(entity::job_exec_history::ActiveModel {
                    schedule_id: Set(params.schedule_id),
                    instance_id: Set(params.instance_id),
                    exit_status: Set(params.exit_status.clone().unwrap_or_default()),
                    exit_code: Set(params.exit_code.unwrap_or_default()),
                    output: Set(output),
                    eid: Set(params.base_job.eid),
                    start_time: Set(params.start_time),
                    end_time: Set(params.end_time),
                    bundle_script_result,
                    job_type,
                    ..Default::default()
                })
                .exec(&self.ctx.db)
                .await?;
                Ok(ret.last_insert_id)
            }
            _ => Ok(ret.last_insert_id),
        }
    }

    pub async fn dispatch_job(
        &self,
        secret: String,
        instance_ids: Vec<String>,
        eid: String,
        is_sync: bool,
        schedule_name: String,
        schedule_type: ScheduleType,
        action: automate::JobAction,
        timer_expr: Option<String>,
        created_user: String,
    ) -> Result<u64> {
        let schedule_id = IdGenerator::get_schedule_uid();

        let endpoints = Instance::find()
            .filter(instance::Column::InstanceId.is_in(instance_ids))
            .all(&self.ctx.db)
            .await?;
        if endpoints.len() == 0 {
            anyhow::bail!("cannot found valid instance");
        }

        let job_record = Job::find()
            .filter(entity::job::Column::Eid.eq(eid.clone()))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("cannot found job {}", eid))?;

        let executor_record = Executor::find()
            .filter(executor::Column::Id.eq(job_record.executor_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!(
                "cannot found executor {}",
                job_record.executor_id.clone()
            ))?;

        let mut dispatch_result = Vec::new();

        let mut upload_file: Option<UploadFile> = None;

        if job_record.upload_file != "" {
            let data = fs::read(job_record.upload_file.clone()).await?;
            upload_file = Some(UploadFile {
                filename: file_name!(job_record.upload_file.clone()),
                data: Some(data),
            });
        }

        let (bundle_script, job_type): (Option<Vec<BundleScript>>, String) =
            match job_record.clone().bundle_script {
                Some(v) => {
                    let list: Vec<BundleScriptRecord> = serde_json::from_value(v)?;
                    let executor_id = list.iter().map(|v| v.executor_id).collect::<Vec<u64>>();
                    let executor_list = ExecutorLogic::new(self.ctx)
                        .get_all_by_executor_id(executor_id)
                        .await?;

                    let mut ret = vec![];
                    for v in list {
                        let e = executor_list
                            .get_by_id(v.executor_id)
                            .ok_or(anyhow!("cannot found executor {}", v.executor_id))?;
                        let command_slice: Vec<&str> = e.command.split(" ").collect();

                        ret.push(BundleScript {
                            eid: v.eid.clone(),
                            cmd_name: command_slice
                                .get(0)
                                .map_or("".to_string(), |&v| v.to_owned()),
                            code: v.code.clone(),
                            args: command_slice
                                .get(1..)
                                .map_or(vec![], |v| v.into_iter().map(|&v| v.to_owned()).collect()),
                        })
                    }

                    (Some(ret), "bundle".to_string())
                }
                None => (None, "default".to_string()),
            };

        let command_slice: Vec<&str> = executor_record.command.split(" ").collect();

        let dispatch_params = automate::DispatchJobParams {
            base_job: automate::BaseJob {
                eid: job_record.eid.clone(),
                cmd_name: command_slice
                    .get(0)
                    .map_or("".to_string(), |&v| v.to_owned()),
                bundle_script,
                code: job_record.code.clone(),
                args: command_slice
                    .get(1..)
                    .map_or(vec![], |v| v.into_iter().map(|&v| v.to_owned()).collect()),
                upload_file: upload_file.clone(),
                work_dir: Some(job_record.work_dir.clone()).filter(|v| !v.is_empty()),
                work_user: Some(job_record.work_user.clone()).filter(|v| !v.is_empty()),
                timeout: job_record.timeout,
                max_retry: job_record.max_retry as u8,
                max_parallel: job_record.max_parallel as u8,
                read_code_from_stdin: false,
            },
            instance_id: "".to_string(),
            fields: None,
            restart_interval: None,
            created_user: created_user.clone(),
            schedule_id: schedule_id.clone(),
            timer_expr: timer_expr.clone(),
            is_sync,
            action: action.clone(),
        };

        let mut dispatch_data = DispatchData {
            target: Vec::new(),
            params: dispatch_params.clone(),
        };

        endpoints.into_iter().for_each(|v| {
            dispatch_data.target.push(DispatchTarget {
                ip: v.ip.clone(),
                mac_addr: v.mac_addr.clone(),
                namespace: v.namespace.clone(),
                instance_id: v.instance_id.clone(),
            });
        });

        let logic = automate::Logic::new(self.ctx.redis().clone());
        let http_client = self.ctx.http_client.clone();

        let batch_push_ret = utils::async_batch_do(dispatch_data.target.clone(), move |v| {
            let mut dispatch_params = dispatch_params.clone();
            let logic = logic.clone();
            let http_client = http_client.clone();
            let secret = secret.clone();
            dispatch_params.instance_id = v.instance_id.clone();
            Box::pin(async move {
                let body = automate::DispatchJobRequest {
                    agent_ip: v.ip.clone(),
                    mac_addr: v.mac_addr.clone(),
                    dispatch_params: dispatch_params.clone(),
                };
                let pair = match logic.get_link_pair(v.ip.clone(), v.mac_addr.clone()).await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            instance_id: v.instance_id.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        })
                    }
                };
                let api_url = format!(
                    "http://{}/dispatch?secret={}",
                    pair.1.comet_addr,
                    secret.clone()
                );
                let response = match http_client.post(api_url).json(&body).send().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            instance_id: v.instance_id.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let response = match response.error_for_status() {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            instance_id: v.instance_id.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let ret = match response.json::<serde_json::Value>().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            instance_id: v.instance_id.clone(),
                            has_err: true,
                            err: Some(e.to_string()),
                        })
                    }
                };

                let (has_err, err) = if ret["code"] != 20000 {
                    (true, Some(ret["msg"].to_string()))
                } else {
                    (false, None)
                };

                Ok(DispatchResult {
                    namespace: v.namespace.clone(),
                    bind_ip: v.ip.clone(),
                    response: ret.clone(),
                    instance_id: v.instance_id.clone(),
                    has_err,
                    err,
                })
            })
        })
        .await;

        let mut has_err = false;
        batch_push_ret.into_iter().for_each(|v| {
            let v = v.unwrap();
            if v.has_err {
                has_err = true;
            }
            dispatch_result.push(v)
        });

        dispatch_data
            .params
            .base_job
            .upload_file
            .iter_mut()
            .for_each(|v| v.data = None);

        let ret = JobScheduleHistory::insert(entity::job_schedule_history::ActiveModel {
            schedule_id: Set(schedule_id.clone()),
            name: Set(schedule_name),
            eid: Set(eid.clone()),
            job_type: Set(job_type),
            schedule_type: Set(schedule_type.to_string()),
            dispatch_result: Set(Some(serde_json::to_value(&dispatch_result)?)),
            action: Set(action.to_string()),
            dispatch_data: Set(Some(serde_json::to_value(&dispatch_data)?)),
            snapshot_data: Set(Some(serde_json::to_value(job_record)?)),
            created_user: Set(created_user.clone()),
            updated_user: Set(created_user.clone()),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;

        if has_err {
            anyhow::bail!("Partial job scheduling failed");
        }

        Ok(ret.last_insert_id)
    }

    pub async fn dispatch_runnable_job_to_endpoint(
        &self,
        bind_namespace: String,
        bind_ip: String,
        mac_address: String,
    ) -> Result<()> {
        let ins = Instance::find()
            .filter(instance::Column::MacAddr.eq(mac_address))
            .filter(instance::Column::Ip.eq(bind_ip.clone()))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("cannot found instance"))?;

        let runnable: Vec<(serde_json::Value, String)> = JobRunningStatus::find()
            .select_only()
            .column(job_schedule_history::Column::DispatchData)
            .column(instance::Column::MacAddr)
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(JobRunningStatus)
                    .from(instance::Column::InstanceId)
                    .to(job_running_status::Column::InstanceId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                JobScheduleHistory::belongs_to(JobRunningStatus)
                    .from(job_schedule_history::Column::ScheduleId)
                    .to(job_running_status::Column::ScheduleId)
                    .into(),
            )
            .filter(job_running_status::Column::ScheduleStatus.is_in([
                ScheduleStatus::Scheduling.to_string(),
                ScheduleStatus::Prepare.to_string(),
            ]))
            .filter(job_running_status::Column::InstanceId.eq(ins.instance_id))
            .into_tuple()
            .all(&self.ctx.db)
            .await?;

        let http_client = self.ctx.http_client.clone();
        let logic = automate::Logic::new(self.ctx.redis().clone());

        for (dispatch_data_val, mac_addr) in runnable {
            let dispatch_data: DispatchData = dispatch_data_val.try_into()?;

            let body = automate::DispatchJobRequest {
                agent_ip: bind_ip.clone(),
                dispatch_params: dispatch_data.params.clone(),
                mac_addr,
            };
            let pair = match logic
                .get_link_pair(bind_ip.clone(), ins.mac_addr.clone())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        "failed get link pair on namespace:{} ip:{}, {}",
                        bind_namespace,
                        bind_ip,
                        e.to_string()
                    );
                    continue;
                }
            };

            let api_url = format!("http://{}/dispatch", pair.1.comet_addr);

            let response = match http_client.post(api_url).json(&body).send().await {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        "failed dispatch runnable job on namespace:{} ip:{}, {}",
                        bind_namespace,
                        bind_ip,
                        e.to_string()
                    );
                    continue;
                }
            };

            let ret = match response.json::<serde_json::Value>().await {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        "failed decode dispatch runnable job response on namespace:{} ip:{}, {}",
                        bind_namespace,
                        bind_ip,
                        e.to_string()
                    );
                    continue;
                }
            };
            if ret["code"] != 20000 {
                error!(
                    "failed check dispatch runnable job response on namespace:{} ip:{}, {}",
                    bind_namespace, bind_ip, ret["msg"]
                );
                continue;
            };
        }
        Ok(())
    }

    pub async fn redispatch_job(
        &self,
        schedule_id: &str,
        action: JobAction,
    ) -> Result<Vec<Result<DispatchResult>>> {
        let job_schedule_record = JobScheduleHistory::find()
            .filter(job_schedule_history::Column::ScheduleId.eq(schedule_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!(
                "cannot found job schedule by schedule_id: {schedule_id}"
            ))?;

        let dispatch_data: DispatchData = job_schedule_record
            .dispatch_data
            .ok_or(anyhow!("cannot found job dispatch data"))?
            .try_into()?;

        let logic = automate::Logic::new(self.ctx.redis().clone());

        let http_client = self.ctx.http_client.clone();

        let batch_push_ret = utils::async_batch_do(dispatch_data.target, move |v| {
            let mut dispatch_params = dispatch_data.params.clone();
            let logic = logic.clone();
            let http_client = http_client.clone();
            dispatch_params.action = action;
            dispatch_params.instance_id = v.instance_id.clone();
            Box::pin(async move {
                let body = automate::DispatchJobRequest {
                    agent_ip: v.ip.clone(),
                    mac_addr: v.mac_addr.clone(),
                    dispatch_params: dispatch_params.clone(),
                };
                let pair = match logic.get_link_pair(v.ip.clone(), v.mac_addr.clone()).await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            instance_id: v.instance_id.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        })
                    }
                };

                let api_url = format!("http://{}/dispatch", pair.1.comet_addr);

                let response = match http_client.post(api_url).json(&body).send().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                            instance_id: v.instance_id.clone(),
                        })
                    }
                };

                let ret = match response.json::<serde_json::Value>().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            has_err: true,
                            instance_id: v.instance_id.clone(),
                            err: Some(e.to_string()),
                        })
                    }
                };
                let (has_err, err) = if ret["code"] != 20000 {
                    (true, Some(ret["msg"].to_string()))
                } else {
                    (false, None)
                };

                Ok(DispatchResult {
                    namespace: v.namespace.clone(),
                    bind_ip: v.ip.clone(),
                    response: ret.clone(),
                    instance_id: v.instance_id.clone(),
                    has_err,
                    err,
                })
            })
        })
        .await;

        let mut dispatch_result = Vec::new();

        let mut has_err = false;
        batch_push_ret.iter().for_each(|v| {
            let v = v.as_ref().unwrap().to_owned();
            if v.has_err {
                has_err = true;
            }
            dispatch_result.push(v)
        });

        JobScheduleHistory::update_many()
            .set(job_schedule_history::ActiveModel {
                action: Set(action.to_string()),
                dispatch_result: Set(Some(serde_json::to_value(&dispatch_result)?)),
                ..Default::default()
            })
            .filter(job_schedule_history::Column::ScheduleId.eq(schedule_id.to_string()))
            .exec(&self.ctx.db)
            .await?;

        if has_err {
            anyhow::bail!("Partial job scheduling failed");
        }

        Ok(batch_push_ret)
    }

    pub async fn query_schedule(
        &self,
        schedule_type: Option<String>,
        created_user: String,
        job_type: String,
        name: Option<String>,
        updated_time_range: Option<(String, String)>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<job_schedule_history::Model>, u64)> {
        let model = JobScheduleHistory::find()
            .filter(job_schedule_history::Column::CreatedUser.eq(created_user))
            .filter(job_schedule_history::Column::JobType.eq(job_type))
            .apply_if(schedule_type, |query, v| {
                query.filter(job_schedule_history::Column::ScheduleType.eq(v))
            })
            .apply_if(name, |query, v| {
                query.filter(job_schedule_history::Column::Name.contains(v))
            })
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    job_schedule_history::Column::UpdatedTime
                        .gt(v.0)
                        .and(job_schedule_history::Column::UpdatedTime.lt(v.1)),
                )
            });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_desc(job_schedule_history::Column::Id)
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn action(
        &self,
        schedule_id: String,
        instance_id: String,
        updated_user: String,
        action: JobAction,
    ) -> Result<Value> {
        let ins = Instance::find()
            .filter(instance::Column::InstanceId.eq(instance_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("cannot found instance"))?;

        let schedule_record =
            self.get_schedule(schedule_id.clone())
                .await?
                .ok_or(anyhow::format_err!(
                    "cannot get shedule by {}",
                    schedule_id.clone()
                ))?;

        let dispatch_data = schedule_record
            .dispatch_data
            .ok_or(anyhow!("cannot get dispatch data"))?;

        let mut dispatch_data = serde_json::from_value::<DispatchData>(dispatch_data)?;

        // if dispatch_data.params.base_job.upload_file.is_some() {
        //     let job_record = Job::find()
        //         .filter(entity::job::Column::Eid.eq(schedule_record.eid.clone()))
        //         .one(&self.ctx.db)
        //         .await?
        //         .ok_or(anyhow!("cannot found job {}", schedule_record.eid))?;

        //     let data = fs::read(job_record.upload_file.clone()).await?;

        //     dispatch_data.params.base_job.upload_file = Some(UploadFile {
        //         filename: file_name!(job_record.upload_file.clone()),
        //         data: Some(data),
        //     })
        // }

        let logic = automate::Logic::new(self.ctx.redis());

        let pair = logic.get_link_pair(&ins.ip, &ins.mac_addr).await?;
        let api_url = format!("http://{}/dispatch", pair.1.comet_addr);
        dispatch_data.params.instance_id = ins.instance_id.clone();
        dispatch_data.params.created_user = updated_user;

        let mut body = automate::DispatchJobRequest {
            agent_ip: ins.ip.clone(),
            mac_addr: ins.mac_addr.clone(),
            dispatch_params: dispatch_data.params.clone(),
        };
        body.dispatch_params.action = action.clone();

        let ret = self
            .ctx
            .http_client
            .post(api_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if ret["code"] != 20000 {
            anyhow::bail!("failed to dispatch job");
        }

        // JobRunningStatus::update_many()
        //     .set(job_running_status::ActiveModel {
        //         dispatch_result: Set(Some(ret.clone())),
        //         // schedule_status: match action {
        //         //     JobAction::Exec | JobAction::Kill => NotSet,
        //         //     JobAction::StartTimer | JobAction::StopTimer => {
        //         //         Set(ScheduleStatus::Prepare.to_string())
        //         //     }
        //         //     JobAction::StartSupervisor => todo!(),
        //         //     JobAction::StopSupervisor => todo!(),
        //         // },
        //         run_status: match action {
        //             JobAction::Exec | JobAction::Kill => Set(RunStatus::Prepare.to_string()),
        //             JobAction::StartTimer | JobAction::StopTimer => NotSet,
        //             JobAction::StartSupervisor => todo!(),
        //             JobAction::StopSupervisor => todo!(),
        //         },
        //         updated_user: Set(updated_user),
        //         ..Default::default()
        //     })
        //     .filter(job_running_status::Column::ScheduleId.eq(schedule_id.clone()))
        //     .filter(job_running_status::Column::InstanceId.eq(ins.instance_id))
        //     .exec(&self.ctx.db)
        //     .await?;

        Ok(ret)
    }

    pub async fn get_schedule(
        &self,
        schedule_id: String,
    ) -> Result<Option<entity::job_schedule_history::Model>> {
        let ret = JobScheduleHistory::find()
            .filter(entity::job_schedule_history::Column::ScheduleId.eq(schedule_id))
            .one(&self.ctx.db)
            .await?;

        Ok(ret)
    }
}
