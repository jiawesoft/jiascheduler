use anyhow::{Ok, Result};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{self, NotSet},
    ColumnTrait, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, Set,
};
use sea_query::Expr;

use crate::{
    entity::{job, prelude::*, team, team_member, user},
    state::AppContext,
};

use super::{
    job::types::TeamMemberModel,
    types::{self, TeamRecord, UserInfo},
};

#[derive(Clone)]
pub struct TeamLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> TeamLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn get_team_member(&self, team_id: u64) -> Result<Vec<TeamMemberModel>> {
        Ok(TeamMember::find()
            .column(user::Column::Username)
            .filter(team_member::Column::TeamId.eq(team_id))
            .join_rev(
                JoinType::LeftJoin,
                User::belongs_to(TeamMember)
                    .from(user::Column::UserId)
                    .to(team_member::Column::UserId)
                    .into(),
            )
            .into_model()
            .all(&self.ctx.db)
            .await?)
    }

    pub async fn can_write_team(&self, team_id: Option<u64>, user_id: String) -> Result<bool> {
        let Some(team_id) = team_id else {
            return Ok(true);
        };
        if self.ctx.can_manage_job(&user_id).await? {
            return Ok(true);
        }
        if Team::find()
            .join_rev(
                JoinType::LeftJoin,
                User::belongs_to(Team)
                    .from(user::Column::Username)
                    .to(team::Column::CreatedUser)
                    .into(),
            )
            .filter(team::Column::Id.eq(team_id))
            .filter(user::Column::UserId.eq(&user_id))
            .one(&self.ctx.db)
            .await?
            .is_some()
        {
            return Ok(true);
        }

        if let Some(member) = TeamMember::find()
            .filter(team_member::Column::TeamId.eq(team_id))
            .filter(team_member::Column::UserId.eq(&user_id))
            .one(&self.ctx.db)
            .await?
        {
            return Ok(member.is_admin);
        }
        Ok(false)
    }

    pub async fn can_read_team(&self, team_id: Option<u64>, user_id: String) -> Result<bool> {
        let Some(team_id) = team_id else {
            return Ok(true);
        };
        if self.ctx.can_manage_job(&user_id).await? {
            return Ok(true);
        }

        if Team::find()
            .join_rev(
                JoinType::LeftJoin,
                User::belongs_to(Team)
                    .from(user::Column::Username)
                    .to(team::Column::CreatedUser)
                    .into(),
            )
            .filter(team::Column::Id.eq(team_id))
            .filter(user::Column::UserId.eq(&user_id))
            .one(&self.ctx.db)
            .await?
            .is_some()
        {
            return Ok(true);
        }

        if TeamMember::find()
            .filter(team_member::Column::TeamId.eq(team_id))
            .filter(team_member::Column::UserId.eq(&user_id))
            .one(&self.ctx.db)
            .await?
            .is_some()
        {
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn get_team(&self, team_id: u64) -> Result<Option<team::Model>> {
        Ok(Team::find_by_id(team_id).one(&self.ctx.db).await?)
    }

    pub async fn save_team(&self, active_model: team::ActiveModel) -> Result<u64> {
        let active_model = active_model.save(&self.ctx.db).await?;
        let team_id = active_model.id.as_ref().to_owned();
        let admin_username = active_model.created_user.as_ref().to_string();

        let Some(admin_user) = User::find()
            .filter(user::Column::Username.eq(&admin_username))
            .one(&self.ctx.db)
            .await?
        else {
            anyhow::bail!("cannot found {admin_username}");
        };

        let m = TeamMember::find()
            .filter(
                team_member::Column::UserId
                    .eq(&admin_user.user_id)
                    .and(team_member::Column::TeamId.eq(team_id)),
            )
            .one(&self.ctx.db)
            .await?;
        team_member::ActiveModel {
            id: m.map_or(NotSet, |v| Set(v.id)),
            team_id: Set(team_id),
            user_id: Set(admin_user.user_id.clone()),
            is_admin: Set(true),
            ..Default::default()
        }
        .save(&self.ctx.db)
        .await?;
        Ok(team_id)
    }

    pub async fn get_my_teams(&self, username: &str) -> Result<Vec<TeamRecord>> {
        let list = Team::find()
            .column(team_member::Column::IsAdmin)
            .join_rev(
                JoinType::LeftJoin,
                TeamMember::belongs_to(Team)
                    .from(team_member::Column::TeamId)
                    .to(team::Column::Id)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                User::belongs_to(TeamMember)
                    .from(user::Column::UserId)
                    .to(team_member::Column::UserId)
                    .into(),
            )
            .filter(user::Column::Username.eq(username))
            .into_model()
            .all(&self.ctx.db)
            .await?;
        Ok(list)
    }

    pub async fn query_team(
        &self,
        name: Option<String>,
        created_user: Option<String>,
        id: Option<u64>,
        default_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<TeamRecord>, u64)> {
        let mut model = Team::find();

        if created_user.is_some() {
            model = model
                .column(team_member::Column::IsAdmin)
                .join_rev(
                    JoinType::LeftJoin,
                    TeamMember::belongs_to(Team)
                        .from(team_member::Column::TeamId)
                        .to(team::Column::Id)
                        .into(),
                )
                .join_rev(
                    JoinType::LeftJoin,
                    User::belongs_to(TeamMember)
                        .from(user::Column::UserId)
                        .to(team_member::Column::UserId)
                        .into(),
                )
                .apply_if(created_user, |query, v| {
                    query.filter(user::Column::Username.eq(&v))
                });
        }

        let model = model
            .apply_if(name, |query, v| {
                query.filter(team::Column::Name.contains(v))
            })
            .apply_if(id, |query, v| query.filter(team::Column::Id.eq(v)));

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(team::Column::Id.eq(v)))
            })
            .order_by_desc(team::Column::UpdatedTime)
            .into_model()
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

    pub async fn add_member(
        &self,
        team_id: u64,
        members: Vec<team_member::ActiveModel>,
    ) -> Result<u64> {
        if Team::find_by_id(team_id).one(&self.ctx.db).await?.is_none() {
            anyhow::bail!("invalid team");
        }

        let user_ids = members
            .iter()
            .filter(|v| v.user_id.is_set())
            .map(|v| v.user_id.clone().unwrap())
            .collect::<Vec<String>>();

        let user_list = User::find()
            .filter(user::Column::UserId.is_in(user_ids))
            .all(&self.ctx.db)
            .await?;

        let members = members
            .into_iter()
            .filter(|v| {
                let ActiveValue::Set(user_id) = v.user_id.clone() else {
                    return false;
                };
                user_list.iter().any(|v| v.user_id == user_id)
            })
            .collect::<Vec<team_member::ActiveModel>>();

        return Ok(TeamMember::insert_many(members)
            .exec(&self.ctx.db)
            .await?
            .last_insert_id);
    }

    pub async fn remove_member(&self, team_id: u64, user_ids: Option<Vec<String>>) -> Result<u64> {
        let Some(user_ids) = user_ids else {
            anyhow::bail!("empty users");
        };

        let user_id = Team::find_by_id(team_id)
            .select_only()
            .column(user::Column::UserId)
            .join_rev(
                JoinType::LeftJoin,
                User::belongs_to(Team)
                    .from(user::Column::Username)
                    .to(team::Column::CreatedUser)
                    .into(),
            )
            .into_tuple::<String>()
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow::anyhow!("cannot found team creator"))?;

        if user_ids.contains(&user_id) {
            anyhow::bail!("cannot remove team creator");
        }

        Ok(TeamMember::delete_many()
            .filter(team_member::Column::TeamId.eq(team_id))
            .filter(team_member::Column::UserId.is_in(user_ids))
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
