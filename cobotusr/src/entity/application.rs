use crate::IdType;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, NotSet, PaginatorTrait, Set};
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "application")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: IdType,
    /// Stable, human/admin-facing handle (e.g. `cobotrix`). Unique.
    #[sea_orm(unique)]
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    /// Optional link to the OIDC client that represents this application's
    /// browser/SDK identity. NULL for M2M-only or backend-only apps.
    pub oidc_client_id: Option<IdType>,
    pub create_time: DateTime,
    pub update_time: DateTime,
}

impl Entity {
    pub async fn get_by_app_key(
        db: &DatabaseConnection,
        app_key: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::AppKey.eq(app_key))
            .one(db)
            .await
    }

    pub async fn get_by_oidc_client_id(
        db: &DatabaseConnection,
        oidc_client_id: IdType,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::OidcClientId.eq(oidc_client_id))
            .one(db)
            .await
    }

    pub async fn insert_one(
        db: &DatabaseConnection,
        insert: ApplicationInsertReq,
    ) -> Result<Model, DbErr> {
        let now = chrono::Utc::now().naive_utc();
        let am = ActiveModel {
            id: NotSet,
            app_key: Set(insert.app_key),
            name: Set(insert.name),
            description: Set(insert.description),
            oidc_client_id: Set(insert.oidc_client_id),
            create_time: Set(now),
            update_time: Set(now),
        };
        am.insert(db).await
    }

    pub async fn paginate(
        db: &DatabaseConnection,
        page_size: u64,
        page_number: u64,
    ) -> Result<(u64, Vec<Model>), DbErr> {
        let paginator = Self::find().paginate(db, page_size);
        let total_pages = paginator.num_pages().await?;
        let records = paginator.fetch_page(page_number - 1).await?;
        Ok((total_pages, records))
    }

    pub async fn update_partial(
        db: &DatabaseConnection,
        patch: ApplicationPatchReq,
    ) -> Result<Model, DbErr> {
        let dbm = Self::find_by_id(patch.id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("application".to_string()))?;
        let mut am: ActiveModel = dbm.into();
        am.name = patch.name.map(Set).unwrap_or(NotSet);
        am.description = Set(patch.description);
        am.oidc_client_id = Set(patch.oidc_client_id);
        am.update_time = Set(chrono::Utc::now().naive_utc());
        am.update(db).await
    }

    pub async fn delete_checked(db: &DatabaseConnection, id: IdType) -> Result<bool, DbErr> {
        let res = Self::delete_by_id(id).exec(db).await?;
        Ok(res.rows_affected == 1)
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationVO {
    pub id: IdType,
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    pub oidc_client_id: Option<IdType>,
    pub create_time: String,
    pub update_time: String,
}

impl From<Model> for ApplicationVO {
    fn from(value: Model) -> Self {
        Self {
            id: value.id,
            app_key: value.app_key,
            name: value.name,
            description: value.description,
            oidc_client_id: value.oidc_client_id,
            create_time: value.create_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            update_time: value.update_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationInsertReq {
    pub app_key: String,
    pub name: String,
    pub description: Option<String>,
    pub oidc_client_id: Option<IdType>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationPatchReq {
    pub id: IdType,
    pub name: Option<String>,
    pub description: Option<String>,
    pub oidc_client_id: Option<IdType>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationDeleteReq {
    pub id: IdType,
}
