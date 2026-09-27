# sea-orm-adapter

[![Crates.io version](https://img.shields.io/crates/v/sea-orm-adapter.svg?style=flat-square)](https://crates.io/crates/sea-orm-adapter)

Sea ORM Adapter is the [Sea ORM](https://github.com/SeaQL/sea-orm) adapter for [Casbin-rs](https://github.com/casbin/casbin-rs). With this library, Casbin can load policy from Sea ORM supported database or save policy to it with fully asynchronous support.

Based on [Sea ORM](https://github.com/SeaQL/sea-orm), The current supported databases are:

- [Mysql](https://www.mysql.com/)
- [Postgres](https://github.com/lib/pq)
- [SQLite](https://www.sqlite.org)

## Example

```rust
use casbin::{CoreApi, DefaultModel, Enforcer};
use sea_orm::Database;
use sea_orm_adapter::SeaOrmAdapter;

#[tokio::main]
async fn main() {
    let m = DefaultModel::from_file("examples/rbac_model.conf")
        .await
        .unwrap();

    let db = Database::connect("mysql://root:123456@localhost:3306/casbin")
        .await
        .unwrap();

    let a = SeaOrmAdapter::new(db).await.unwrap();
    let e = Enforcer::new(m, a).await.unwrap();
}
```
