pub use sea_orm_migration::prelude::*;

mod m20250412_add_job_soft_deleted;
mod v1_0_0_create_table;
mod v1_1_0_001_create_table;
mod v1_1_0_002_create_table;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(v1_0_0_create_table::Migration),
            Box::new(v1_1_0_001_create_table::Migration),
            Box::new(v1_1_0_002_create_table::Migration),
            Box::new(m20250412_add_job_soft_deleted::Migration),
        ]
    }
}
