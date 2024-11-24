use std::sync::LazyLock;

use anyhow::{anyhow, Result};
use futures::Future;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QueryTrait, Set,
};
use sea_query::Expr;

use super::types::Permission;
use crate::{
    entity::{instance, instance_role, prelude::*, role, user},
    state::AppContext,
};

const POLICY_ALLOW_MANAGE_ALL_USER: Permission = Permission {
    name: "Allow manage all user",
    object: "user",
    action: "manage",
};

const POLICY_ALLOW_MANAGE_ALL_INSTANCE: Permission = Permission {
    name: "Allow manage all instance",
    object: "instance",
    action: "manage",
};

const POLICY_DO_NOT_ALLOW_CHANGE_DATA: Permission = Permission {
    name: "Don't allow changes data ",
    object: "change",
    action: "forbid",
};

pub static PERMISSIONS: LazyLock<Vec<Permission>> = LazyLock::new(|| {
    // vec![
    //     Permission {
    //         name: "Allow manage all user",
    //         object: "user",
    //         action: "manage",
    //     },
    //     Permission {
    //         name: "Allow manage all instance",
    //         object: "instance",
    //         action: "manage",
    //     },
    //     Permission {
    //         name: "Don't allow changes data ",
    //         object: "change",
    //         action: "forbid",
    //     },
    // ]
    vec![
        POLICY_ALLOW_MANAGE_ALL_INSTANCE,
        POLICY_ALLOW_MANAGE_ALL_USER,
        POLICY_DO_NOT_ALLOW_CHANGE_DATA,
    ]
});

#[derive(Clone)]
pub struct RoleLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> RoleLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn save_role(
        &self,
        active_model: role::ActiveModel,
        user_ids: Option<Vec<String>>,
    ) -> Result<u64> {
        let active_model = active_model.save(&self.ctx.db).await?;
        if let Some(user_ids) = user_ids {
            let role_ids = Role::find()
                .filter(role::Column::Name.eq("admin"))
                .all(&self.ctx.db)
                .await?
                .iter()
                .map(|v| v.id)
                .collect::<Vec<_>>();

            User::update_many()
                .set(user::ActiveModel {
                    role_id: Set(active_model.id.as_ref().to_owned()),
                    ..Default::default()
                })
                .filter(
                    user::Column::UserId
                        .is_in(user_ids)
                        .and(user::Column::RoleId.is_not_in(role_ids)),
                )
                .exec(&self.ctx.db)
                .await?;
        }
        Ok(active_model.id.as_ref().to_owned())
    }

    pub async fn set_user<T, F>(
        &self,
        role_id: u64,
        user_ids: Option<Vec<String>>,
        update_role: F,
    ) -> Result<u64>
    where
        F: FnOnce(String, String) -> T + Clone,
        T: Future<Output = Result<()>>,
    {
        let role_record = Role::find_by_id(role_id)
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("invalid role, role_id: {role_id}"))?;

        let affected = if let Some(user_ids) = user_ids {
            let affected = User::update_many()
                .set(user::ActiveModel {
                    role_id: Set(role_id),
                    ..Default::default()
                })
                .filter(user::Column::UserId.is_in(user_ids.clone()))
                .filter(user::Column::IsRoot.eq(false))
                .exec(&self.ctx.db)
                .await?
                .rows_affected;

            let user_records = User::find()
                .filter(user::Column::UserId.is_in(user_ids))
                .filter(user::Column::IsRoot.eq(false))
                .all(&self.ctx.db)
                .await?;

            for v in user_records {
                let update_role = update_role.clone();
                update_role(v.user_id, role_record.id.to_string()).await?;
            }

            affected
        } else {
            0
        };
        Ok(affected)
    }

    pub async fn query_role(
        &self,
        name: Option<String>,
        created_user: Option<String>,
        id: Option<u64>,
        default_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<role::Model>, u64)> {
        let model = Role::find()
            .apply_if(name, |query, v| {
                query.filter(role::Column::Name.contains(v))
            })
            .apply_if(created_user, |query, v| {
                query.filter(role::Column::CreatedUser.contains(v))
            })
            .apply_if(id, |query, v| query.filter(role::Column::Id.eq(v)));

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(role::Column::Id.eq(v)))
            })
            .order_by_desc(role::Column::UpdatedTime)
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;

        Ok((list, total))
    }

    pub async fn delete_role(&self, id: u64) -> Result<u64> {
        let record = User::find()
            .filter(user::Column::RoleId.eq(id))
            .one(&self.ctx.db)
            .await?;
        if record.is_some() {
            anyhow::bail!("forbidden to delete the role in use")
        }

        let ret = Role::delete(role::ActiveModel {
            id: Set(id),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;
        Ok(ret.rows_affected)
    }

    pub async fn bind_instance(
        &self,
        role_id: u64,
        instance_group_ids: Option<Vec<u64>>,
        instance_ids: Option<Vec<String>>,
    ) -> Result<u64> {
        if Role::find_by_id(role_id).one(&self.ctx.db).await?.is_none() {
            anyhow::bail!("invalid role");
        }

        if let Some(instance_ids) = instance_ids {
            let instance_list = Instance::find()
                .filter(instance::Column::InstanceId.is_in(instance_ids))
                .all(&self.ctx.db)
                .await?;
            let data = instance_list
                .into_iter()
                .map(|v| instance_role::ActiveModel {
                    role_id: Set(role_id),
                    instance_id: Set(v.instance_id),
                    ..Default::default()
                })
                .collect::<Vec<instance_role::ActiveModel>>();

            return Ok(InstanceRole::insert_many(data)
                .exec(&self.ctx.db)
                .await?
                .last_insert_id);
        }

        if let Some(instance_group_ids) = instance_group_ids {
            let data = instance_group_ids
                .into_iter()
                .map(|v| instance_role::ActiveModel {
                    role_id: Set(role_id),
                    instance_group_id: Set(v),
                    ..Default::default()
                })
                .collect::<Vec<instance_role::ActiveModel>>();

            Ok(InstanceRole::insert_many(data)
                .exec(&self.ctx.db)
                .await?
                .last_insert_id)
        } else {
            Ok(0)
        }
    }

    pub async fn unbind_instance(
        &self,
        role_id: u64,
        instance_group_ids: Option<Vec<u64>>,
        instance_ids: Option<Vec<String>>,
    ) -> Result<u64> {
        Ok(InstanceRole::delete_many()
            .filter(instance_role::Column::RoleId.eq(role_id))
            .apply_if(instance_ids, |query, v| {
                query.filter(
                    Condition::all()
                        .add(instance_role::Column::InstanceId.is_in(v))
                        .add(instance_role::Column::InstanceGroupId.eq(0)),
                )
            })
            .apply_if(instance_group_ids, |query, v| {
                query.filter(
                    Condition::all()
                        .add(instance_role::Column::InstanceGroupId.is_in(v))
                        .add(instance_role::Column::InstanceId.eq("")),
                )
            })
            .exec(&self.ctx.db)
            .await?
            .rows_affected)
    }

    pub async fn is_admin(&self, role_id: u64) -> Result<bool> {
        Ok(Role::find_by_id(role_id)
            .one(&self.ctx.db)
            .await?
            .map(|v| v.is_admin)
            .unwrap_or(false))
    }
}
