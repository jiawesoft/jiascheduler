use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let sql = include_str!("../sql/m20250412_add_job_soft_deleted/up.sql");
        db.execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let sql = include_str!("../sql/m20250412_add_job_soft_deleted/down.sql");
        db.execute_unprepared(sql).await?;
        Ok(())
    }
}
