pub use sea_orm_migration::prelude::*;

mod m20230127_000001_start;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230127_000001_start::Migration),
        ]
    }
}
