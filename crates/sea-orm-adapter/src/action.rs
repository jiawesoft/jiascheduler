use casbin::{error::AdapterError, Error as CasbinError, Filter, Result};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter,
};

use crate::entity::{self, Column, Entity};

#[derive(Debug, Default)]
pub(crate) struct Rule<'a> {
    pub(crate) v0: &'a str,
    pub(crate) v1: &'a str,
    pub(crate) v2: &'a str,
    pub(crate) v3: &'a str,
    pub(crate) v4: &'a str,
    pub(crate) v5: &'a str,
}

impl<'a> Rule<'a> {
    pub(crate) fn from_str(value: &'a [&'a str]) -> Self {
        #[allow(clippy::get_first)]
        Rule {
            v0: value.get(0).map_or("", |x| x),
            v1: value.get(1).map_or("", |x| x),
            v2: value.get(2).map_or("", |x| x),
            v3: value.get(3).map_or("", |x| x),
            v4: value.get(4).map_or("", |x| x),
            v5: value.get(5).map_or("", |x| x),
        }
    }

    pub(crate) fn from_string(value: &'a [String]) -> Self {
        #[allow(clippy::get_first)]
        Rule {
            v0: value.get(0).map_or("", |x| x),
            v1: value.get(1).map_or("", |x| x),
            v2: value.get(2).map_or("", |x| x),
            v3: value.get(3).map_or("", |x| x),
            v4: value.get(4).map_or("", |x| x),
            v5: value.get(5).map_or("", |x| x),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuleWithType<'a> {
    pub(crate) ptype: &'a str,
    pub(crate) v0: &'a str,
    pub(crate) v1: &'a str,
    pub(crate) v2: &'a str,
    pub(crate) v3: &'a str,
    pub(crate) v4: &'a str,
    pub(crate) v5: &'a str,
}

impl<'a> RuleWithType<'a> {
    pub(crate) fn from_rule(ptype: &'a str, rule: Rule<'a>) -> Self {
        RuleWithType {
            ptype,
            v0: rule.v0,
            v1: rule.v1,
            v2: rule.v2,
            v3: rule.v3,
            v4: rule.v4,
            v5: rule.v5,
        }
    }
}

pub(crate) async fn remove_policy<'conn, 'rule, C: ConnectionTrait>(
    conn: &'conn C,
    rule: RuleWithType<'rule>,
) -> Result<bool> {
    Entity::delete_many()
        .filter(Column::Ptype.eq(rule.ptype))
        .filter(Column::V0.eq(rule.v0))
        .filter(Column::V1.eq(rule.v1))
        .filter(Column::V2.eq(rule.v2))
        .filter(Column::V3.eq(rule.v3))
        .filter(Column::V4.eq(rule.v4))
        .filter(Column::V5.eq(rule.v5))
        .exec(conn)
        .await
        .map(|count| count.rows_affected == 1)
        .map_err(|err| CasbinError::from(AdapterError(Box::new(err))))
}

pub(crate) async fn remove_policies<'conn, 'rule, C: ConnectionTrait>(
    conn: &'conn C,
    rules: Vec<RuleWithType<'rule>>,
) -> Result<bool> {
    for rule in rules {
        remove_policy(conn, rule).await?;
    }

    Ok(true)
}

pub(crate) async fn remove_filtered_policy<'conn, 'rule, C: ConnectionTrait>(
    conn: &'conn C,
    ptype: &'rule str,
    index_of_match_start: usize,
    rule: Rule<'rule>,
) -> Result<bool> {
    let mut conditions = Condition::all().add(Column::Ptype.eq(ptype));

    if index_of_match_start == 0 {
        conditions = conditions
            .add_option((!rule.v0.is_empty()).then(|| Column::V0.eq(rule.v0)))
            .add_option((!rule.v1.is_empty()).then(|| Column::V1.eq(rule.v1)))
            .add_option((!rule.v2.is_empty()).then(|| Column::V2.eq(rule.v2)))
            .add_option((!rule.v3.is_empty()).then(|| Column::V3.eq(rule.v3)))
            .add_option((!rule.v4.is_empty()).then(|| Column::V4.eq(rule.v4)))
            .add_option((!rule.v5.is_empty()).then(|| Column::V5.eq(rule.v5)));
    } else if index_of_match_start == 1 {
        conditions = conditions
            .add_option((!rule.v0.is_empty()).then(|| Column::V1.eq(rule.v0)))
            .add_option((!rule.v1.is_empty()).then(|| Column::V2.eq(rule.v1)))
            .add_option((!rule.v2.is_empty()).then(|| Column::V3.eq(rule.v2)))
            .add_option((!rule.v3.is_empty()).then(|| Column::V4.eq(rule.v3)))
            .add_option((!rule.v4.is_empty()).then(|| Column::V5.eq(rule.v4)));
    } else if index_of_match_start == 2 {
        conditions = conditions
            .add_option((!rule.v0.is_empty()).then(|| Column::V2.eq(rule.v0)))
            .add_option((!rule.v1.is_empty()).then(|| Column::V3.eq(rule.v1)))
            .add_option((!rule.v2.is_empty()).then(|| Column::V4.eq(rule.v2)))
            .add_option((!rule.v3.is_empty()).then(|| Column::V5.eq(rule.v3)));
    } else if index_of_match_start == 3 {
        conditions = conditions
            .add_option((!rule.v0.is_empty()).then(|| Column::V3.eq(rule.v0)))
            .add_option((!rule.v1.is_empty()).then(|| Column::V4.eq(rule.v1)))
            .add_option((!rule.v2.is_empty()).then(|| Column::V5.eq(rule.v2)));
    } else if index_of_match_start == 4 {
        conditions = conditions
            .add_option((!rule.v0.is_empty()).then(|| Column::V4.eq(rule.v0)))
            .add_option((!rule.v1.is_empty()).then(|| Column::V5.eq(rule.v1)));
    } else {
        conditions = conditions.add_option((!rule.v0.is_empty()).then(|| Column::V5.eq(rule.v0)));
    }

    Entity::delete_many()
        .filter(conditions)
        .exec(conn)
        .await
        .map(|count| count.rows_affected >= 1)
        .map_err(|err| CasbinError::from(AdapterError(Box::new(err))))
}

pub(crate) async fn load_policy<C: ConnectionTrait>(conn: &C) -> Result<Vec<entity::Model>> {
    entity::Entity::find()
        .all(conn)
        .await
        .map_err(|err| CasbinError::from(AdapterError(Box::new(err))))
}

pub(crate) async fn load_filtered_policy<'conn, 'filter, C: ConnectionTrait>(
    conn: &'conn C,
    filter: Filter<'filter>,
) -> Result<Vec<entity::Model>> {
    let g_filter = Rule::from_str(&filter.g);
    let p_filter = Rule::from_str(&filter.p);

    entity::Entity::find()
        .filter(
            Condition::any()
                .add(
                    Condition::all()
                        .add(Column::Ptype.starts_with("g"))
                        .add_option((!g_filter.v0.is_empty()).then(|| Column::V0.eq(g_filter.v0)))
                        .add_option((!g_filter.v1.is_empty()).then(|| Column::V1.eq(g_filter.v1)))
                        .add_option((!g_filter.v2.is_empty()).then(|| Column::V2.eq(g_filter.v2)))
                        .add_option((!g_filter.v3.is_empty()).then(|| Column::V3.eq(g_filter.v3)))
                        .add_option((!g_filter.v4.is_empty()).then(|| Column::V4.eq(g_filter.v4)))
                        .add_option((!g_filter.v5.is_empty()).then(|| Column::V5.eq(g_filter.v5))),
                )
                .add(
                    Condition::all()
                        .add(Column::Ptype.starts_with("p"))
                        .add_option((!p_filter.v0.is_empty()).then(|| Column::V0.eq(p_filter.v0)))
                        .add_option((!p_filter.v1.is_empty()).then(|| Column::V1.eq(p_filter.v1)))
                        .add_option((!p_filter.v2.is_empty()).then(|| Column::V2.eq(p_filter.v2)))
                        .add_option((!p_filter.v3.is_empty()).then(|| Column::V3.eq(p_filter.v3)))
                        .add_option((!p_filter.v4.is_empty()).then(|| Column::V4.eq(p_filter.v4)))
                        .add_option((!p_filter.v5.is_empty()).then(|| Column::V5.eq(p_filter.v5))),
                ),
        )
        .all(conn)
        .await
        .map_err(|err| CasbinError::from(AdapterError(Box::new(err))))
}

pub(crate) async fn save_policies<'conn, 'rule, C: ConnectionTrait>(
    conn: &'conn C,
    rules: Vec<RuleWithType<'rule>>,
) -> Result<()> {
    clear_policy(conn).await?;
    add_policies(conn, rules).await?;
    Ok(())
}

pub(crate) async fn add_policy<'conn, 'rule, C: ConnectionTrait>(
    conn: &'conn C,
    rule: RuleWithType<'rule>,
) -> Result<bool> {
    let model = entity::ActiveModel {
        id: NotSet,
        ptype: Set(rule.ptype.to_string()),
        v0: Set(rule.v0.to_string()),
        v1: Set(rule.v1.to_string()),
        v2: Set(rule.v2.to_string()),
        v3: Set(rule.v3.to_string()),
        v4: Set(rule.v4.to_string()),
        v5: Set(rule.v5.to_string()),
    };

    model
        .insert(conn)
        .await
        .map_err(|err| CasbinError::from(AdapterError(Box::new(err))))?;

    Ok(true)
}

pub(crate) async fn add_policies<'conn, 'rule, C: ConnectionTrait>(
    conn: &'conn C,
    rules: Vec<RuleWithType<'rule>>,
) -> Result<bool> {
    for rule in rules {
        add_policy(conn, rule).await?;
    }

    Ok(true)
}

pub(crate) async fn clear_policy<C: ConnectionTrait>(conn: &C) -> Result<()> {
    entity::Entity::delete_many()
        .exec(conn)
        .await
        .map_err(|err| CasbinError::from(AdapterError(Box::new(err))))?;

    Ok(())
}
