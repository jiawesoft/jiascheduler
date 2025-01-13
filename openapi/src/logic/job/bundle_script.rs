use crate::entity::executor;
use crate::entity::job_bundle_script;
use crate::entity::prelude::*;
use anyhow::Result;
use sea_orm::JoinType;
use sea_orm::QuerySelect;
use sea_orm::QueryTrait;
use sea_orm::Set;
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
            .join_rev(
                JoinType::LeftJoin,
                Executor::belongs_to(JobBundleScript)
                    .from(executor::Column::Id)
                    .to(job_bundle_script::Column::ExecutorId)
                    .into(),
            )
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

    pub async fn delete_bundle_script(&self, username: String, eid: String) -> Result<u64> {
        let ret = JobBundleScript::delete(job_bundle_script::ActiveModel {
            eid: Set(eid),
            created_user: Set(username),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;
        Ok(ret.rows_affected)
    }
}
