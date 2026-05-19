use std::{pin::Pin, str::FromStr, sync::Arc};

use crate::{
    entity::prelude::*,
    logic::types::{CustomTimerExpr, ResourceType, UserInfo},
};
use anyhow::{Result, anyhow};

use chrono::{Local, Utc};
use entity::{tag_resource, team, workflow, workflow_timer, workflow_version};
use local_ip_address::local_ip;
use redis::{
    AsyncCommands, from_redis_value,
    streams::{StreamMaxlen, StreamReadOptions, StreamReadReply},
};
use redis_macros::{FromRedisValue, ToRedisArgs};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, FromQueryResult, JoinType, PaginatorTrait,
    QueryOrder, QuerySelect, QueryTrait, prelude::DateTimeLocal,
};
use sea_orm::{EntityTrait, QueryFilter};
use sea_query::{ConditionType, Expr, IntoCondition, Query};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::logic::workflow::WorkflowLogic;

#[derive(Serialize, Deserialize, FromRedisValue, ToRedisArgs, Clone)]
pub enum WorkflowTimerTask {
    StartTimer(u64),
    StopTimer(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize, FromQueryResult)]
pub struct WorkflowTimerWithTeamModel {
    pub id: u64,
    pub name: String,
    pub team_id: u64,
    pub team_name: Option<String>,
    pub workflow_id: u64,
    pub workflow_name: String,
    pub version_id: u64,
    pub timer_expr: serde_json::Value,
    pub schedule_guid: String,
    pub is_active: bool,
    pub startup_error: String,
    pub info: String,
    pub created_user: String,
    pub updated_user: String,
    pub process_args: Option<serde_json::Value>,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
}

impl<'a> WorkflowLogic<'a> {
    pub async fn new_scheduler(&self) -> Result<JobScheduler> {
        let sched = JobScheduler::new().await?;
        sched.start().await?;
        Ok(sched)
    }

    pub async fn recv_timer_msg(
        &self,
        is_continue: Arc<RwLock<bool>>,
        mut cb: impl Sync
        + Send
        + FnMut(
            String,
            WorkflowTimerTask,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    ) -> Result<()> {
        if !*is_continue.read().await {
            return Ok(());
        }

        let redis_client = self.ctx.redis();
        let mut conn = redis_client.get_multiplexed_async_connection().await?;

        let ret: String = conn
            .xgroup_create_mkstream(Self::WORKFLOW_TIMER_TOPIC, Self::CONSUMER_GROUP, "$")
            .await
            .map_or_else(
                |e| {
                    if e.code() != Some("BUSYGROUP") {
                        warn!("failed create workflow timer stream group - {}", e);
                    }

                    "".to_string()
                },
                |v| v,
            );

        info!("create workflow timer stream group {}", ret);

        let opts = StreamReadOptions::default()
            .group(Self::CONSUMER_GROUP, local_ip()?.to_string())
            .block(100)
            .count(100);

        loop {
            if !*is_continue.read().await {
                return Ok(());
            }
            let ret: StreamReadReply = conn
                .xread_options(&[Self::WORKFLOW_TIMER_TOPIC], &[">"], &opts)
                .await?;

            if let Err(e) = conn
                .xtrim::<_, u64>(Self::WORKFLOW_TIMER_TOPIC, StreamMaxlen::Equals(5000))
                .await
            {
                error!("failed to trim workflow timer stream - {e}");
            };

            for stream_key in ret.keys {
                let msg_key = stream_key.key;

                for stream_id in stream_key.ids {
                    for (k, v) in stream_id.map {
                        let ret = match from_redis_value::<WorkflowTimerTask>(&v) {
                            Ok(msg) => cb(k, msg).await,
                            Err(e) => {
                                error!("failed to parse workflow timer val - {e}");
                                Ok(())
                            }
                        };

                        if let Err(e) = ret {
                            error!("failed to handle workflow timer msg - {e}");
                        }

                        let _: i32 = conn
                            .xack(
                                msg_key.clone(),
                                Self::CONSUMER_GROUP,
                                &[stream_id.id.clone()],
                            )
                            .await
                            .map_or_else(
                                |v| {
                                    error!("faile to exec workflow timer msg xack - {}", v);
                                    0
                                },
                                |v| v,
                            );
                    }
                }
            }
        }
    }

    pub async fn send_timer_msg<'b>(&self, msg: WorkflowTimerTask) -> Result<String> {
        let data = &[("t", msg)];
        let mut conn = self.ctx.redis().get_multiplexed_async_connection().await?;
        let v: String = conn.xadd(Self::WORKFLOW_TIMER_TOPIC, "*", data).await?;
        Ok(v)
    }

