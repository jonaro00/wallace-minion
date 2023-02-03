use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .modify_column(ColumnDef::new(User::BankAccountId).integer().unique_key())
                    .add_column(
                        ColumnDef::new(User::Mature)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(User::Table)
                    .modify_column(ColumnDef::new(User::BankAccountId).integer())
                    .drop_column(User::Mature)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum User {
    Table,
    Mature,
    BankAccountId,
}
