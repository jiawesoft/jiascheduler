use crate::{
    entity::{
        executor, instance, job, job_running_status, job_supervisor, prelude::*, tag_resource, team,
    },
    logic::types::{ResourceType, UserInfo},
};
use anyhow::Result;
use automate::scheduler::types::{RunStatus, ScheduleStatus, ScheduleType};
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use sea_query::Query;

use super::{Executor, Job, JobLogic, JobSupervisor, Team, types::JobSupervisorRelatedJobModel};

impl<'a> JobLogic<'a> {
    pub async fn query_job_supervisor(
        &self,
        created_user: Option<&String>,
        name: Option<String>,
        eid: Option<String>,
        team_id: Option<u64>,
        updated_time_range: Option<(String, String)>,
        tag_ids: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<JobSupervisorRelatedJobModel>, u64)> {
        let mut select = job_supervisor::Entity::find()
            .column_as(job::Column::Id, "job_id")
            .column_as(job::Column::Name, "job_name")
            .column_as(executor::Column::Name, "executor_name")
            .column_as(executor::Column::Platform, "executor_platform")
            .column_as(team::Column::Name, "team_name")
            .column_as(team::Column::Id, "team_id")
            .column(job::Column::ExecutorId)
            .join_rev(
                JoinType::LeftJoin,
                Job::belongs_to(JobSupervisor)
                    .from(job::Column::Eid)
                    .to(job_supervisor::Column::Eid)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Executor::belongs_to(Job)
                    .from(executor::Column::Id)
                    .to(job::Column::ExecutorId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Job)
                    .from(team::Column::Id)
                    .to(job::Column::TeamId)
                    .into(),
            )
            .filter(job_supervisor::Column::IsDeleted.eq(false))
            .apply_if(name, |query, v| {
                query.filter(job_supervisor::Column::Name.contains(v))
            })
            .apply_if(created_user, |query, v| {
                query.filter(job_supervisor::Column::CreatedUser.eq(v))
            })
            .apply_if(team_id, |q, v| q.filter(job::Column::TeamId.eq(v)))
            .apply_if(eid, |q, v| q.filter(job_supervisor::Column::Eid.eq(v)))
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    job_supervisor::Column::UpdatedTime
                        .gt(v.0)
                        .and(job_supervisor::Column::UpdatedTime.lt(v.1)),
                )
            });

        match tag_ids {
            Some(v) if v.len() > 0 => {
                select = select.filter(
                    Condition::any().add(
                        job::Column::Id.in_subquery(
                            Query::select()
                                .column(tag_resource::Column::ResourceId)
                                .and_where(
                                    tag_resource::Column::ResourceType
                                        .eq(ResourceType::Job.to_string())
                                        .and(tag_resource::Column::TagId.is_in(v)),
                                )
                                .from(TagResource)
                                .to_owned(),
                        ),
                    ),
                );
            }
            _ => {}
        };

        let total = select.clone().count(&self.ctx.db).await?;

        let list = select
            .order_by_desc(job_supervisor::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((list, total))
    }

    pub async fn save_job_supervisor(
        &self,
        active_model: job_supervisor::ActiveModel,
    ) -> Result<job_supervisor::ActiveModel> {
        Ok(active_model.save(&self.ctx.db).await?)
    }

    pub async fn delete_job_supervisor(&self, user_info: &UserInfo, id: u64) -> Result<u64> {
        let ret = JobSupervisor::update_many()
            .set(job_supervisor::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_supervisor::Column::Id.eq(id))
            .exec(&self.ctx.db)
            .await?;

        Ok(ret.rows_affected)
    }

    pub async fn fix_running_status(&self, ip: &str, mac_addr: &str) -> Result<()> {
        let instance_ids = Instance::find()
            .filter(instance::Column::Ip.eq(ip))
            .filter(instance::Column::MacAddr.eq(mac_addr))
            .select_only()
            .column(instance::Column::InstanceId)
            .into_tuple::<String>()
            .all(&self.ctx.db)
            .await?;
        if instance_ids.is_empty() {
            return Ok(());
        }

        let _ = JobRunningStatus::update_many()
            .set(job_running_status::ActiveModel {
                schedule_status: sea_orm::ActiveValue::Set(
                    ScheduleStatus::Unsupervised.to_string(),
                ),
                run_status: sea_orm::ActiveValue::Set(RunStatus::Stop.to_string()),
                ..Default::default()
            })
            .filter(job_running_status::Column::InstanceId.is_in(instance_ids.clone()))
            .filter(job_running_status::Column::ScheduleType.eq(ScheduleType::Daemon.to_string()))
            .filter(
                job_running_status::Column::ScheduleStatus
                    .eq(ScheduleStatus::Supervising.to_string())
                    .or(job_running_status::Column::RunStatus.eq(RunStatus::Running.to_string())),
            )
            .exec(&self.ctx.db)
            .await?;

        let _ = JobRunningStatus::update_many()
            .set(job_running_status::ActiveModel {
                run_status: sea_orm::ActiveValue::Set(RunStatus::Stop.to_string()),
                ..Default::default()
            })
            .filter(job_running_status::Column::InstanceId.is_in(instance_ids))
            .filter(
                Condition::any()
                    .add(
                        job_running_status::Column::ScheduleType.eq(ScheduleType::Once.to_string()),
                    )
                    .add(
                        job_running_status::Column::ScheduleStatus
                            .eq(ScheduleStatus::Unscheduled.to_string())
                            .and(job_running_status::Column::RunStatus.is_in(vec![
                                RunStatus::Running.to_string(),
                                RunStatus::Prepare.to_string(),
                            ])),
                    ),
            )
            .exec(&self.ctx.db)
            .await?;

        Ok(())
    }
}