    pub async fn init_timer(&self, sched: JobScheduler) -> Result<()> {
        let all_active_timers = WorkflowTimer::find()
            .filter(workflow_timer::Column::IsActive.eq(true))
            .filter(workflow_timer::Column::IsDeleted.eq(false))
            .all(&self.ctx.db)
            .await?;

        for timer in all_active_timers {
            self.add_job_to_scheduler(timer, sched.clone()).await?;
        }
        Ok(())
    }

    async fn add_job_to_scheduler(
        &self,
        timer: workflow_timer::Model,
        sched: JobScheduler,
    ) -> Result<()> {
        let timer_expr: CustomTimerExpr = serde_json::from_value(timer.timer_expr)?;
        let ctx = self.ctx.clone();
        let workflow_id = timer.workflow_id;
        let version_id = timer.version_id;
        let timer_id = timer.id;
        let workflow_record = Workflow::find()
            .filter(workflow::Column::Id.eq(workflow_id))
            .filter(workflow::Column::IsDeleted.eq(false))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("not found workflow {}", workflow_id))?;

        let process_args = timer.process_args.map(serde_json::from_value).transpose()?;
        let handler = move |uuid, mut l: JobScheduler| {
            let ctx_clone = ctx.clone();
            let now = Local::now();
            let username = timer.created_user.clone();
            let process_args = process_args.clone();
            let process_name = format!(
                "{}-{}",
                workflow_record.name.clone(),
                now.format("%Y%m%d%H%M%S").to_string()
            );
            Box::pin(async move {
                let svc = ctx_clone.service();
                if let Err(e) = svc
                    .workflow
                    .start_process(
                        &UserInfo {
                            username,
                            ..Default::default()
                        },
                        workflow_id,
                        version_id,
                        Some(timer_id),
                        process_name,
                        process_args,
                    )
                    .await
                {
                    error!(
                        "failed start process, {}, workflow_id: {}, version_id: {}",
                        e.to_string(),
                        workflow_id,
                        version_id
                    );
                    match l.remove(&uuid).await {
                        Ok(_) => {
                            let _ = WorkflowTimer::update(workflow_timer::ActiveModel {
                                id: Set(timer_id),
                                is_active: Set(false),
                                startup_error: Set(e.to_string()),
                                ..Default::default()
                            })
                            .exec(&ctx_clone.db)
                            .await
                            .map_err(|e| error!("failed update timer {}", e.to_string()));
                        }
                        Err(e) => error!("failed remove invalid timer {}", e.to_string()),
                    };
                }

                let next_tick = l.next_tick_for_job(uuid).await;
                match next_tick {
                    Ok(Some(ts)) => {
                        let _ = WorkflowTimer::update(workflow_timer::ActiveModel {
                            id: Set(timer_id),
                            prev_time: Set(Some(now)),
                            next_time: Set(Some(ts.with_timezone(&Local))),
                            ..Default::default()
                        })
                        .exec(&ctx_clone.db)
                        .await
                        .map_err(|e| error!("failed update timer {}", e.to_string()));
                    }
                    Err(e) => error!(
                        "could not get next tick for workflow:{}, {}",
                        workflow_id,
                        e.to_string()
                    ),
                    _ => (),
                };
            })
        };

        let job = match timer_expr.timezone.as_str() {
            "utc" => {
                let job =
                    Job::new_async_tz(&timer_expr.expr, Utc, move |uuid, l: JobScheduler| {
                        handler(uuid, l)
                    })?;
                job
            }
            "local" => {
                let job =
                    Job::new_async_tz(&timer_expr.expr, Local, move |uuid, l: JobScheduler| {
                        handler(uuid, l)
                    })?;
                job
            }
            _ => anyhow::bail!("not support timezone"),
        };

