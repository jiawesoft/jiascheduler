use crate::logic::types::UserInfo;
use crate::logic::workflow::types;
use crate::{
    entity::{prelude::*, team_member},
    state::AppContext,
};
use anyhow::Result;
use entity::{job, team, workflow};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait,
};
use sea_query::Expr;

use super::types::{EdgeConfig, NodeConfig};

pub struct WorkflowLogic<'a> {
    ctx: &'a AppContext,
}

impl<'a> WorkflowLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn get_workflow_list(
        &self,
        _user_info: &UserInfo,
        created_user: Option<&str>,
        default_id: Option<u64>,
        team_id: Option<u64>,
        name: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::WorkflowModel>, u64)> {
        let select = Workflow::find()
            .column_as(team::Column::Name, "team_name")
            .apply_if(team_id, |q, v| q.filter(workflow::Column::TeamId.eq(v)))
            .apply_if(name, |q, v| q.filter(workflow::Column::Name.contains(v)))
            .apply_if(created_user, |q, v| {
                q.filter(workflow::Column::CreatedUser.eq(v))
            })
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Workflow)
                    .from(team::Column::Id)
                    .to(workflow::Column::TeamId)
                    .into(),
            );

        let total = select.clone().count(&self.ctx.db).await?;
        let ret = select
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(workflow::Column::Id.eq(v)))
            })
            .order_by_desc(workflow::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((ret, total))
    }

    pub async fn get_workflow_version_list(
        &self,
        _user_info: &UserInfo,
        name: Option<String>,
        version_status: Option<String>,
        created_user: Option<String>,
        id: u64,
        default_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<workflow::Model>, u64)> {
        let select = Workflow::find()
            .filter(workflow::Column::Pid.eq(id))
            .apply_if(created_user, |q, v| {
                q.filter(workflow::Column::CreatedUser.eq(v))
            })
            .apply_if(version_status, |q, v| {
                q.filter(workflow::Column::VersionStatus.eq(v))
            })
            .apply_if(name, |q, v| q.filter(workflow::Column::Name.contains(v)));

        let total = select.clone().count(&self.ctx.db).await?;
        let ret = select
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(workflow::Column::Id.eq(v)))
            })
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((ret, total))
    }

    pub async fn can_write_workflow(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        workflow_id: Option<u64>,
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

        let Some(workflow_id) = workflow_id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(workflow_record) = Workflow::find()
            .filter(workflow::Column::Id.eq(workflow_id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if workflow_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(workflow_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(workflow_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub async fn save_workflow(
        &self,
        id: Option<u64>,
        user_info: &UserInfo,
        name: String,
        info: Option<String>,
        team_id: Option<u64>,
    ) -> Result<u64> {
        let active_model = workflow::ActiveModel {
            id: id.map_or(NotSet, |v| Set(v)),
            name: Set(name),
            info: info.map_or(NotSet, |v| Set(v)),
            team_id: team_id.map_or(NotSet, |v| Set(v)),
            created_user: Set(user_info.username.clone()),
            updated_user: Set(user_info.username.clone()),
            ..Default::default()
        };

        if let Some(id) = id {
            let affected = Workflow::update_many()
                .set(active_model)
                .filter(workflow::Column::Id.eq(id))
                .filter(workflow::Column::IsDeleted.eq(false))
                .filter(workflow::Column::Pid.eq(0))
                .exec(&self.ctx.db)
                .await?
                .rows_affected;
            return Ok(affected);
        }

        let active_model = active_model.save(&self.ctx.db).await?;
        Ok(active_model.id.as_ref().to_owned())
    }

    pub async fn save_workflow_version(
        &self,
        pid: Option<u64>,
        workflow_id: Option<u64>,
        user_info: &UserInfo,
        name: String,
        info: Option<String>,
        version: String,
        version_status: String,
        nodes: Option<Vec<NodeConfig>>,
        edges: Option<Vec<EdgeConfig>>,
        team_id: Option<u64>,
    ) -> Result<u64> {
        let mut active_model = workflow::ActiveModel {
            pid: pid.map_or(NotSet, |v| Set(v)),
            name: Set(name),
            info: info.map_or(NotSet, |v| Set(v)),
            team_id: team_id.map_or(NotSet, |v| Set(v)),
            version: Set(version),
            version_status: Set(version_status),
            nodes: Set(nodes.map(|v| serde_json::to_value(v)).transpose()?),
            edges: Set(edges.map(|v| serde_json::to_value(v)).transpose()?),
            created_user: Set(user_info.username.clone()),
            updated_user: Set(user_info.username.clone()),
            ..Default::default()
        };

        if let Some(workflow_id) = workflow_id {
            active_model.pid = NotSet;
            let affected = Workflow::update_many()
                .set(active_model)
                .filter(workflow::Column::Id.eq(workflow_id))
                .filter(workflow::Column::IsDeleted.eq(false))
                .filter(workflow::Column::VersionStatus.ne("released"))
                .exec(&self.ctx.db)
                .await?
                .rows_affected;
            Ok(affected)
        } else {
            let active_model = active_model.save(&self.ctx.db).await?;
            Ok(active_model.id.as_ref().to_owned())
        }
    }
}
