pub use sea_orm_migration::prelude::*;

mod m20251016_140448_create_user_table;
mod m20251016_140740_create_authority_table;
mod m20251016_140756_create_role_table;
mod m20251016_140822_create_authority_role_table;
mod m20251022_152723_create_recording_table;
mod m20251114_154712_create_user_authority;
mod m20251119_194133_create_refresh_token;
mod m20260318_000001_create_im_core_tables;
mod m20260331_000001_create_oauth_client;
mod m20260331_000002_create_oauth_scope;
mod m20260331_000003_create_oauth_token;
mod m20260331_000004_create_oauth_authorization_code;
mod m20260401_000001_create_oauth_consent;
mod m20260411_000001_rebuild_oauth_client_with_owner;
mod m20260508_000001_create_application;
mod m20260508_000002_create_permission;
mod m20260508_000003_create_role_permission;
mod m20260508_000004_create_user_permission;
mod m20260617_000001_create_mcp_token;
mod m20260725_000001_add_oauth_client_public_client;
mod m20261107_000001_security_identity;
mod m20261107_000002_security_permit_audit;
mod m20261108_000001_security_refresh_pair;
mod m20261108_000002_auth_center;
mod m20261109_000001_identity_accounts;
mod m20261109_000002_identity_applications;
mod m20261109_000003_identity_sessions;
mod m20261109_000005_identity_code_rp;
mod m20261109_000006_identity_governance;
mod m20261109_000007_identity_client_secret_revision;
mod m20261109_000008_identity_assignment_operations;
mod m20261109_000009_identity_policy_barrier;

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
            Box::new(m20260318_000001_create_im_core_tables::Migration),
            Box::new(m20260331_000001_create_oauth_client::Migration),
            Box::new(m20260331_000002_create_oauth_scope::Migration),
            Box::new(m20260331_000003_create_oauth_token::Migration),
            Box::new(m20260331_000004_create_oauth_authorization_code::Migration),
            Box::new(m20260401_000001_create_oauth_consent::Migration),
            Box::new(m20260411_000001_rebuild_oauth_client_with_owner::Migration),
            Box::new(m20260508_000001_create_application::Migration),
            Box::new(m20260508_000002_create_permission::Migration),
            Box::new(m20260508_000003_create_role_permission::Migration),
            Box::new(m20260508_000004_create_user_permission::Migration),
            Box::new(m20260617_000001_create_mcp_token::Migration),
            Box::new(m20260725_000001_add_oauth_client_public_client::Migration),
        ];
        // This SeaORM version executes the supplied order. Preserve the legacy
        // schema/seed sequence, then append security migrations so rolling back
        // the new schema never attempts an irreversible legacy seed first.
        migrations.extend(seed::Migrator::migrations());
        migrations.push(Box::new(m20261107_000001_security_identity::Migration));
        migrations.push(Box::new(m20261107_000002_security_permit_audit::Migration));
        migrations.push(Box::new(
            seed::m20261107_000003_security_operations::Migration,
        ));
        migrations.push(Box::new(m20261108_000001_security_refresh_pair::Migration));
        migrations.push(Box::new(m20261108_000002_auth_center::Migration));
        migrations.push(Box::new(m20261109_000001_identity_accounts::Migration));
        migrations.push(Box::new(m20261109_000002_identity_applications::Migration));
        migrations.push(Box::new(m20261109_000003_identity_sessions::Migration));
        migrations.push(Box::new(
            seed::m20261109_000004_identity_operations::Migration,
        ));
        migrations.push(Box::new(m20261109_000005_identity_code_rp::Migration));
        migrations.push(Box::new(m20261109_000006_identity_governance::Migration));
        migrations.push(Box::new(
            m20261109_000007_identity_client_secret_revision::Migration,
        ));
        migrations.push(Box::new(
            m20261109_000008_identity_assignment_operations::Migration,
        ));
        migrations.push(Box::new(
            m20261109_000009_identity_policy_barrier::Migration,
        ));
        migrations.push(Box::new(
            seed::m20261109_000010_identity_reader_operations::Migration,
        ));
        migrations.push(Box::new(
            seed::m20261109_000011_identity_migration_operations::Migration,
        ));
        migrations.push(Box::new(
            seed::m20261109_000012_identity_console_operations::Migration,
        ));
        migrations
    }
}
