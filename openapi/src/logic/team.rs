use anyhow::{Ok, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set,
};
use sea_query::Expr;

use crate::{
    entity::{job, prelude::*, role, team, team_member, user},
    state::AppContext,
};

use super::types;

#[derive(Clone)]
pub struct TeamLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> TeamLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn get_team_member(&self, team_id: u64) -> Result<Vec<team_member::Model>> {
        Ok(TeamMember::find()
            .filter(team_member::Column::TeamId.eq(team_id))
            .all(&self.ctx.db)
            .await?)
    }

    pub async fn can_write_job(&self, team_id: Option<u64>, user_id: String) -> Result<bool> {
        let ret = self.ctx.can_manage_job(&user_id).await?;
        if ret {
            return Ok(true);
        }

        let team_id = if let Some(team_id) = team_id {
            team_id
        } else {
            return Ok(false);
        };

        if let Some(_) = TeamMember::find()
            .filter(team_member::Column::TeamId.eq(team_id))
            .filter(team_member::Column::UserId.eq(user_id))
            .one(&self.ctx.db)
            .await?
        {
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn can_write_team(&self, team_id: Option<u64>, user_id: String) -> Result<bool> {
        let team_id = if let Some(team_id) = team_id {
            team_id
        } else {
            return Ok(true);
        };
        let ret = self.ctx.can_manage_job(&user_id).await?;
        if ret {
            return Ok(true);
        }
        if let Some(member) = TeamMember::find()
            .filter(team_member::Column::TeamId.eq(team_id))
            .filter(team_member::Column::UserId.eq(user_id))
            .one(&self.ctx.db)
            .await?
        {
            return Ok(member.is_admin);
        }
        Ok(false)
    }

    pub async fn get_team(
        &self,
        team_id: u64,
    ) -> Result<Option<(team::Model, Vec<team_member::Model>)>> {
        let team_record = Team::find_by_id(team_id).one(&self.ctx.db).await?;
        if let Some(team_record) = team_record {
            let team_member = TeamMember::find()
                .filter(team_member::Column::TeamId.eq(team_id))
                .all(&self.ctx.db)
                .await?;

            return Ok(Some((team_record, team_member)));
        }
        Ok(None)
    }

    pub async fn save_team(
        &self,
        active_model: team::ActiveModel,
        user_ids: Option<Vec<String>>,
    ) -> Result<u64> {
        let active_model = active_model.save(&self.ctx.db).await?;
        let team_id = active_model.id.as_ref().to_owned();
        if let Some(mut user_ids) = user_ids {
            TeamMember::delete_many()
                .filter(team_member::Column::UserId.is_not_in(user_ids.clone()))
                .filter(team_member::Column::TeamId.eq(team_id))
                .exec(&self.ctx.db)
                .await?;

            let exists = TeamMember::find()
                .filter(team_member::Column::TeamId.eq(team_id))
                .all(&self.ctx.db)
                .await?
                .into_iter()
                .map(|v| v.user_id)
                .collect::<Vec<String>>();

            user_ids.retain(|v| !exists.contains(v));
            if user_ids.len() == 0 {
                return Ok(team_id);
            }

            let models: Vec<team_member::ActiveModel> = User::find()
                .filter(user::Column::UserId.is_in(user_ids))
                .all(&self.ctx.db)
                .await?
                .into_iter()
                .map(|v| team_member::ActiveModel {
                    team_id: Set(team_id),
                    user_id: Set(v.user_id),
                    ..Default::default()
                })
                .collect();

            TeamMember::insert_many(models).exec(&self.ctx.db).await?;
        }
        Ok(team_id)
    }

    pub async fn query_team(
        &self,
        name: Option<String>,
        created_user: Option<String>,
        id: Option<u64>,
        default_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<team::Model>, u64)> {
        let model = Team::find()
            .apply_if(name, |query, v| {
                query.filter(team::Column::Name.contains(v))
            })
            .apply_if(created_user, |query, v| {
                query.filter(team::Column::CreatedUser.contains(v))
            })
            .apply_if(id, |query, v| query.filter(role::Column::Id.eq(v)));

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(team::Column::Id.eq(v)))
            })
            .order_by_desc(team::Column::UpdatedTime)
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((list, total))
    }

    pub async fn delete_team(&self, id: u64) -> Result<u64> {
        let record = Job::find()
            .filter(job::Column::TeamId.eq(id))
            .one(&self.ctx.db)
            .await?;
        if record.is_some() {
            anyhow::bail!("forbidden to delete the team in use")
        }

        let ret = Team::delete_by_id(id).exec(&self.ctx.db).await?;

        TeamMember::delete_many()
            .filter(team_member::Column::TeamId.eq(id))
            .exec(&self.ctx.db)
            .await?;

        Ok(ret.rows_affected)
    }

    pub async fn append_member(&self, team_id: u64, user_ids: Option<Vec<String>>) -> Result<u64> {
        if Team::find_by_id(team_id).one(&self.ctx.db).await?.is_none() {
            anyhow::bail!("invalid team");
        }

        if let Some(user_ids) = user_ids {
            let user_list = User::find()
                .filter(user::Column::UserId.is_in(user_ids))
                .all(&self.ctx.db)
                .await?;
            let data = user_list
                .into_iter()
                .map(|v| team_member::ActiveModel {
                    team_id: Set(team_id),
                    user_id: Set(v.user_id),
                    ..Default::default()
                })
                .collect::<Vec<team_member::ActiveModel>>();

            return Ok(TeamMember::insert_many(data)
                .exec(&self.ctx.db)
                .await?
                .last_insert_id);
        }
        Ok(0)
    }

    pub async fn remove_member(&self, team_id: u64, user_ids: Option<Vec<String>>) -> Result<u64> {
        Ok(TeamMember::delete_many()
            .filter(team_member::Column::TeamId.eq(team_id))
            .apply_if(user_ids, |query, v| {
                query.filter(team_member::Column::UserId.is_in(v))
            })
            .exec(&self.ctx.db)
            .await?
            .rows_affected)
    }

    pub async fn count_team_member(&self) -> Result<types::TeamMemberCountList> {
        let list: Vec<types::TeamMemberCount> = TeamMember::find()
            .select_only()
            .column_as(team_member::Column::UserId.count(), "total")
            .column(team_member::Column::TeamId)
            .group_by(team_member::Column::TeamId)
            .into_model()
            .all(&self.ctx.db)
            .await?;
        Ok(types::TeamMemberCountList(list))
    }
}
