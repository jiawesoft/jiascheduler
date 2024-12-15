use sea_orm::ActiveValue::{self, NotSet, Set};

pub mod executor;
pub mod instance;
pub mod job;
pub mod migration;
pub(crate) mod role;
pub mod ssh;
pub mod team;
pub mod types;
pub(crate) mod user;

pub fn omit_empty_active_value<T>(val: T) -> ActiveValue<T>
where
    T: Default + Into<sea_orm::Value>,
    T: PartialEq,
{
    if val != Default::default() {
        Set(val)
    } else {
        NotSet
    }
}
