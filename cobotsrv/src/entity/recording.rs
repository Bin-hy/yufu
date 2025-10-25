use crate::entity::user;
use crate::entity::user::UserVO;
/// 用户操作记录表
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "recording")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub create_time: DateTime,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: HasOne<super::user::Entity>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Clone, Serialize, Deserialize)]
pub struct RecordingVO {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub create_time: String,
    pub user_vo: UserVO,
}

impl From<(Model, Option<user::Model>)> for RecordingVO {
    fn from((recording_db, user_db): (Model, Option<user::Model>)) -> Self {
        Self {
            id: recording_db.id.clone(),
            name: recording_db.name.clone(),
            user_id: recording_db.user_id.clone(),
            create_time: recording_db
                .create_time
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            user_vo: user_db.map(UserVO::from).unwrap(), // !!!!!!!!!!!! 我不知道怎么处理!!!!,这里是肯定有值的，但是直接unwrap()并不安全
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
