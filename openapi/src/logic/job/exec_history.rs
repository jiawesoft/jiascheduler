use crate::entity::{instance, job_exec_history, job_schedule_history, prelude::*};
use anyhow::Result;
use sea_orm::{
    ColumnTrait, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait,
};

use super::types::ExecHistoryRelatedScheduleModel;
use super::JobLogic;

impl<'a> JobLogic<'a> {
    pub async fn create_exec_history(&self) {}

    pub async fn query_exec_history(
        &self,
        job_type: String,
        schedule_id: Option<String>,
        schedule_type: Option<String>,
        eid: Option<String>,
        schedule_name: Option<String>,
        username: Option<String>,
        instance_id: Option<String>,
        bind_namespace: Option<String>,
        bind_ip: Option<String>,
        start_time_range: Option<(String, String)>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ExecHistoryRelatedScheduleModel>, u64)> {
        let model = JobExecHistory::find()
            .column_as(job_schedule_history::Column::Name, "schedule_name")
            .column(job_schedule_history::Column::CreatedUser)
            .column(instance::Column::Ip)
            .column(instance::Column::Namespace)
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
            .filter(job_schedule_history::Column::CreatedUser.eq(username))
            .filter(job_exec_history::Column::JobType.eq(job_type))
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
            });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_desc(job_exec_history::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((list, total))
    }
}
