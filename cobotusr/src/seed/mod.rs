pub use sea_orm_migration::prelude::*;
mod m20251019_134930_set_init_role;
mod m20251022_123046_set_init_all_authority;
mod m20260331_000005_seed_oidc_scopes;
mod m20260508_000006_seed_permission_authorities;
mod m20260731_000001_seed_mcp_access;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251019_134930_set_init_role::Migration),
            Box::new(m20251022_123046_set_init_all_authority::Migration),
            Box::new(m20260331_000005_seed_oidc_scopes::Migration),
            Box::new(m20260508_000006_seed_permission_authorities::Migration),
            Box::new(m20260731_000001_seed_mcp_access::Migration),
        ]
    }
}
