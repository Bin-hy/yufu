use crate::entity::{authority, recording};
use crate::error::AppError;
use crate::result::Result;
use anyhow::anyhow;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;
pub struct Service;
impl Service {
    pub async fn add_recording(
        db: &DatabaseConnection,
        method: String,
        uri: String,
        user_id: String,
    ) -> Result<()> {
        // 获取到用户的操作 get user operations.
        // 查找权限表记录该操作 find the authority in the table.

        let authority_req = format!("{}?{}", method.to_lowercase(), uri);
        let (operation_name, operation_uri) =
            match authority::Entity::find_by_unique_authority(authority_req.clone())
                .one(db)
                .await
            {
                Ok(Some(authority_obj)) => (authority_obj.name, authority_obj.authority_uri),
                Ok(None) => ("Unknown Request...".to_string(), authority_req),
                Err(e) => return Err(AppError::InternalServerError(anyhow!(e))),
            };

        // save into db.
        let new_recording = recording::ActiveModel {
            id: Set(Uuid::new_v4().to_string().to_owned()),
            name: Set(operation_name.to_owned()),
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
