use sea_orm::entity::prelude::*;
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth_scope")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    #[sea_orm(unique)]
    pub name: String,
    pub description: Option<String>,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    pub async fn exists_by_names(db: &DatabaseConnection, names: &[String]) -> Result<bool, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        let count = Self::find()
            .filter(Column::Name.is_in(names.to_vec()))
            .count(db)
            .await?;
        Ok(count as usize == names.len())
    }
}

impl ActiveModelBehavior for ActiveModel {}
