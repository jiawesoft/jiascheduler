pub use sea_orm_migration::prelude::*;

mod v1_0_0_create_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(v1_0_0_create_table::Migration)]
    }
}
