//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "guild")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub default_name: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::rn_object::Entity")]
    RnObject,
    #[sea_orm(has_many = "super::rn_subject::Entity")]
    RnSubject,
}

impl Related<super::rn_object::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RnObject.def()
    }
}

impl Related<super::rn_subject::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RnSubject.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
