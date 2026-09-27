use sea_orm::{
    sea_query::{ColumnDef, Index, Table},
    ConnectionTrait, DbErr, DeriveIden, ExecResult,
};

#[derive(DeriveIden)]
enum CasbinRule {
    Table,
    Id,
    Ptype,
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
}

pub async fn up<C: ConnectionTrait>(conn: &C) -> Result<ExecResult, DbErr> {
    let create_table = Table::create()
        .if_not_exists()
        .table(CasbinRule::Table)
        .col(
            ColumnDef::new(CasbinRule::Id)
                .big_integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        )
        // MySQL max key length is `3072` bytes, in `utf8mb4` charset, it's `3072 / 4 = 768` characters
        // 18 + 125 * 6 = 768
        .col(ColumnDef::new(CasbinRule::Ptype).string_len(18).not_null())
        .col(ColumnDef::new(CasbinRule::V0).string_len(125).not_null())
        .col(ColumnDef::new(CasbinRule::V1).string_len(125).not_null())
        .col(ColumnDef::new(CasbinRule::V2).string_len(125).not_null())
        .col(ColumnDef::new(CasbinRule::V3).string_len(125).not_null())
        .col(ColumnDef::new(CasbinRule::V4).string_len(125).not_null())
        .col(ColumnDef::new(CasbinRule::V5).string_len(125).not_null())
        .index(
            Index::create()
                .name("unique_key_sea_orm_adapter")
                .unique()
                .table(CasbinRule::Table)
                .col(CasbinRule::Ptype)
                .col(CasbinRule::V0)
                .col(CasbinRule::V1)
                .col(CasbinRule::V2)
                .col(CasbinRule::V3)
                .col(CasbinRule::V4)
                .col(CasbinRule::V5),
        )
        .to_owned();

    let builder = conn.get_database_backend();

    conn.execute_raw(builder.build(&create_table)).await
}

pub async fn down<C: ConnectionTrait>(conn: &C) -> Result<ExecResult, DbErr> {
    let drop_table = Table::drop()
        .if_exists()
        .table(CasbinRule::Table)
        .to_owned();

    let builder = conn.get_database_backend();
    conn.execute_raw(builder.build(&drop_table)).await
}
