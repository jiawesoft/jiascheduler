use anyhow::Result;

mod bundle_script;
mod dashboard;
mod exec_history;
mod schedule;
mod supervisor;
mod timer;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set,
};
use sea_query::Expr;

use crate::{
    entity::{
        self, executor, instance, job, job_exec_history, job_running_status, job_schedule_history,
        prelude::*,
    },
    state::AppContext,
};
use sea_orm::JoinType;

pub mod types;

pub struct JobLogic<'a> {
    ctx: &'a AppContext,
}

impl<'a> JobLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }
    pub async fn save_job(
        &self,
        model: entity::job::ActiveModel,
    ) -> Result<entity::job::ActiveModel> {
        let model = model.save(&self.ctx.db).await?;
        Ok(model)
    }

    pub async fn query_job(
        &self,
        created_user: Option<String>,
        job_type: Option<String>,
        name: Option<String>,
        updated_time_range: Option<(String, String)>,
        default_id: Option<u64>,
        default_eid: Option<String>,
        team_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::JobRelatedExecutorModel>, u64)> {
        let model = Job::find()
            .column_as(executor::Column::Name, "executor_name")
            .column_as(executor::Column::Command, "executor_command")
            .column_as(executor::Column::Platform, "executor_platform")
            .join_rev(
                JoinType::LeftJoin,
                Executor::belongs_to(Job)
                    .from(executor::Column::Id)
                    .to(job::Column::ExecutorId)
                    .into(),
            )
            .apply_if(created_user, |query, v| {
                query.filter(job::Column::CreatedUser.eq(v))
            })
            .apply_if(job_type, |query, v| {
                query.filter(job::Column::JobType.eq(v))
            })
            .apply_if(team_id, |q, v| q.filter(job::Column::TeamId.eq(v)))
            .apply_if(name, |q, v| q.filter(job::Column::Name.contains(v)))
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    job::Column::UpdatedTime
                        .gt(v.0)
                        .and(job::Column::UpdatedTime.lt(v.1)),
                )
            });

        let total = model.clone().count(&self.ctx.db).await?;
        let list = model
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(job::Column::Id.eq(v)))
            })
            .apply_if(default_eid, |query, v| {
                query.order_by_desc(Expr::expr(job::Column::Eid.eq(v)))
            })
            .order_by_desc(entity::job::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn delete_job(&self, eid: String) -> Result<u64> {
        let record = JobExecHistory::find()
            .filter(job_exec_history::Column::Eid.eq(&eid))
            .one(&self.ctx.db)
            .await?;
        if record.is_some() {
            anyhow::bail!("forbidden to delete the executed jobs")
        }

        let ret = Job::delete(entity::job::ActiveModel {
            eid: Set(eid),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;
        Ok(ret.rows_affected)
    }

    pub async fn query_run_list(
        &self,
        created_user: Option<String>,
        bind_ip: Option<String>,
        team_id: Option<u64>,
        schedule_name: Option<String>,
        schedule_type: Option<String>,
        job_type: Option<String>,
        updated_time_range: Option<(String, String)>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::RunStatusRelatedScheduleJobModel>, u64)> {
        let model = JobRunningStatus::find()
            .column_as(instance::Column::Ip, "bind_ip")
            .column_as(instance::Column::Namespace, "bind_namespace")
            .column_as(job_schedule_history::Column::Name, "schedule_name")
            .column_as(job_schedule_history::Column::DispatchData, "dispatch_data")
            .column_as(executor::Column::Name, "executor_name")
            .column_as(job::Column::ExecutorId, "executor_id")
            .column_as(
                job_schedule_history::Column::SnapshotData,
                "schedule_snapshot_data",
            )
            .join_rev(
                JoinType::LeftJoin,
                JobScheduleHistory::belongs_to(JobRunningStatus)
                    .from(job_schedule_history::Column::ScheduleId)
                    .to(job_running_status::Column::ScheduleId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(JobRunningStatus)
                    .from(instance::Column::InstanceId)
                    .to(job_running_status::Column::InstanceId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Job::belongs_to(JobRunningStatus)
                    .from(job::Column::Eid)
                    .to(job_running_status::Column::Eid)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Executor::belongs_to(Job)
                    .from(executor::Column::Id)
                    .to(job::Column::ExecutorId)
                    .into(),
            )
            .apply_if(schedule_type, |query, v| {
                query.filter(job_running_status::Column::ScheduleType.eq(v))
            })
            .apply_if(job_type, |query, v| {
                query.filter(job_running_status::Column::JobType.eq(v))
            })
            .apply_if(created_user, |query, v| {
                query.filter(job_running_status::Column::UpdatedUser.eq(v))
            })
            .apply_if(bind_ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            })
            .apply_if(schedule_name, |query, v| {
                query.filter(job_schedule_history::Column::Name.contains(v))
            })
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    job_running_status::Column::UpdatedTime
                        .gt(v.0)
                        .and(job::Column::UpdatedTime.lt(v.1)),
                )
            })
            .apply_if(team_id, |q, v| q.filter(job::Column::TeamId.eq(v)));

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_desc(entity::job_running_status::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }
}
