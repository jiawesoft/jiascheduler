use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, Default)]
#[sea_orm(table_name = "workflow_timer")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    pub name: String,
    pub workflow_id: u64,
    pub version_id: u64,
    pub process_args: Option<Json>,
    pub timer_expr: Json,
    pub schedule_guid: String,
    pub is_active: bool,
    pub next_time: Option<DateTimeLocal>,
    pub prev_time: Option<DateTimeLocal>,
    pub info: String,
    pub startup_error: String,
    pub created_user: String,
    pub updated_user: String,
    pub created_time: DateTimeLocal,
    pub updated_time: DateTimeLocal,
    #[serde(default)]
    pub is_deleted: bool,
    pub deleted_at: Option<DateTimeLocal>,
    #[serde(default)]
    pub deleted_by: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
