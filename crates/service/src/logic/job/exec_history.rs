use crate::entity::{
    instance, job, job_exec_history, job_schedule_history, prelude::*, tag_resource, team,
};
use crate::logic::types::ResourceType;
use anyhow::Result;
use automate::scheduler::types::ScheduleType;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait,
};
use sea_query::Query;

use super::types::ExecHistoryRelatedScheduleModel;
use super::JobLogic;

impl<'a> JobLogic<'a> {
    pub async fn create_exec_history(&self) {}

    pub async fn query_exec_history(
        &self,
        job_type: String,
        schedule_id: Option<String>,
        schedule_type: Option<String>,
        team_id: Option<u64>,
        eid: Option<String>,
        schedule_name: Option<String>,
        username: Option<String>,
        instance_id: Option<String>,
        bind_namespace: Option<String>,
        bind_ip: Option<String>,
        start_time_range: Option<(String, String)>,
        tag_ids: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ExecHistoryRelatedScheduleModel>, u64)> {
        let mut select = JobExecHistory::find()
            .column_as(job_schedule_history::Column::Name, "schedule_name")
            .column_as(team::Column::Id, "team_id")
            .column_as(team::Column::Name, "team_name")
            .column_as(job::Column::Id, "job_id")
            .column_as(job::Column::Name, "job_name")
            .column(instance::Column::Ip)
            .column(instance::Column::Namespace)
            .column_as(instance::Column::Status, "is_online")
            .join_rev(
                JoinType::LeftJoin,
                Job::belongs_to(JobExecHistory)
                    .from(job::Column::Eid)
                    .to(job_exec_history::Column::Eid)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Job)
                    .from(team::Column::Id)
                    .to(job::Column::TeamId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                JobScheduleHistory::belongs_to(JobExecHistory)
                    .from(job_schedule_history::Column::ScheduleId)
                    .to(job_exec_history::Column::ScheduleId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(JobExecHistory)
                    .from(instance::Column::InstanceId)
                    .to(job_exec_history::Column::InstanceId)
                    .into(),
            )
            .filter(job_exec_history::Column::JobType.eq(job_type))
            .apply_if(username, |q, v| {
                q.filter(job_exec_history::Column::CreatedUser.eq(v))
            })
            .apply_if(schedule_type, |query, v| {
                query.filter(job_schedule_history::Column::ScheduleType.eq(v))
            })
            .apply_if(schedule_name, |query, v| {
                query.filter(job_schedule_history::Column::Name.contains(v))
            })
            .apply_if(schedule_id, |query, v| {
                query.filter(job_exec_history::Column::ScheduleId.eq(v))
            })
            .apply_if(bind_namespace, |query, v| {
                query.filter(instance::Column::Namespace.contains(v))
            })
            .apply_if(bind_ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            })
            .apply_if(instance_id, |query, v| {
                query.filter(instance::Column::InstanceId.eq(v))
            })
            .apply_if(eid, |query, v| {
                query.filter(job_exec_history::Column::Eid.eq(v))
            })
            .apply_if(start_time_range, |query, v| {
                query.filter(
                    job_exec_history::Column::StartTime
                        .gt(v.0)
                        .and(job_exec_history::Column::EndTime.lt(v.1)),
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
            .order_by_desc(job_exec_history::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((list, total))
    }

    pub async fn delete_exec_history(
        &self,
        ids: Option<Vec<u64>>,
        schedule_id: Option<String>,
        schedule_type: Option<ScheduleType>,
        instance_id: Option<String>,
        eid: Option<String>,
        team_id: Option<u64>,
        username: Option<String>,
        time_range_start: Option<String>,
        time_range_end: Option<String>,
    ) -> Result<u64> {
        let ret = JobExecHistory::delete_many()
            .apply_if(schedule_type, |q, v| {
                q.filter(
                    job_exec_history::Column::ScheduleId.in_subquery(
                        Query::select()
                            .column(job_schedule_history::Column::ScheduleId)
                            .from(job_schedule_history::Entity)
                            .and_where(job_schedule_history::Column::ScheduleType.eq(v.to_string()))
                            .to_owned(),
                    ),
                )
            })
            .apply_if(team_id, |q, v| {
                q.filter(
                    job_exec_history::Column::Eid.in_subquery(
                        Query::select()
                            .column(job::Column::Eid)
                            .from(job::Entity)
                            .and_where(job::Column::TeamId.eq(v))
                            .to_owned(),
                    ),
                )
            })
            .apply_if(ids, |q, v| q.filter(job_exec_history::Column::Id.is_in(v)))
            .apply_if(instance_id, |q, v| {
                q.filter(job_exec_history::Column::InstanceId.eq(v))
            })
            .apply_if(schedule_id, |q, v| {
                q.filter(job_exec_history::Column::ScheduleId.eq(v))
            })
            .apply_if(username, |q, v| {
                q.filter(job_exec_history::Column::CreatedUser.eq(v))
            })
            .apply_if(eid, |q, v| q.filter(job_exec_history::Column::Eid.eq(v)))
            .apply_if(time_range_start, |query, v| {
                query.filter(job_exec_history::Column::StartTime.gt(v))
            })
            .apply_if(time_range_end, |query, v| {
                query.filter(job_exec_history::Column::EndTime.gt(v))
            })
            .exec(&self.ctx.db)
            .await?;

        Ok(ret.rows_affected)
    }
}
