pub use sea_orm_migration::prelude::*;
mod m20251016_140448_create_user_table;
mod m20251016_140740_create_authority_table;
mod m20251016_140756_create_role_table;
mod m20251016_140811_create_user_authority_table;
mod m20251016_140822_create_authority_role_table;
mod m20251016_140834_create_group_table;
mod m20251019_134930_set_init_role;
mod m20251019_134920_set_init_group;
mod m20251019_135752_set_init_super_admin;


pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251016_140448_create_user_table::Migration),
            Box::new(m20251016_140740_create_authority_table::Migration),
            Box::new(m20251016_140756_create_role_table::Migration),
            Box::new(m20251016_140811_create_user_authority_table::Migration),
            Box::new(m20251016_140822_create_authority_role_table::Migration),
            Box::new(m20251016_140834_create_group_table::Migration),
            Box::new(m20251019_134930_set_init_role::Migration),
            Box::new(m20251019_134920_set_init_group::Migration),
            Box::new(m20251019_135752_set_init_super_admin::Migration),
        ]
    }
}
