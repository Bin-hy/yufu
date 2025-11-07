use crate::IdType;
use crate::entity::user;
use crate::entity::user::UserVO;
/// 用户操作记录表
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "recording")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    pub name: String,
    pub authority_uri: String,
    pub user_id: IdType,
    pub create_time: DateTime,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: HasOne<super::user::Entity>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Clone, Serialize, Deserialize)]
pub struct RecordingVO {
    pub id: IdType,
    pub name: String,
    pub authority_uri: String,
    pub user_id: IdType,
    pub create_time: String,
    pub user_vo: UserVO,
}

impl From<(Model, Option<user::Model>)> for RecordingVO {
    fn from((recording_db, user_db): (Model, Option<user::Model>)) -> Self {
        Self {
            id: recording_db.id,
            name: recording_db.name.clone(),
            authority_uri: recording_db.authority_uri.clone(),
            user_id: recording_db.user_id,
            create_time: recording_db
                .create_time
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            user_vo: user_db.map(UserVO::from).unwrap(), // TODO :处理 unwrap 可能出现的问题
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
