pub use sea_orm_migration::prelude::*;

mod m20251016_140448_create_user_table;
mod m20251016_140740_create_authority_table;
mod m20251016_140756_create_role_table;
mod m20251016_140822_create_authority_role_table;
mod m20251022_152723_create_recording_table;
mod m20251114_154712_create_user_authority;
mod m20251119_194133_create_refresh_token;
mod m20260331_000001_create_oauth_client;
mod m20260331_000002_create_oauth_scope;
mod m20260331_000003_create_oauth_token;
mod m20260331_000004_create_oauth_authorization_code;

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
            Box::new(m20260331_000001_create_oauth_client::Migration),
            Box::new(m20260331_000002_create_oauth_scope::Migration),
            Box::new(m20260331_000003_create_oauth_token::Migration),
            Box::new(m20260331_000004_create_oauth_authorization_code::Migration),
        ];
        // Seeds run after all schema migrations
        // Note: SeaORM sorts migrations by name, so seed timestamps
        // must be later than the tables they depend on
        migrations.extend(seed::Migrator::migrations());
        migrations
    }
}
