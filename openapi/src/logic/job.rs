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
        self, executor, instance, job, job_bundle_script, job_exec_history, job_running_status,
        job_schedule_history, prelude::*, team,
    },
    state::AppContext,
};
use sea_orm::JoinType;

use super::types::UserInfo;

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

    pub async fn can_write_bundle_script(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        eid: &str,
    ) -> Result<bool> {
        let ok = self.ctx.can_manage_job(&user_info.user_id).await?;
        if ok {
            Ok(true)
        } else {
            let v = JobBundleScript::find()
                .apply_if(team_id, |q, v| {
                    q.filter(
                        job_bundle_script::Column::TeamId
                            .eq(v)
                            .and(job_bundle_script::Column::Eid.eq(eid)),
                    )
                })
                .apply_if(
                    team_id.map_or(Some(user_info.username.clone()), |_| None),
                    |q, v| {
                        q.filter(
                            job_bundle_script::Column::Eid
                                .eq(eid)
                                .and(job_bundle_script::Column::CreatedUser.eq(v)),
                        )
                    },
                )
                .one(&self.ctx.db)
                .await?
                .map_or(false, |_| true);
            Ok(v)
        }
    }

    pub async fn can_write_bundle_script_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        id: u64,
    ) -> Result<bool> {
        let ok = self.ctx.can_manage_job(&user_info.user_id).await?;
        if ok {
            Ok(true)
        } else {
            let v = JobBundleScript::find()
                .apply_if(team_id, |q, v| {
                    q.filter(
                        job_bundle_script::Column::TeamId
                            .eq(v)
                            .and(job_bundle_script::Column::Id.eq(id)),
                    )
                })
                .apply_if(
                    team_id.map_or(Some(user_info.username.clone()), |_| None),
                    |q, v| {
                        q.filter(
                            job_bundle_script::Column::Id
                                .eq(id)
                                .and(job_bundle_script::Column::CreatedUser.eq(v)),
                        )
                    },
                )
                .one(&self.ctx.db)
                .await?
                .map_or(false, |_| true);
            Ok(v)
        }
    }

    pub async fn can_write_job(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        eid: &str,
    ) -> Result<bool> {
        let ok = self.ctx.can_manage_job(&user_info.user_id).await?;
        if ok {
            Ok(Job::find()
                .filter(job::Column::Eid.eq(eid))
                .one(&self.ctx.db)
                .await?
                .is_some())
        } else {
            let v = Job::find()
                .apply_if(team_id, |q, v| {
                    q.filter(job::Column::TeamId.eq(v).and(job::Column::Eid.eq(eid)))
                })
                .apply_if(
                    team_id.map_or(Some(&user_info.username), |_| None),
                    |q, v| q.filter(job::Column::Eid.eq(eid).and(job::Column::CreatedUser.eq(v))),
                )
                .one(&self.ctx.db)
                .await?
                .is_some();
            Ok(v)
        }
    }

    pub async fn can_write_job_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        job_id: u64,
    ) -> Result<bool> {
        let ok = self.ctx.can_manage_job(&user_info.username).await?;
        if ok {
            Ok(true)
        } else {
            let v = Job::find()
                .apply_if(team_id, |q, v| {
                    q.filter(job::Column::TeamId.eq(v).and(job::Column::Id.eq(job_id)))
                })
                .apply_if(
                    team_id.map_or(Some(&user_info.username), |_| None),
                    |q, v| {
                        q.filter(
                            job::Column::Id
                                .eq(job_id)
                                .and(job::Column::CreatedUser.eq(v)),
                        )
                    },
                )
                .one(&self.ctx.db)
                .await?
                .map_or(false, |_| true);
            Ok(v)
        }
    }

    pub async fn can_dispatch_job(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        schedule_user: Option<&str>,
        eid: &str,
    ) -> Result<bool> {
        if !self.can_write_job(user_info, team_id, eid).await? {
            return Ok(false);
        }
        let Some(schedule_user) = schedule_user else {
            return Ok(true);
        };
        if self.ctx.can_manage_instance(&user_info.user_id).await? {
            return Ok(true);
        }
        Ok(schedule_user.eq(&user_info.username))
    }

    pub async fn get_authorized_job(
        &self,
        username: &str,
        team_id: Option<u64>,
        eid: &str,
    ) -> Result<job::Model> {
        let ok = self.ctx.can_manage_job(username).await?;
        if ok {
            let v = Job::find()
                .filter(job::Column::Eid.eq(eid))
                .one(&self.ctx.db)
                .await?;
            v.ok_or(anyhow::anyhow!("no permission to write job"))
        } else {
            Job::find()
                .apply_if(team_id, |q, v| {
                    q.filter(job::Column::TeamId.eq(v).and(job::Column::Eid.eq(eid)))
                })
                .apply_if(team_id.map_or(Some(username), |_| None), |q, v| {
                    q.filter(job::Column::Eid.eq(eid).and(job::Column::CreatedUser.eq(v)))
                })
                .one(&self.ctx.db)
                .await?
                .ok_or(anyhow::anyhow!("no permission to write job"))
        }
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
        tag_ids: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::JobRelatedExecutorModel>, u64)> {
        let select = Job::find()
            .column_as(team::Column::Name, "team_name")
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
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Job)
                    .from(team::Column::Id)
                    .to(job::Column::TeamId)
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

        match tag_ids {
            Some(v) if v.len() > 0 => todo!(),
            _ => {}
        };

        let total = select.clone().count(&self.ctx.db).await?;
        let list = select
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
            .column_as(team::Column::Id, "team_id")
            .column_as(team::Column::Name, "team_name")
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
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Job)
                    .from(team::Column::Id)
                    .to(job::Column::TeamId)
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
