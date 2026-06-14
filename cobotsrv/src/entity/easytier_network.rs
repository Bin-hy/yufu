use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "easytier_network")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    #[sea_orm(unique)]
    pub robot_sn: String, // 设备唯一标识
    #[sea_orm(unique)]
    pub virtual_ip: Option<String>, // 分配的 IP
    pub status: String,
    pub allocated_at: DateTime,
    pub last_online_at: Option<DateTime>,
    pub lease_expired_at: Option<DateTime>,
}

#[derive(Debug, PartialEq)]
pub enum NetStatus {
    Active,
    Reclaimed,
    Scrapped,
}

impl NetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NetStatus::Active => "ACTIVE",
            NetStatus::Reclaimed => "RECLAIMED",
            NetStatus::Scrapped => "SCRAPPED",
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
