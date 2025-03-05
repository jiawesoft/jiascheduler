use super::types::{self, ResourceType, TeamRecord};
use crate::{
    entity::{instance, job, prelude::*, tag, tag_resource},
    state::AppContext,
};
use anyhow::{anyhow, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityOrSelect, EntityTrait, JoinType, QueryFilter, QuerySelect,
    QueryTrait, Related, Set,
};
use sea_query::Expr;

#[derive(Clone)]
pub struct TagLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> TagLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn bind_tag(
        &self,
        user_info: &types::UserInfo,
        tag_name: &str,
        resource_type: ResourceType,
        resource_id: u64,
    ) -> Result<u64> {
        match resource_type {
            ResourceType::Job => {
                let record = Job::find()
                    .filter(job::Column::Id.eq(resource_id))
                    .one(&self.ctx.db)
                    .await?;
                if record.is_none() {
                    anyhow::bail!("cannot found job by id {}", resource_id);
                }
            }
            ResourceType::Instance => {
                let record = Instance::find()
                    .filter(instance::Column::Id.eq(resource_id))
                    .one(&self.ctx.db)
                    .await?;
                if record.is_none() {
                    anyhow::bail!("cannot found instance by id {}", resource_id);
                }
            }
        }

        let tag_record = Tag::find()
            .filter(tag::Column::TagName.eq(tag_name))
            .one(&self.ctx.db)
            .await?;

        let tag_id = if tag_record.is_none() {
            let inserted = tag::ActiveModel {
                tag_name: Set(tag_name.to_string()),
                created_user: Set(user_info.username.clone()),
                ..Default::default()
            }
            .save(&self.ctx.db)
            .await?;

            inserted.id.as_ref().to_owned()
        } else {
            tag_record.unwrap().id
        };

        match resource_type {
            ResourceType::Job => {
                Job::find()
                    .filter(job::Column::Id.eq(resource_id))
                    .one(&self.ctx.db)
                    .await?
                    .ok_or(anyhow!("cannot found job by id {resource_id}"))?;
            }
            ResourceType::Instance => {
                Instance::find()
                    .filter(instance::Column::Id.eq(resource_id))
                    .one(&self.ctx.db)
                    .await?
                    .ok_or(anyhow!("cannot found instance by id {resource_id}"))?;
            }
        };

        tag_resource::ActiveModel {
            tag_id: Set(tag_id),
            resource_type: Set(resource_type.to_string()),
            resource_id: Set(resource_id),
            created_user: Set(user_info.username.clone()),
            ..Default::default()
        }
        .save(&self.ctx.db)
        .await?;

        Ok(tag_id)
    }

    pub async fn unbind_tag(
        &self,
        _user_info: &types::UserInfo,
        tag_id: u64,
        resource_type: ResourceType,
        resource_val: Vec<u64>,
    ) -> Result<u64> {
        let ret = TagResource::delete_many()
            .filter(tag_resource::Column::TagId.eq(tag_id))
            .filter(tag_resource::Column::ResourceType.eq(resource_type.to_string()))
            .filter(tag_resource::Column::ResourceId.is_in(resource_val))
            .exec(&self.ctx.db)
            .await?;
        Ok(ret.rows_affected)
    }

    pub async fn count_team_tag(
        &self,
        user_info: &types::UserInfo,
        resource_type: ResourceType,
        team_id: Option<u64>,
    ) -> Result<Vec<types::TagCount>> {
        let select = TagResource::find()
            .column(tag::Column::Id)
            .column_as(tag::Column::Id.count(), "count")
            .join_rev(
                JoinType::LeftJoin,
                TagResource::belongs_to(Tag)
                    .from(tag_resource::Column::TagId)
                    .to(tag::Column::Id)
                    .into(),
            )
            .filter(tag_resource::Column::ResourceType.eq(resource_type.to_string()))
            .apply_if(
                team_id.map_or(Some(user_info.username.clone()), |_| None),
                |q, v| q.filter(tag_resource::Column::CreatedUser.eq(v)),
            );

        let select = match resource_type {
            ResourceType::Job => select
                .join_rev(
                    JoinType::LeftJoin,
                    Job::belongs_to(TagResource)
                        .from(job::Column::Id)
                        .to(tag_resource::Column::ResourceId)
                        .into(),
                )
                .apply_if(team_id, |q, v| q.filter(job::Column::TeamId.eq(v))),
            ResourceType::Instance => select.join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(TagResource)
                    .from(instance::Column::Id)
                    .to(tag_resource::Column::ResourceId)
                    .into(),
            ),
        };

        let tag_count: Vec<types::TagCount> = select
            .group_by(tag::Column::Id)
            .into_model()
            .all(&self.ctx.db)
            .await?;

        Ok(tag_count)
    }
}
