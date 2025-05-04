use crate::{
    entity::{self, role, user},
    state::AppContext,
};
use anyhow::{Result, anyhow};
use crypto::digest::Digest;
use crypto::md5::Md5;

use crate::state::AppState;

use futures::Future;
use nanoid::nanoid;

use entity::prelude::*;
use sea_orm::*;

use super::{omit_empty_active_value, types};

#[derive(Clone)]
pub struct UserLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> UserLogic<'a> {
    pub const SESS_KEY: &'static str = "USER_SESSION";

    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub fn encry_password<S: Into<String>>(password: S, salt: S) -> String {
        let mut md5 = Md5::new();
        md5.input_str(&format!("{}{}", password.into(), salt.into()));
        md5.result_str()
    }

    pub async fn get_user(
        &self,
        username: Option<&str>,
        user_id: Option<&str>,
    ) -> Result<Option<types::UserRecord>> {
        let ret: Option<types::UserRecord> = user::Entity::find()
            .column_as(role::Column::Name, "role")
            .join_rev(
                JoinType::LeftJoin,
                Role::belongs_to(User)
                    .from(role::Column::Id)
                    .to(user::Column::RoleId)
                    .into(),
            )
            .apply_if(username, |query, v| {
                query.filter(user::Column::Username.eq(v))
            })
            .apply_if(user_id, |query, v| query.filter(user::Column::UserId.eq(v)))
            .into_model()
            .one(&self.ctx.db)
            .await?;
        Ok(ret)
    }

    pub async fn valid_user(&self, username: &str, password: &str) -> Result<types::UserRecord> {
        let got_user = self
            .get_user(Some(username), None)
            .await?
            .ok_or(anyhow!("invalid username"))?;

        let password = Self::encry_password(password, &got_user.salt);

        if got_user.password != password {
            Err(anyhow!("invalid username or password"))
        } else {
            Ok(got_user)
        }
    }

    pub async fn save(db: &DbConn, user: user::Model) -> Result<user::ActiveModel, DbErr> {
        user::ActiveModel {
            username: Set(user.username.to_owned()),
            nickname: Set(user.nickname.to_owned()),
            ..Default::default()
        }
        .save(db)
        .await
    }

    pub async fn create_user(&self, user: user::Model) -> Result<u64> {
        let salt = nanoid!();
        let user_id = nanoid!(10);

        let nickname = if user.nickname != "" {
            user.nickname
        } else {
            user.username.clone()
        };

        let active_model = user::ActiveModel {
            username: Set(user.username),
            nickname: Set(nickname),
            user_id: Set(user_id),
            password: Set(Self::encry_password(&user.password, &salt)),
            email: Set(user.email),
            gender: Set(user.gender),
            role_id: omit_empty_active_value(user.role_id),
            phone: omit_empty_active_value(user.phone),
            salt: Set(salt),
            introduction: omit_empty_active_value(user.introduction),
            ..Default::default()
        };

        Ok(User::insert(active_model)
            .exec(&self.ctx.db)
            .await?
            .last_insert_id)
    }

    pub async fn update_user<F, T>(
        &self,
        mut record: user::ActiveModel,
        update_role_policy: F,
    ) -> Result<u64>
    where
        F: FnOnce(String, String) -> T,
        T: Future<Output = Result<()>>,
    {
        if let ActiveValue::Set(password) = record.password {
            let salt = nanoid!();
            record.password = Set(Self::encry_password(&password, &salt));
            record.salt = Set(salt)
        }
        let user_id = if let ActiveValue::Set(ref v) = record.user_id {
            v.to_owned()
        } else {
            anyhow::bail!("not set user_id");
        };

        let user_record = self
            .get_user(None, Some(&user_id))
            .await?
            .ok_or(anyhow!("invalid user_id"))?;

        if let ActiveValue::Set(role_id) = record.role_id {
            if user_record.is_root && role_id != user_record.role_id {
                anyhow::bail!("root user cannot modify")
            }
            if role_id != 0 && role_id != user_record.role_id {
                let role_record = Role::find_by_id(role_id)
                    .one(&self.ctx.db)
                    .await?
                    .ok_or(anyhow!("invalid role_Id"))?;

                update_role_policy(user_record.user_id, role_record.id.to_string()).await?;
            } else if role_id == 0 {
                update_role_policy(user_record.user_id, "0".to_string()).await?;
            }
        }

        let update_result = User::update_many()
            .set(record)
            .filter(user::Column::UserId.eq(user_id))
            .exec(&self.ctx.db)
            .await?;

        Ok(update_result.rows_affected)
    }

    pub async fn set_role<T: Into<String>>(&self, user_id: T, role_id: u64) -> Result<u64> {
        let update_result = User::update_many()
            .set(user::ActiveModel {
                role_id: Set(role_id),
                ..Default::default()
            })
            .filter(user::Column::UserId.eq(user_id.into()))
            .exec(&self.ctx.db)
            .await?;

        Ok(update_result.rows_affected)
    }

    pub async fn load_user_role(&self, state: &AppState) -> Result<()> {
        let user_records = User::find().all(&self.ctx.db).await?;
        for v in user_records {
            state
                .set_role_for_user(&v.user_id, v.role_id.to_string().as_str())
                .await?;
        }
        Ok(())
    }

    async fn init_admin_user(db: &DbConn, username: &str, password: &str) -> Result<()> {
        let salt = nanoid!();
        let record = User::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?;

        let mut user_active_model = user::ActiveModel {
            username: Set(username.to_string()),
            nickname: Set(username.to_string()),
            password: Set(Self::encry_password(password.to_string(), salt.clone())),
            gender: Set("male".to_string()),
            is_root: Set(true),
            salt: Set(salt),
            role_id: Set(1),
            ..Default::default()
        };

        match record {
            Some(ref v) => {
                if !v.is_root {
                    anyhow::bail!("Do not change existing regular users to administrators.")
                }
                user_active_model.id = Set(v.id);
            }
            None => {
                user_active_model.user_id = Set(nanoid!(10));
            }
        };
        user_active_model.save(db).await?;

        Ok(())
    }

    async fn init_admin_role(db: &DbConn, username: &str) -> Result<()> {
        let role_record = Role::find_by_id(1u32).one(db).await?;

        if role_record.is_none() {
            role::ActiveModel {
                id: Set(1),
                name: Set("admin".to_string()),
                info: Set("System initialization administrator role, unable to delete".to_string()),
                is_admin: Set(true),
                created_user: Set(username.to_string()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        } else {
            role::ActiveModel {
                id: Set(1),
                name: Set("admin".to_string()),
                is_admin: Set(true),
                created_user: Set(username.to_string()),
                ..Default::default()
            }
            .save(db)
            .await?;
        }
        Ok(())
    }

    pub async fn init_admin(db: &DbConn, username: &str, password: &str) -> Result<()> {
        Self::init_admin_role(db, username).await?;
        Self::init_admin_user(db, username, password).await?;
        Ok(())
    }

    pub async fn count_by_role(&self) -> Result<types::UserRoleCountList> {
        let list: Vec<types::UserRoleCount> = User::find()
            .select_only()
            .column_as(user::Column::Id.count(), "total")
            .column(user::Column::RoleId)
            .group_by(user::Column::RoleId)
            .into_model()
            .all(&self.ctx.db)
            .await?;
        Ok(types::UserRoleCountList(list))
    }

    pub async fn query_user(
        &self,
        user_id: Option<Vec<String>>,
        username: Option<String>,
        nickname: Option<String>,
        phone: Option<String>,
        role_id: Option<u64>,
        ignore_role_id: Option<u64>,
        keyword: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::UserRecord>, u64)> {
        let mut model = User::find().column_as(role::Column::Name, "role").join_rev(
            JoinType::LeftJoin,
            Role::belongs_to(User)
                .from(role::Column::Id)
                .to(user::Column::RoleId)
                .into(),
        );

        model = model
            .apply_if(user_id, |query, v| {
                query.filter(user::Column::UserId.is_in(v))
            })
            .apply_if(nickname, |query, v| {
                query.filter(user::Column::Nickname.contains(v))
            })
            .apply_if(username, |query, v| {
                query.filter(user::Column::Username.contains(v))
            })
            .apply_if(phone, |query, v| {
                query.filter(user::Column::Phone.contains(v))
            })
            .apply_if(role_id, |query, v| query.filter(user::Column::RoleId.eq(v)))
            .apply_if(ignore_role_id, |query, v| {
                query.filter(user::Column::RoleId.ne(v))
            })
            .apply_if(keyword, |query, v| {
                query.filter(
                    Condition::any()
                        .add(user::Column::Nickname.contains(&v))
                        .add(user::Column::Username.contains(&v))
                        .add(user::Column::Phone.contains(&v))
                        .add(user::Column::Email.contains(&v)),
                )
            });

        let total = model.clone().count(&self.ctx.db).await?;
        let list = model
            .order_by_asc(user::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }
}
