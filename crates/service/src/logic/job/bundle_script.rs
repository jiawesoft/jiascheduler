use crate::entity::executor;
use crate::entity::job;
use crate::entity::job_bundle_script;
use crate::entity::prelude::*;
use crate::entity::team;
use crate::logic::types::UserInfo;
use anyhow::anyhow;
use anyhow::Result;
use chrono::Local;
use sea_orm::ActiveValue::Set;
use sea_orm::Condition;
use sea_orm::JoinType;
use sea_orm::QuerySelect;
use sea_orm::QueryTrait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use sea_query::Expr;

use super::types;
use super::JobLogic;

impl<'a> JobLogic<'a> {
    pub async fn save_job_bundle_script(
        &self,
        active_model: job_bundle_script::ActiveModel,
    ) -> Result<job_bundle_script::ActiveModel> {
        let active_model = active_model.save(&self.ctx.db).await?;
        Ok(active_model)
    }

    pub async fn query_bundle_script(
        &self,
        username: Option<String>,
        team_id: Option<u64>,
        default_eid: Option<String>,
        name: Option<String>,
        updated_time_range: Option<(String, String)>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::BundleScriptRelatedExecutorModel>, u64)> {
        let model = JobBundleScript::find()
            .column_as(executor::Column::Name, "executor_name")
            .column_as(team::Column::Name, "team_name")
            .join_rev(
                JoinType::LeftJoin,
                Executor::belongs_to(JobBundleScript)
                    .from(executor::Column::Id)
                    .to(job_bundle_script::Column::ExecutorId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(JobBundleScript)
                    .from(team::Column::Id)
                    .to(job_bundle_script::Column::TeamId)
                    .into(),
            )
            .filter(job_bundle_script::Column::IsDeleted.eq(false))
            .apply_if(username, |q, v| {
                q.filter(job_bundle_script::Column::CreatedUser.eq(v))
            })
            .apply_if(name, |query, v| {
                query.filter(job_bundle_script::Column::Name.contains(v))
            })
            .apply_if(updated_time_range, |query, v| {
                query.filter(
                    job_bundle_script::Column::UpdatedTime
                        .gt(v.0)
                        .and(job_bundle_script::Column::UpdatedTime.lt(v.1)),
                )
            })
            .apply_if(team_id, |q, v| {
                q.filter(job_bundle_script::Column::TeamId.eq(v))
            });

        let total = model.clone().count(&self.ctx.db).await?;
        let list = model
            .apply_if(default_eid, |query, v| {
                query.order_by_desc(Expr::expr(job_bundle_script::Column::Eid.eq(v)))
            })
            .order_by_desc(job_bundle_script::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn delete_bundle_script(&self, user_info: &UserInfo, eid: String) -> Result<u64> {
        let cond = Condition::all().add(Expr::cust_with_values(
            "JSON_CONTAINS(bundle_script, ?)",
            vec![serde_json::json!({ "eid": eid.clone() })],
        ));

        let has = Job::find()
            .filter(cond)
            .filter(job::Column::JobType.eq("bundle"))
            .one(&self.ctx.db)
            .await?;
        if has.is_some() {
            anyhow::bail!("this bundle script is used by job");
        }

        let ret = JobBundleScript::update_many()
            .set(job_bundle_script::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(job_bundle_script::Column::Eid.eq(eid))
            .exec(&self.ctx.db)
            .await?;
        Ok(ret.rows_affected)
    }

    pub async fn get_bundle_script_by_eid(
        &self,
        eid: &str,
    ) -> Result<Option<job_bundle_script::Model>> {
        let model = JobBundleScript::find()
            .filter(job_bundle_script::Column::Eid.eq(eid))
            .one(&self.ctx.db)
            .await?;
        Ok(model)
    }

    pub async fn get_default_validate_team_id_by_bundle_script(
        &self,
        user_info: &UserInfo,
        eid: Option<&str>,
        default_team_id: Option<u64>,
    ) -> Result<Option<u64>> {
        let Some(eid) = eid else {
            return Ok(default_team_id);
        };

        let record = self
            .get_bundle_script_by_eid(eid)
            .await?
            .ok_or(anyhow!("not found the bundle script by {eid}"))?;
        let team_id = if record.team_id == 0 {
            return Ok(default_team_id);
        } else {
            Some(record.team_id)
        };
        let ok = self
            .can_write_bundle_script(user_info, team_id, None)
            .await?;

        if ok {
            Ok(team_id)
        } else {
            Ok(None)
        }
    }
}
