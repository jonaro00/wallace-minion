use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Guild::Table)
                    .add_column(ColumnDef::new(Guild::DefaultName).string_len(100))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(RNSubject::Table)
                    .col(ColumnDef::new(RNSubject::GuildId).big_unsigned().not_null())
                    .col(ColumnDef::new(RNSubject::Value).string_len(45).not_null())
                    .primary_key(
                        Index::create()
                            .col(RNSubject::GuildId)
                            .col(RNSubject::Value),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_rn_subject_guild")
                            .from(RNSubject::Table, RNSubject::GuildId)
                            .to(Guild::Table, Guild::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(RNObject::Table)
                    .col(
                        ColumnDef::new(RNObject::GuildId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RNObject::Value)
                            .string_len(45)
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(RNObject::GuildId)
                            .col(RNObject::Value),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_rn_object_guild")
                            .from(RNObject::Table, RNObject::GuildId)
                            .to(Guild::Table, Guild::Id)
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
            .alter_table(
                Table::alter()
                    .table(Guild::Table)
                    .drop_column(Guild::DefaultName)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(RNSubject::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RNObject::Table).to_owned())
            .await?;
        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Guild {
    Table,
    Id,
    DefaultName,
}
#[derive(Iden)]
enum RNSubject {
    Table,
    GuildId,
    Value,
}
#[derive(Iden)]
enum RNObject {
    Table,
    GuildId,
    Value,
}
