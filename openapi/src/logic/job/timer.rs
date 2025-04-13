use anyhow::Result;
use automate::scheduler::types::ScheduleType;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use sea_query::Query;

use super::{types::JobTimerRelatedJobModel, JobLogic};
use crate::{
    entity::{
        executor, job, job_exec_history, job_schedule_history, job_timer, prelude::*, tag_resource,
        team,
    },
    logic::types::{ResourceType, UserInfo},
};

impl<'a> JobLogic<'a> {
    pub async fn save_job_timer(
        &self,
        active_model: job_timer::ActiveModel,
    ) -> Result<job_timer::ActiveModel> {
        Ok(active_model.save(&self.ctx.db).await?)
    }

    pub async fn query_job_timer(
        &self,
        team_id: Option<u64>,
        created_user: Option<&String>,
        name: Option<String>,
        job_type: Option<String>,
        updated_time_range: Option<(String, String)>,
        tag_ids: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<JobTimerRelatedJobModel>, u64)> {
        let mut select = job_timer::Entity::find()
            .column_as(job::Column::Id, "job_id")
            .column_as(job::Column::Name, "job_name")
            .column(job::Column::ExecutorId)
            .column_as(executor::Column::Name, "executor_name")
            .column_as(executor::Column::Platform, "executor_platform")
            .column_as(team::Column::Id, "team_id")
            .column_as(team::Column::Name, "team_name")
            .join_rev(
                JoinType::LeftJoin,
                Job::belongs_to(JobTimer)
                    .from(job::Column::Eid)
                    .to(job_timer::Column::Eid)
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
            .filter(job_timer::Column::IsDeleted.eq(false))
            .apply_if(name, |query, v| {
                query.filter(job_timer::Column::Name.contains(v))
            })
            .apply_if(created_user, |query, v| {
                query.filter(job_timer::Column::CreatedUser.eq(v))
            })
            .apply_if(job_type, |query, v| {
                query.filter(job_timer::Column::JobType.eq(v))
            })
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    job_timer::Column::UpdatedTime
                        .gt(v.0)
                        .and(job_timer::Column::UpdatedTime.lt(v.1)),
                )
            })
            .apply_if(team_id, |q, v| q.filter(job::Column::TeamId.eq(v)));

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
            .order_by_desc(job_timer::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((list, total))
    }

    pub async fn delete_job_timer(&self, user_info: &UserInfo, id: u64) -> Result<u64> {
        let job_timer_record = JobTimer::find()
            .filter(job_timer::Column::Id.eq(id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow::anyhow!(
                "cannot found this job's supervisor by id {id}"
            ))?;

        let ret = JobTimer::update_many()
            .set(job_timer::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_timer::Column::Id.eq(id))
            .exec(&self.ctx.db)
            .await?;

        JobScheduleHistory::update_many()
            .set(job_schedule_history::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_schedule_history::Column::Eid.eq(&job_timer_record.eid))
            .filter(job_schedule_history::Column::ScheduleType.eq(ScheduleType::Timer.to_string()))
            .exec(&self.ctx.db)
            .await?;

        JobExecHistory::delete_many()
            .filter(job_exec_history::Column::Eid.eq(&job_timer_record.eid))
            .filter(
                Condition::any().add(
                    job_exec_history::Column::ScheduleId.in_subquery(
                        Query::select()
                            .column(job_schedule_history::Column::ScheduleId)
                            .and_where(
                                job_schedule_history::Column::ScheduleType
                                    .eq(ScheduleType::Timer.to_string()),
                            )
                            .and_where(job_schedule_history::Column::IsDeleted.eq(true))
                            .from(JobScheduleHistory)
                            .to_owned(),
                    ),
                ),
            )
            .exec(&self.ctx.db)
            .await?;

        Ok(ret.rows_affected)
    }
}
