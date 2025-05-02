use crate::{
    entity::{self, executor, prelude::*},
    state::AppContext,
};
use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QueryTrait,
};

pub struct ExecutorList(Vec<entity::executor::Model>);

impl ExecutorList {
    pub fn get_by_id(&self, executor_id: u64) -> Option<executor::Model> {
        let v = self
            .0
            .iter()
            .find(|&v| v.id == executor_id)
            .map(|v| v.to_owned());
        v
    }
}

#[derive(Clone)]
pub struct ExecutorLogic<'a> {
    ctx: &'a AppContext,
}
impl<'a> ExecutorLogic<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn get_by_id(&self, id: u32) -> Result<Option<executor::Model>> {
        let one = Executor::find_by_id(id).one(&self.ctx.db).await?;
        Ok(one)
    }

    pub async fn query_executor(
        &self,
        default_id: Option<u64>,
        name: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<entity::executor::Model>, u64)> {
        let model = Executor::find().apply_if(name, |query, v| {
            query.filter(entity::executor::Column::Name.contains(v))
        });

        let total = model.clone().count(&self.ctx.db).await?;

        let list = model
            .apply_if(default_id, |query, v| {
                query.order_by_desc(executor::Column::Id.eq(v))
            })
            .order_by_asc(entity::executor::Column::Id)
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page)
            .await?;
        Ok((list, total))
    }

    pub async fn get_all_by_executor_id(&self, executor_id: Vec<u64>) -> Result<ExecutorList> {
        let model = Executor::find().filter(entity::executor::Column::Id.is_in(executor_id));

        let list = model
            .order_by_asc(entity::executor::Column::Id)
            .all(&self.ctx.db)
            .await?;
        Ok(ExecutorList(list))
    }

    pub async fn save_executor(
        &self,
        model: entity::executor::ActiveModel,
    ) -> Result<entity::executor::ActiveModel> {
        let model = model.save(&self.ctx.db).await?;
        Ok(model)
    }

    pub async fn delete_job(&self, id: u32) -> Result<u64> {
        let ret = Executor::delete_by_id(id).exec(&self.ctx.db).await?;
        Ok(ret.rows_affected)
    }
}
