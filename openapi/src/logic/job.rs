use anyhow::{anyhow, Ok, Result};

mod bundle_script;
mod dashboard;
mod exec_history;
mod schedule;
mod supervisor;
mod timer;

use automate::scheduler::types::ScheduleType;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};
use sea_query::{Expr, Query};

use crate::{
    entity::{
        self, executor, instance, job, job_bundle_script, job_exec_history, job_running_status,
        job_schedule_history, job_supervisor, job_timer, prelude::*, tag_resource, team,
        team_member,
    },
    state::AppContext,
};
use sea_orm::JoinType;

use super::types::{ResourceType, UserInfo};

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
        eid: Option<String>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(eid) = eid else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(job_record) = JobBundleScript::find()
            .filter(job_bundle_script::Column::Eid.eq(eid))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if job_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(job_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(job_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_write_bundle_script_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        id: Option<u64>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(id) = id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(bundle_script_record) = JobBundleScript::find()
            .filter(job_bundle_script::Column::Id.eq(id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if bundle_script_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(bundle_script_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(bundle_script_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_write_job(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        eid: Option<String>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(eid) = eid else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(job_record) = Job::find()
            .filter(job::Column::Eid.eq(eid))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if job_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(job_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(job_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_write_job_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        job_id: Option<u64>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(job_id) = job_id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(job_record) = Job::find()
            .filter(job::Column::Id.eq(job_id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if job_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(job_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(job_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_write_schedule_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        schedule_id: Option<String>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(schedule_id) = schedule_id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(schedule_record) = JobScheduleHistory::find()
            .filter(job_schedule_history::Column::ScheduleId.eq(schedule_id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        let Some(job_record) = Job::find()
            .filter(job::Column::Eid.eq(schedule_record.eid))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if schedule_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(job_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(job_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_write_job_timer_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        timer_id: Option<u64>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(timer_id) = timer_id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(timer_record) = JobTimer::find()
            .filter(job_timer::Column::Id.eq(timer_id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        let Some(job_record) = Job::find()
            .filter(job::Column::Eid.eq(timer_record.eid))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if timer_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(job_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(job_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_write_job_supervisor_by_id(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        supvervisor_id: Option<u64>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(supervisor_id) = supvervisor_id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(supervisor_record) = JobSupervisor::find()
            .filter(job_supervisor::Column::Id.eq(supervisor_id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        let Some(job_record) = Job::find()
            .filter(job::Column::Eid.eq(supervisor_record.eid))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if supervisor_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(job_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(job_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn can_dispatch_job(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        schedule_user: Option<&str>,
        eid: &str,
    ) -> Result<bool> {
        if !self
            .can_write_job(user_info, team_id, Some(eid.to_string()))
            .await?
        {
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
        let mut select = Job::find()
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
            .filter(job::Column::IsDeleted.eq(false))
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

    pub async fn delete_job(&self, user_info: &UserInfo, eid: String) -> Result<u64> {
        if JobTimer::find()
            .filter(job_timer::Column::Eid.eq(&eid))
            .filter(job_timer::Column::IsDeleted.eq(false))
            .one(&self.ctx.db)
            .await?
            .is_some()
        {
            anyhow::bail!("do not delete jobs linked to timers")
        }

        if JobSupervisor::find()
            .filter(job_supervisor::Column::Eid.eq(&eid))
            .filter(job_supervisor::Column::IsDeleted.eq(false))
            .one(&self.ctx.db)
            .await?
            .is_some()
        {
            anyhow::bail!("do not delete jobs linked to supervisors")
        }

        let ret = Job::update_many()
            .set(job::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job::Column::Eid.eq(&eid))
            .exec(&self.ctx.db)
            .await?;

        JobRunningStatus::update_many()
            .set(job_running_status::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_running_status::Column::Eid.eq(&eid))
            .exec(&self.ctx.db)
            .await?;

        JobScheduleHistory::update_many()
            .set(job_schedule_history::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_schedule_history::Column::Eid.eq(&eid))
            .exec(&self.ctx.db)
            .await?;

        JobExecHistory::delete_many()
            .filter(job_exec_history::Column::Eid.eq(&eid))
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
        tag_ids: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::RunStatusRelatedScheduleJobModel>, u64)> {
        let mut select = JobRunningStatus::find()
            .column_as(job::Column::Id, "job_id")
            .column_as(instance::Column::Ip, "bind_ip")
            .column_as(instance::Column::Namespace, "bind_namespace")
            .column_as(instance::Column::Status, "is_online")
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
            .filter(job_running_status::Column::IsDeleted.eq(false))
            // .filter(job_schedule_history::Column::IsDeleted.eq(false))
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
            .order_by_desc(entity::job_running_status::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn delete_running_status(
        &self,
        user_info: &UserInfo,
        eid: &str,
        schedule_type: ScheduleType,
        instance_id: &str,
    ) -> Result<u64> {
        let ret = JobRunningStatus::update_many()
            .set(job_running_status::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_running_status::Column::Eid.eq(eid))
            .filter(job_running_status::Column::ScheduleType.eq(schedule_type.to_string()))
            .filter(job_running_status::Column::InstanceId.eq(instance_id))
            .exec(&self.ctx.db)
            .await?;

        JobExecHistory::delete_many()
            .filter(job_exec_history::Column::Eid.eq(eid))
            .filter(
                job_exec_history::Column::ScheduleId.in_subquery(
                    Query::select()
                        .column(job_schedule_history::Column::ScheduleId)
                        .from(job_schedule_history::Entity)
                        .and_where(
                            job_schedule_history::Column::ScheduleType
                                .eq(schedule_type.to_string()),
                        )
                        .to_owned(),
                ),
            )
            .filter(job_exec_history::Column::InstanceId.eq(instance_id))
            .exec(&self.ctx.db)
            .await?;

        Ok(ret.rows_affected)
    }

    pub async fn get_job_by_eid(&self, eid: &str) -> Result<Option<job::Model>> {
        let model = Job::find()
            .filter(job::Column::Eid.eq(eid))
            .one(&self.ctx.db)
            .await?;
        Ok(model)
    }

    pub async fn get_default_validate_team_id_by_job(
        &self,
        user_info: &UserInfo,
        eid: Option<&str>,
        default_team_id: Option<u64>,
    ) -> Result<Option<u64>> {
        let Some(eid) = eid else {
            return Ok(default_team_id);
        };

        let record = self
            .get_job_by_eid(eid)
            .await?
            .ok_or(anyhow!("not found the job by {eid}"))?;
        let team_id = if record.team_id == 0 {
            return Ok(default_team_id);
        } else {
            Some(record.team_id)
        };
        let ok = self.can_write_job(user_info, team_id, None).await?;

        if ok {
            Ok(team_id)
        } else {
            Ok(None)
        }
    }
}
