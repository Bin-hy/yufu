pub use sea_orm_migration::prelude::*;

mod m20251016_140448_create_user_table;
mod m20251016_140740_create_authority_table;
mod m20251016_140756_create_role_table;
mod m20251016_140822_create_authority_role_table;
mod m20251022_152723_create_recording_table;
mod m20251114_154712_create_user_authority;
mod m20251119_194133_create_refresh_token;

use crate::seed;
pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations: Vec<Box<dyn MigrationTrait>> = vec![
            Box::new(m20251016_140448_create_user_table::Migration),
            Box::new(m20251016_140740_create_authority_table::Migration),
            Box::new(m20251016_140756_create_role_table::Migration),
            Box::new(m20251016_140822_create_authority_role_table::Migration),
            Box::new(m20251022_152723_create_recording_table::Migration),
            Box::new(m20251114_154712_create_user_authority::Migration),
            Box::new(m20251119_194133_create_refresh_token::Migration),
        ];
        // https://github.com/SeaQL/sea-orm/blob/aec4795211cfda43ceea951261e698afe8536557/examples/axum_example/seeder/src/lib.rs#L13
        migrations.extend(seed::Migrator::migrations());
        migrations
    }
}
