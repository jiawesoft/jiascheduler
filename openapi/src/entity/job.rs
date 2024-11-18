//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.0

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, Default)]
#[sea_orm(table_name = "job")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    pub eid: String,
    pub executor_id: u64,
    pub job_type: String,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub code: String,
    pub info: String,
    pub bundle_script: Option<Json>,
    pub upload_file: String,
    pub work_dir: String,
    pub work_user: String,
    pub timeout: u64,
    pub max_retry: u8,
    pub max_parallel: u8,
    pub is_public: i8,
    pub display_on_dashboard: bool,
    pub created_user: String,
    pub updated_user: String,
    pub args: Option<Json>,
    pub created_time: DateTimeUtc,
    pub updated_time: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}