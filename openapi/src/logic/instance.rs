use automate::scheduler::types::SshConnectionOption;
use chrono::Local;

use sea_orm::ActiveValue::NotSet;
use sea_orm::Condition;
use sea_orm::DbBackend;
use sea_orm::FromQueryResult;
use sea_orm::Order;
use sea_orm::Statement;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set,
};

use sea_query::MysqlQueryBuilder;
use sea_query::UnionType;
use sea_query::{ConditionType, Expr, IntoCondition, OnConflict};

use crate::entity::instance_role;
use crate::entity::tag;
use crate::entity::tag_resource;
use crate::entity::user;
use crate::entity::{self, instance, instance_group, prelude::*, user_server};
use crate::state::AppContext;
use crate::state::AppState;
use crate::IdGenerator;
use anyhow::Result;

use super::job::types::InstanceStatSummary;
use super::types;
use super::user::UserLogic;

#[derive(Debug, FromQueryResult)]
struct InstanceStatusCount {
    status: bool,
    total: u64,
}

pub struct InstanceLogic<'a> {
    // db: &'a DbConn,
    ctx: &'a AppContext,
    // redis: Client,
}

impl<'a> InstanceLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn update_status(
        &mut self,
        namespace: Option<String>,
        agent_ip: String,
        mac_addr: String,
        status: i8,
        assign_user: Option<(String, String)>,
        ssh_connection_option: Option<SshConnectionOption>,
    ) -> Result<()> {
        let (sys_user, password, ssh_port) = match ssh_connection_option {
            Some(opt) => (
                Set(opt.user),
                Set(self.ctx.encrypt(opt.password)?),
                Set(opt.port),
            ),
            None => (NotSet, NotSet, NotSet),
        };

        if status == 1 && namespace.is_some() {
            let ins_vec = Instance::find()
                .apply_if(namespace.clone(), |q, v| {
                    q.filter(instance::Column::Namespace.eq(v))
                })
                .filter(instance::Column::Ip.eq(&agent_ip))
                .all(&self.ctx.db)
                .await?;
            if ins_vec.len() > 0 {
                Instance::update(instance::ActiveModel {
                    id: Set(ins_vec[0].id),
                    mac_addr: Set(mac_addr.clone()),
                    ..Default::default()
                })
                .exec(&self.ctx.db)
                .await?;
            }
            if ins_vec.len() > 1 {
                let id_vec = ins_vec.iter().map(|v| v.id).collect::<Vec<u64>>();
                if id_vec.len() > 0 {
                    Instance::update_many()
                        .set(instance::ActiveModel {
                            status: Set(0),
                            ..Default::default()
                        })
                        .filter(instance::Column::Id.is_in(id_vec[1..].to_vec()))
                        .exec(&self.ctx.db)
                        .await?;
                }
            }
        }

        let mut updated = if sys_user.is_set() {
            OnConflict::columns([instance::Column::MacAddr, instance::Column::Ip])
                .value(instance::Column::UpdatedTime, Local::now())
                .value(instance::Column::Status, status)
                .value(instance::Column::SysUser, sys_user.clone().unwrap())
                .value(instance::Column::Password, password.clone().unwrap())
                .value(instance::Column::SshPort, ssh_port.clone().unwrap())
                .to_owned()
        } else {
            OnConflict::columns([instance::Column::MacAddr, instance::Column::Ip])
                .value(instance::Column::UpdatedTime, Local::now())
                .value(instance::Column::Status, status)
                .to_owned()
        };

        if let Some(ref namespace) = namespace {
            updated.value(instance::Column::Namespace, namespace.clone());
        }

        let instance_id = IdGenerator::get_instance_uid();

        Instance::insert(instance::ActiveModel {
            ip: Set(agent_ip.clone()),
            namespace: namespace.clone().map_or(NotSet, |v| Set(v)),
            status: Set(status),
            instance_id: Set(instance_id),
            mac_addr: Set(mac_addr),
            sys_user,
            password,
            ssh_port,
            ..Default::default()
        })
        .on_conflict(updated)
        .exec(&self.ctx.db)
        .await?;

        if status == 0 {
            return Ok(());
        }

        if let (Some(assign_user), Some(ins)) = (
            assign_user,
            Instance::find()
                .filter(instance::Column::Ip.eq(agent_ip.clone()))
                .filter(instance::Column::Namespace.eq(namespace.clone()))
                .one(&self.ctx.db)
                .await?,
        ) {
            if let Some(u) = User::find()
                .filter(user::Column::Username.eq(assign_user.0))
                .one(&self.ctx.db)
                .await?
            {
                if UserLogic::encry_password(assign_user.1, u.salt) != u.password {
                    anyhow::bail!("invalid assign user password")
                }

                UserServer::insert(user_server::ActiveModel {
                    user_id: Set(u.user_id),
                    instance_id: Set(ins.instance_id),
                    ..Default::default()
                })
                .on_conflict(
                    OnConflict::columns([
                        user_server::Column::UserId,
                        user_server::Column::InstanceId,
                    ])
                    .do_nothing_on([user_server::Column::UserId, user_server::Column::InstanceId])
                    .to_owned(),
                )
                .exec(&self.ctx.db)
                .await?;
            }
        }

        Ok(())
    }

    pub async fn update_instance(&mut self, mac_addr: String, ip: String) -> Result<u64> {
        let ret = Instance::update_many()
            .set(instance::ActiveModel {
                status: Set(1),
                ..Default::default()
            })
            .filter(instance::Column::MacAddr.eq(mac_addr))
            .filter(instance::Column::Ip.eq(ip))
            .exec(&self.ctx.db)
            .await?;
        Ok(ret.rows_affected)
    }

    pub async fn query_instance_by_role_id(
        &self,
        ip: Option<String>,
        status: Option<u8>,
        role_id: u64,
        ignore_role_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::InstanceRecord>, u64)> {
        let model = InstanceRole::find()
            .select_only()
            .column_as(instance_group::Column::Name, "instance_group")
            .columns([
                instance::Column::Id,
                instance::Column::InstanceId,
                instance::Column::Ip,
                instance::Column::Namespace,
                instance::Column::Info,
                instance::Column::Status,
                instance::Column::SysUser,
                instance::Column::SshPort,
                instance::Column::Password,
                instance::Column::InstanceGroupId,
                instance::Column::CreatedTime,
                instance::Column::UpdatedTime,
            ])
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(InstanceRole)
                    .from(instance::Column::InstanceId)
                    .to(instance_role::Column::InstanceId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .filter(instance_role::Column::RoleId.eq(role_id))
            .filter(instance_role::Column::InstanceGroupId.eq(0))
            .apply_if(ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            })
            .apply_if(status, |query, v| {
                query.filter(instance::Column::Status.eq(v))
            })
            .apply_if(ignore_role_id, |query, v| {
                query.filter(instance_role::Column::RoleId.ne(v))
            });
        let total = model.clone().count(&self.ctx.db).await?;
        let list = model
            .order_by_asc(entity::instance::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn query_instance(
        &self,
        ip: Option<String>,
        status: Option<u8>,
        ignore_role_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::InstanceRecord>, u64)> {
        let model = Instance::find()
            .column_as(instance_group::Column::Name, "instance_group")
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .apply_if(ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            })
            .apply_if(ignore_role_id, |query, v| {
                query.filter(
                    Condition::all().add(
                        instance::Column::Id.not_in_subquery(
                            InstanceRole::find()
                                .select_only()
                                .column(instance_role::Column::InstanceId)
                                .filter(instance_role::Column::RoleId.eq(v))
                                .as_query()
                                .clone(),
                        ),
                    ),
                )
            })
            .apply_if(status, |query, v| {
                query.filter(instance::Column::Status.eq(v))
            });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_asc(entity::instance::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn granted_user(
        &self,
        user_id: Vec<String>,
        instance_ids: Option<Vec<String>>,
        instance_group_ids: Option<Vec<i64>>,
    ) -> Result<u64> {
        let mut models = vec![];

        if let Some(instance_ids) = instance_ids {
            user_id.iter().for_each(|v| {
                for instance_id in &instance_ids {
                    models.push(user_server::ActiveModel {
                        user_id: Set(v.to_owned()),
                        instance_id: Set(instance_id.to_owned()),
                        ..Default::default()
                    });
                }
            })
        }

        if let Some(group_ids) = instance_group_ids {
            user_id.iter().for_each(|v| {
                for group_id in &group_ids {
                    models.push(user_server::ActiveModel {
                        user_id: Set(v.to_owned()),
                        instance_group_id: Set(group_id.to_owned()),
                        ..Default::default()
                    });
                }
            });
        }

        if models.len() == 0 {
            anyhow::bail!("no valid instance granted to the user")
        }

        Ok(UserServer::insert_many(models)
            .exec(&self.ctx.db)
            .await?
            .last_insert_id)
    }

    pub async fn query_server_by_tag(
        &self,
        user_id: Option<String>,
        instance_group_id: Option<u64>,
        status: Option<u8>,
        ip: Option<String>,
        tag_id: Option<Vec<u64>>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::UserServer>, u64)> {
        let model = Tag::find()
            .select_only()
            .column(instance::Column::Id)
            .column(instance::Column::Ip)
            .column(instance::Column::InstanceId)
            .column(instance::Column::Namespace)
            .column(instance::Column::InstanceGroupId)
            .column(instance::Column::Info)
            .column_as(instance_group::Column::Name, "instance_group_name")
            .column(instance::Column::Status)
            .column(instance::Column::CreatedTime)
            .column(instance::Column::UpdatedTime)
            .column(tag::Column::TagKey)
            .column(tag::Column::TagVal)
            .column_as(tag::Column::Id, "tag_id")
            .join_rev(
                JoinType::LeftJoin,
                TagResource::belongs_to(Tag)
                    .from(tag_resource::Column::TagId)
                    .to(tag::Column::Id)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(TagResource)
                    .from(instance::Column::Id)
                    .to(tag_resource::Column::ResourceVal)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .apply_if(ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            })
            .apply_if(instance_group_id, |query, v| {
                query.filter(instance::Column::InstanceGroupId.eq(v))
            })
            .apply_if(status, |query, v| {
                query.filter(instance::Column::Status.eq(v))
            })
            .apply_if(user_id, |query, v| {
                query.filter(tag::Column::CreatedUser.eq(v))
            })
            .filter(instance::Column::Id.gt(0))
            .apply_if(tag_id, |query, v| query.filter(tag::Column::Id.is_in(v)));

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_asc(entity::instance::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn query_admin_server(
        &self,
        instance_id: Option<String>,
        instance_group_id: Option<u64>,
        status: Option<u8>,
        ip: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::UserServer>, u64)> {
        let model = Instance::find()
            .select_only()
            .column(instance::Column::Id)
            .column(instance::Column::InstanceId)
            .column(instance::Column::Ip)
            .column(instance::Column::Namespace)
            .column(instance::Column::Info)
            .column(instance::Column::MacAddr)
            .column(instance::Column::InstanceGroupId)
            .column_as(instance_group::Column::Name, "instance_group_name")
            .column(instance::Column::Status)
            .column(instance::Column::CreatedTime)
            .column(instance::Column::UpdatedTime)
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .apply_if(status, |query, v| {
                query.filter(instance::Column::Status.eq(v))
            })
            .apply_if(instance_id, |query, v| {
                query.filter(instance::Column::InstanceId.eq(v))
            })
            .apply_if(ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            })
            .apply_if(instance_group_id, |query, v| {
                query.filter(instance_group::Column::Id.eq(v))
            });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_desc(entity::instance::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn get_instance_summary(
        &self,
        user_id: Option<String>,
    ) -> Result<InstanceStatSummary> {
        let mut summary = InstanceStatSummary::default();

        if let Some(user_id) = user_id {
            let (sql, vals) = User::find()
                .select_only()
                .column(instance::Column::Status)
                .column_as(instance::Column::Id.count(), "total")
                .join_rev(
                    JoinType::LeftJoin,
                    InstanceRole::belongs_to(User)
                        .from(instance_role::Column::RoleId)
                        .to(user::Column::RoleId)
                        .into(),
                )
                .join_rev(
                    JoinType::LeftJoin,
                    Instance::belongs_to(InstanceRole)
                        .condition_type(ConditionType::Any)
                        .on_condition(|a, b| {
                            Expr::col((b.clone(), instance::Column::InstanceGroupId))
                                .equals((a.clone(), instance_role::Column::InstanceGroupId))
                                .and(Expr::col((b, instance::Column::InstanceGroupId)).gt(0))
                                .into_condition()
                        })
                        .from(instance::Column::Id)
                        .to(instance_role::Column::InstanceId)
                        .into(),
                )
                .filter(user::Column::UserId.eq(user_id.clone()))
                .as_query()
                .to_owned()
                .union(
                    UnionType::Distinct,
                    UserServer::find()
                        .select_only()
                        .column(instance::Column::Status)
                        .column_as(instance::Column::Id.count(), "total")
                        .join_rev(
                            JoinType::LeftJoin,
                            Instance::belongs_to(UserServer)
                                .condition_type(ConditionType::Any)
                                .on_condition(|a, b| {
                                    Expr::col((b.clone(), instance::Column::InstanceGroupId))
                                        .equals((a.clone(), user_server::Column::InstanceGroupId))
                                        .and(
                                            Expr::col((b, instance::Column::InstanceGroupId)).gt(0),
                                        )
                                        .into_condition()
                                })
                                .from(instance::Column::Id)
                                .to(user_server::Column::InstanceId)
                                .into(),
                        )
                        .filter(user_server::Column::UserId.eq(user_id.clone()))
                        .as_query()
                        .clone(),
                )
                .group_by_col(instance::Column::Status)
                .build(MysqlQueryBuilder);

            let list = InstanceStatusCount::find_by_statement(Statement::from_sql_and_values(
                DbBackend::MySql,
                sql,
                vals,
            ))
            .all(&self.ctx.db)
            .await?;

            list.iter().for_each(|v| {
                if v.status {
                    summary.online += v.total;
                } else {
                    summary.offline += v.total;
                }
            });
        } else {
            let list: Vec<(u64, i8)> = Instance::find()
                .select_only()
                .column_as(instance::Column::Id.count(), "total")
                .column(instance::Column::Status)
                .group_by(instance::Column::Status)
                .into_tuple()
                .all(&self.ctx.db)
                .await?;
            list.iter().for_each(|v| {
                if v.1 == 1 {
                    summary.online += v.0;
                } else {
                    summary.offline += v.0;
                }
            });
        };

        Ok(summary)
    }

    pub async fn query_user_server(
        &self,
        user_id: String,
        instance_id: Option<String>,
        instance_group_id: Option<u64>,
        status: Option<u8>,
        ip: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::UserServer>, u64)> {
        let mut model = User::find()
            .select_only()
            .column(instance::Column::Id)
            .column(instance::Column::Ip)
            .column(instance::Column::Namespace)
            .column(instance::Column::Info)
            .column(instance::Column::MacAddr)
            .column(instance::Column::InstanceId)
            .column(instance::Column::InstanceGroupId)
            .column_as(instance_group::Column::Name, "instance_group_name")
            .column(instance::Column::Status)
            .column(instance::Column::CreatedTime)
            .column(instance::Column::UpdatedTime)
            .join_rev(
                JoinType::LeftJoin,
                InstanceRole::belongs_to(User)
                    .from(instance_role::Column::RoleId)
                    .to(user::Column::RoleId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(InstanceRole)
                    .condition_type(ConditionType::Any)
                    .on_condition(|a, b| {
                        Expr::col((b.clone(), instance::Column::InstanceGroupId))
                            .equals((a.clone(), instance_role::Column::InstanceGroupId))
                            .and(Expr::col((b, instance::Column::InstanceGroupId)).gt(0))
                            .into_condition()
                    })
                    .from(instance::Column::InstanceId)
                    .to(instance_role::Column::InstanceId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .filter(user::Column::UserId.eq(user_id.clone()))
            .filter(instance::Column::Id.gt(0))
            .apply_if(status, |query, v| {
                query.filter(instance::Column::Status.eq(v))
            })
            .apply_if(instance_id.clone(), |query, v| {
                query.filter(instance::Column::InstanceId.eq(v))
            })
            .apply_if(instance_group_id, |query, v| {
                query.filter(instance_group::Column::Id.eq(v))
            })
            .as_query()
            .to_owned();

        let model = model.union(
            UnionType::Distinct,
            UserServer::find()
                .select_only()
                .column(instance::Column::Id)
                .column(instance::Column::Ip)
                .column(instance::Column::Namespace)
                .column(instance::Column::Info)
                .column(instance::Column::MacAddr)
                .column(instance::Column::InstanceId)
                .column(instance::Column::InstanceGroupId)
                .column_as(instance_group::Column::Name, "instance_group_name")
                .column(instance::Column::Status)
                .column(instance::Column::CreatedTime)
                .column(instance::Column::UpdatedTime)
                .join_rev(
                    JoinType::LeftJoin,
                    Instance::belongs_to(UserServer)
                        .condition_type(ConditionType::Any)
                        .on_condition(|a, b| {
                            Expr::col((b.clone(), instance::Column::InstanceGroupId))
                                .equals((a.clone(), user_server::Column::InstanceGroupId))
                                .and(Expr::col((b, instance::Column::InstanceGroupId)).gt(0))
                                .into_condition()
                        })
                        .from(instance::Column::InstanceId)
                        .to(user_server::Column::InstanceId)
                        .into(),
                )
                .join_rev(
                    JoinType::LeftJoin,
                    InstanceGroup::belongs_to(Instance)
                        .from(instance_group::Column::Id)
                        .to(instance::Column::InstanceGroupId)
                        .into(),
                )
                .filter(user_server::Column::UserId.eq(user_id.clone()))
                .apply_if(status, |query, v| {
                    query.filter(instance::Column::Status.eq(v))
                })
                .apply_if(instance_id, |query, v| {
                    query.filter(instance::Column::InstanceId.eq(v))
                })
                .as_query()
                .clone(),
        );

        let model = if let Some(ip) = ip {
            model.and_where(instance::Column::Ip.contains(ip))
        } else {
            model
        };

        let model = if let Some(instance_group_id) = instance_group_id {
            model.and_where(instance::Column::InstanceGroupId.eq(instance_group_id))
        } else {
            model
        };

        let (sql, vals) = model
            .clone()
            .order_by(instance::Column::UpdatedTime, Order::Desc)
            .build(MysqlQueryBuilder);

        let total = types::UserServer::find_by_statement(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            &sql,
            vals.clone(),
        ))
        .count(&self.ctx.db)
        .await?;

        let list = types::UserServer::find_by_statement(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            &sql,
            vals,
        ))
        .paginate(&self.ctx.db, page_size)
        .fetch_page(page)
        .await?;

        Ok((list, total))
    }

    pub async fn save_instance(
        &self,
        model: instance::ActiveModel,
    ) -> Result<instance::ActiveModel> {
        let model = model.save(&self.ctx.db).await?;
        Ok(model)
    }

    pub async fn save_group(
        &self,
        model: instance_group::ActiveModel,
    ) -> Result<instance_group::ActiveModel> {
        let model = model.save(&self.ctx.db).await?;
        Ok(model)
    }

    pub async fn query_group(
        &self,
        name: Option<String>,
        ignore_role_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<instance_group::Model>, u64)> {
        let model = InstanceGroup::find()
            .apply_if(name, |query, v| {
                query.filter(instance_group::Column::Name.contains(v))
            })
            .apply_if(ignore_role_id, |query, v| {
                query.filter(
                    Condition::all().add(
                        instance_group::Column::Id.not_in_subquery(
                            InstanceRole::find()
                                .select_only()
                                .column(instance_role::Column::InstanceGroupId)
                                .filter(instance_role::Column::RoleId.eq(v))
                                .as_query()
                                .clone(),
                        ),
                    ),
                )
            });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_desc(instance_group::Column::UpdatedTime)
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn query_group_by_role_id(
        &self,
        name: Option<String>,
        role_id: u64,
        ignore_role_id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<instance_group::Model>, u64)> {
        let model = InstanceRole::find()
            .select_only()
            .columns([
                instance_group::Column::Id,
                instance_group::Column::Name,
                instance_group::Column::Info,
                instance_group::Column::CreatedTime,
                instance_group::Column::UpdatedTime,
                instance_group::Column::CreatedUser,
            ])
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(InstanceRole)
                    .from(instance_group::Column::Id)
                    .to(instance_role::Column::InstanceGroupId)
                    .into(),
            )
            .filter(instance_role::Column::RoleId.eq(role_id))
            .filter(instance_role::Column::InstanceGroupId.gt(0))
            .apply_if(ignore_role_id, |query, v| {
                query.filter(instance_role::Column::RoleId.ne(v))
            })
            .apply_if(name, |query, v| {
                query.filter(instance_group::Column::Name.contains(v))
            });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .order_by_desc(instance_group::Column::UpdatedTime)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn delete_group(&self, id: u64) -> Result<u64> {
        let record = Instance::find()
            .filter(instance::Column::InstanceGroupId.eq(id))
            .one(&self.ctx.db)
            .await?;
        if record.is_some() {
            anyhow::bail!("cannot delete in used group {id}")
        }
        let ret = InstanceGroup::delete_by_id(id).exec(&self.ctx.db).await?;
        Ok(ret.rows_affected)
    }

    pub async fn get_one_admin_server(
        &self,
        namespace: Option<String>,
        ip: Option<String>,
        instance_id: Option<String>,
    ) -> Result<Option<types::UserServer>> {
        let model = Instance::find()
            .select_only()
            .column(instance::Column::Id)
            .column(instance::Column::InstanceId)
            .column(instance::Column::Ip)
            .column(instance::Column::Namespace)
            .column(instance::Column::Info)
            .column(instance::Column::MacAddr)
            .column(instance::Column::Password)
            .column(instance::Column::SysUser)
            .column(instance::Column::SshPort)
            .column(instance::Column::InstanceGroupId)
            .column_as(instance_group::Column::Name, "instance_group_name")
            .column(instance::Column::Status)
            .column(instance::Column::CreatedTime)
            .column(instance::Column::UpdatedTime)
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .apply_if(namespace, |query, v| {
                query.filter(instance::Column::Namespace.eq(v))
            })
            .apply_if(instance_id, |query, v| {
                query.filter(instance::Column::InstanceId.eq(v))
            })
            .apply_if(ip, |query, v| {
                query.filter(instance::Column::Ip.contains(v))
            });

        let one = model.into_model().one(&self.ctx.db).await?;
        Ok(one)
    }

    pub async fn get_one_user_server(
        &self,
        namespace: Option<String>,
        ip: Option<String>,
        instance_id: Option<String>,
        user_id: String,
    ) -> Result<Option<types::UserServer>> {
        let mut model = User::find()
            .select_only()
            .column(instance::Column::Id)
            .column(instance::Column::InstanceId)
            .column(instance::Column::Ip)
            .column(instance::Column::MacAddr)
            .column(instance::Column::Namespace)
            .column(instance::Column::Info)
            .column(instance::Column::SysUser)
            .column(instance::Column::SshPort)
            .column(instance::Column::Password)
            .column(instance::Column::Status)
            .column(instance::Column::InstanceGroupId)
            .column_as(instance_group::Column::Name, "instance_group_name")
            .column(instance::Column::CreatedTime)
            .column(instance::Column::UpdatedTime)
            .join_rev(
                JoinType::LeftJoin,
                InstanceRole::belongs_to(User)
                    .from(instance_role::Column::RoleId)
                    .to(user::Column::RoleId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Instance::belongs_to(InstanceRole)
                    .condition_type(ConditionType::Any)
                    .on_condition(|a, b| {
                        Expr::col((b.clone(), instance::Column::InstanceGroupId))
                            .equals((a.clone(), instance_role::Column::InstanceGroupId))
                            .and(Expr::col((b, instance::Column::InstanceGroupId)).gt(0))
                            .into_condition()
                    })
                    .from(instance::Column::InstanceId)
                    .to(instance_role::Column::InstanceId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                InstanceGroup::belongs_to(Instance)
                    .from(instance_group::Column::Id)
                    .to(instance::Column::InstanceGroupId)
                    .into(),
            )
            .filter(user::Column::UserId.eq(user_id.clone()))
            .apply_if(instance_id.clone(), |query, v| {
                query.filter(instance::Column::InstanceId.eq(v))
            })
            .apply_if(namespace.clone(), |query, v| {
                query.filter(instance::Column::Namespace.eq(v))
            })
            .as_query()
            .to_owned();

        let model = model.union(
            UnionType::Distinct,
            UserServer::find()
                .select_only()
                .column(instance::Column::Id)
                .column(instance::Column::InstanceId)
                .column(instance::Column::Ip)
                .column(instance::Column::MacAddr)
                .column(instance::Column::Namespace)
                .column(instance::Column::Info)
                .column(instance::Column::SysUser)
                .column(instance::Column::SshPort)
                .column(instance::Column::Password)
                .column(instance::Column::Status)
                .column(instance::Column::InstanceGroupId)
                .column_as(instance_group::Column::Name, "instance_group_name")
                .column(instance::Column::CreatedTime)
                .column(instance::Column::UpdatedTime)
                .join_rev(
                    JoinType::LeftJoin,
                    Instance::belongs_to(UserServer)
                        .condition_type(ConditionType::Any)
                        .on_condition(|a, b| {
                            Expr::col((b.clone(), instance::Column::InstanceGroupId))
                                .equals((a.clone(), user_server::Column::InstanceGroupId))
                                .and(Expr::col((b, instance::Column::InstanceGroupId)).gt(0))
                                .into_condition()
                        })
                        .from(instance::Column::InstanceId)
                        .to(user_server::Column::InstanceId)
                        .into(),
                )
                .join_rev(
                    JoinType::LeftJoin,
                    InstanceGroup::belongs_to(Instance)
                        .from(instance_group::Column::Id)
                        .to(instance::Column::InstanceGroupId)
                        .into(),
                )
                .filter(user_server::Column::UserId.eq(user_id.clone()))
                .apply_if(namespace, |query, v| {
                    query.filter(instance::Column::Namespace.eq(v))
                })
                .apply_if(instance_id.clone(), |query, v| {
                    query.filter(user_server::Column::InstanceId.eq(v))
                })
                .as_query()
                .clone(),
        );

        if let Some(ip) = ip {
            model.and_where(instance::Column::Ip.contains(ip));
        }

        let (sql, vals) = model.clone().build(MysqlQueryBuilder);

        let one = types::UserServer::find_by_statement(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            &sql,
            vals,
        ))
        .one(&self.ctx.db)
        .await?;

        Ok(one)
    }

    pub async fn get_one_user_server_with_permission(
        &self,
        state: AppState,
        user_info: &types::UserInfo,
        instance_id: String,
    ) -> Result<Option<types::UserServer>> {
        let can_manage_instance = state.can_manage_instance(&user_info.user_id).await?;
        let instance_record = if can_manage_instance {
            self.get_one_admin_server(None, None, Some(instance_id))
                .await
        } else {
            self.get_one_user_server(None, None, Some(instance_id), user_info.user_id.to_string())
                .await
        };

        instance_record
    }
}
