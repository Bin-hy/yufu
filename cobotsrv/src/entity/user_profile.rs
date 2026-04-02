use crate::IdType;
use sea_orm::entity::prelude::*;
use serde::Serialize;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "o_user_profile")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    #[sea_orm(unique)]
    pub user_id: IdType,
    pub name: Option<String>,
    #[sea_orm(unique)]
    pub email: Option<String>,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub create_time: DateTime,
    pub update_time: DateTime,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: HasOne<super::user::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
