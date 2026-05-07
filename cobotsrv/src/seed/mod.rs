pub use sea_orm_migration::prelude::*;
mod m20251019_134930_set_init_role;
mod m20251022_123046_set_init_all_authority;
mod m20260331_000005_seed_oidc_scopes;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251019_134930_set_init_role::Migration),
            Box::new(m20251022_123046_set_init_all_authority::Migration),
            Box::new(m20260331_000005_seed_oidc_scopes::Migration),
        ]
    }
}
