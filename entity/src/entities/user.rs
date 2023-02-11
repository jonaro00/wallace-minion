//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(unique)]
    pub bank_account_id: Option<i32>,
    pub mature: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bank_account::Entity",
        from = "Column::BankAccountId",
        to = "super::bank_account::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    BankAccount,
    #[sea_orm(has_many = "super::lol_account::Entity")]
    LolAccount,
}

impl Related<super::bank_account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BankAccount.def()
    }
}

impl Related<super::lol_account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LolAccount.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
