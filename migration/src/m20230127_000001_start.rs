use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BankAccount::Table)
                    .col(
                        ColumnDef::new(BankAccount::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BankAccount::Balance)
                            .big_unsigned()
                            .default(0i64)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .col(
                        ColumnDef::new(User::Id)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(User::BankAccountId).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_user_bank_account")
                            .from(User::Table, User::BankAccountId)
                            .to(BankAccount::Table, BankAccount::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(LolAccount::Table)
                    .col(
                        ColumnDef::new(LolAccount::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(LolAccount::Server).string_len(10).not_null())
                    .col(
                        ColumnDef::new(LolAccount::Summoner)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(LolAccount::UserId).big_unsigned().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_lol_account_user")
                            .from(LolAccount::Table, LolAccount::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Guild::Table)
                    .col(
                        ColumnDef::new(Guild::Id)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Channel::Table)
                    .col(
                        ColumnDef::new(Channel::Id)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Task::Table)
                    .col(
                        ColumnDef::new(Task::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Task::Cron).string_len(255).not_null())
                    .col(ColumnDef::new(Task::Cmd).string_len(255).not_null())
                    .col(ColumnDef::new(Task::Arg).string_len(255))
                    .col(ColumnDef::new(Task::ChannelId).big_unsigned().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_task_channel")
                            .from(Task::Table, Task::ChannelId)
                            .to(Channel::Table, Channel::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Guild::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Channel::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BankAccount::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(LolAccount::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Task::Table).to_owned())
            .await?;
        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Guild {
    Table,
    Id,
}
#[derive(Iden)]
enum User {
    Table,
    Id,
    BankAccountId,
}
#[derive(Iden)]
enum Channel {
    Table,
    Id,
}
#[derive(Iden)]
enum LolAccount {
    Table,
    Id,
    Server,
    Summoner,
    UserId,
}
#[derive(Iden)]
enum BankAccount {
    Table,
    Id,
    Balance,
}
#[derive(Iden)]
enum Task {
    Table,
    Id,
    Cron,
    Cmd,
    Arg,
    ChannelId,
}