        WorkflowTimer::update(workflow_timer::ActiveModel {
            id: Set(timer.id),
            schedule_guid: Set(job.guid().to_string()),
            is_active: Set(true),
            startup_error: Set("".to_string()),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;
        sched.add(job).await?;

        Ok(())
    }

    pub async fn start_timer(&self, timer_id: u64, sched: JobScheduler) -> Result<()> {
        let timer_record = WorkflowTimer::find()
            .filter(workflow_timer::Column::Id.eq(timer_id))
            .filter(workflow_timer::Column::IsDeleted.eq(false))
            .one(&self.ctx.db)
            .await?;

        let Some(timer_record) = timer_record else {
            return Ok(());
        };

        WorkflowTimer::update(workflow_timer::ActiveModel {
            id: Set(timer_id),
            is_active: Set(true),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;

        self.add_job_to_scheduler(timer_record, sched).await
    }

    pub async fn stop_timer(&self, timer_id: u64, sched: JobScheduler) -> Result<()> {
        let timer_record = WorkflowTimer::find()
            .filter(workflow_timer::Column::Id.eq(timer_id))
            .filter(workflow_timer::Column::IsDeleted.eq(false))
            .one(&self.ctx.db)
            .await?;

        let Some(timer_record) = timer_record else {
            return Ok(());
        };

        WorkflowTimer::update(workflow_timer::ActiveModel {
            id: Set(timer_id),
            is_active: Set(false),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;

        sched
            .remove(&Uuid::from_str(&timer_record.schedule_guid)?)
            .await?;
        Ok(())
    }

    pub async fn get_timer_list(
        &self,
        team_id: Option<u64>,
        created_user: Option<&String>,
        name: Option<String>,
        workflow_name: Option<String>,
        updated_time_range: Option<(String, String)>,
        tag_ids: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkflowTimerWithTeamModel>, u64)> {
        let mut select = WorkflowTimer::find()
            .column_as(workflow::Column::Name, "workflow_name")
            .column(workflow::Column::TeamId)
            .column_as(team::Column::Name, "team_name")
            .join_rev(
                JoinType::LeftJoin,
                WorkflowVersion::belongs_to(WorkflowTimer)
                    .condition_type(ConditionType::All)
                    .on_condition(|l, r| {
                        Expr::col((l, workflow_version::Column::Id))
                            .equals((r, workflow_timer::Column::VersionId))
                            .into_condition()
                    })
                    .from(workflow_version::Column::WorkflowId)
                    .to(workflow_timer::Column::WorkflowId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Workflow::belongs_to(WorkflowTimer)
                    .from(workflow::Column::Id)
                    .to(workflow_timer::Column::WorkflowId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Workflow)
                    .from(team::Column::Id)
                    .to(workflow::Column::TeamId)
                    .into(),
            )
            .filter(workflow_timer::Column::IsDeleted.eq(false))
            .apply_if(name, |query, v| {
                query.filter(workflow_timer::Column::Name.contains(v))
            })
            .apply_if(created_user, |query, v| {
                query.filter(workflow_timer::Column::CreatedUser.eq(v))
            })
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    workflow_timer::Column::UpdatedTime
                        .gt(v.0)
                        .and(workflow_timer::Column::UpdatedTime.lt(v.1)),
                )
            })
            .apply_if(workflow_name, |q, v| {
                q.filter(workflow::Column::Name.like(v))
            })
            .apply_if(team_id, |q, v| q.filter(workflow::Column::TeamId.eq(v)));

        match tag_ids {
            Some(v) if v.len() > 0 => {
                select = select.filter(
                    Condition::any().add(
                        workflow::Column::Id.in_subquery(
                            Query::select()
                                .column(tag_resource::Column::ResourceId)
                                .and_where(
                                    tag_resource::Column::ResourceType
                                        .eq(ResourceType::Workflow.to_string())
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
            .order_by_desc(workflow_timer::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((list, total))
    }
}
