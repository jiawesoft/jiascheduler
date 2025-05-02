use anyhow::Result;
use sea_orm::{ConnectionTrait, Statement};

use crate::state::AppContext;

use super::types;

mod v100;

#[derive(Clone)]
pub struct MigrationLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> MigrationLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub fn query_version(
        &self,
        name: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::VersionRecord>, u64)> {
        let list: Vec<types::VersionRecord> = vec![types::VersionRecord {
            name: "v1.0.0".to_string(),
            info: "first verison".to_string(),
        }]
        .into_iter()
        .filter(|v| match &name {
            Some(n) => v.name.contains(n),
            None => true,
        })
        .collect();

        let total = list.len();

        let m =
            (((page - 1) * page_size) as usize)..(((page - 1) * page_size + page_size) as usize);

        let retain: Vec<types::VersionRecord> = list
            .into_iter()
            .enumerate()
            .filter(|v| m.contains(&v.0))
            .map(|v| v.1)
            .collect();

        return Ok((retain, total as u64));
    }

    fn version(&self, ver: &str) -> Result<&str> {
        match ver {
            "v1.0.0" => Ok(v100::SQL),
            _ => anyhow::bail!("invalid version {ver}"),
        }
    }

    pub async fn migrate(&self, ver: &str) -> Result<u64> {
        let sql = self.version(ver)?;
        let ret = self.ctx.db.execute_unprepared(sql).await?;
        Ok(ret.rows_affected())
    }

    pub async fn get_database(&self, db: &str) -> Result<Option<(String, String)>> {
        let backend = self.ctx.db.get_database_backend();
        let ret = self
            .ctx
            .db
            .query_one(Statement::from_string(
                backend,
                format!("show create database {db}"),
            ))
            .await?
            .map_or(Ok(None::<(String, String)>), |ret| {
                let v1 = ret.try_get_by_index::<String>(0);
                v1.and_then(|v1| ret.try_get_by_index::<String>(1).map(|v2| Some((v1, v2))))
            })?;
        Ok(ret)
    }
}
