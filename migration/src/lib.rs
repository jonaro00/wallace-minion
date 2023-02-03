pub use sea_orm_migration::prelude::*;

mod m20230127_000001_start;
mod m20230202_231652_mature_user;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230127_000001_start::Migration),
            Box::new(m20230202_231652_mature_user::Migration),
        ]
    }
}
