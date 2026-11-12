use crate::IdType;
use crate::entity::{authority, recording};
use crate::error::AppError;
use crate::rbac::handle_query_path;
use crate::result::Result;
use anyhow::anyhow;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};

pub struct Service;
impl Service {
    pub async fn add_recording(
        db: &DatabaseConnection,
        method: String,
        full_uri: String,
        user_id: IdType,
    ) -> Result<()> {
        // 获取到用户的操作 get user operations.
        // 查找权限表记录该操作 find the authority in the table.
        let authority_uri = handle_query_path(full_uri.clone());
        let (operation_name, operation_uri) = match authority::Entity::find()
            .filter(authority::Column::Method.eq(method.clone()))
            .filter(authority::Column::AuthorityUri.eq(authority_uri))
            .one(db)
            .await
        {
            Ok(Some(authority_obj)) => (authority_obj.name, full_uri),
            Ok(None) => ("Unknown Request...".to_string(), full_uri),
            Err(e) => return Err(AppError::InternalServerError(anyhow!(e))),
        };

        // save into db.
        let new_recording = recording::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(operation_name.to_owned()),
            method: Set(method.to_owned()),
            authority_uri: Set(operation_uri.to_owned()),
            user_id: Set(user_id.to_owned()),
            create_time: Set(Utc::now().naive_utc().to_owned()),
        };
        if let Err(e) = new_recording.insert(db).await {
            return Err(AppError::InternalServerError(anyhow!(e)));
        }
        Ok(())
    }
}
