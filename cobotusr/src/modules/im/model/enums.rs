use sea_orm::entity::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum ConversationType {
    #[sea_orm(num_value = 1)]
    Single,
    #[sea_orm(num_value = 2)]
    Group,
}

impl ConversationType {
    pub fn from_i16(value: i16) -> Result<Self, String> {
        Self::try_from(value)
    }
}

impl From<ConversationType> for i16 {
    fn from(value: ConversationType) -> Self {
        match value {
            ConversationType::Single => 1,
            ConversationType::Group => 2,
        }
    }
}

impl TryFrom<i16> for ConversationType {
    type Error = String;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Single),
            2 => Ok(Self::Group),
            _ => Err("conversation_type must be 1(single) or 2(group)".to_string()),
        }
    }
}

impl Serialize for ConversationType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i16((*self).into())
    }
}

impl<'de> Deserialize<'de> for ConversationType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i16::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum MessageType {
    #[sea_orm(num_value = 1)]
    Text,
    #[sea_orm(num_value = 2)]
    Image,
    #[sea_orm(num_value = 3)]
    Audio,
    #[sea_orm(num_value = 4)]
    Video,
    #[sea_orm(num_value = 5)]
    File,
    #[sea_orm(num_value = 6)]
    System,
}

impl MessageType {
    pub fn from_i16(value: i16) -> Result<Self, String> {
        Self::try_from(value)
    }
}

impl From<MessageType> for i16 {
    fn from(value: MessageType) -> Self {
        match value {
            MessageType::Text => 1,
            MessageType::Image => 2,
            MessageType::Audio => 3,
            MessageType::Video => 4,
            MessageType::File => 5,
            MessageType::System => 6,
        }
    }
}

impl TryFrom<i16> for MessageType {
    type Error = String;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Text),
            2 => Ok(Self::Image),
            3 => Ok(Self::Audio),
            4 => Ok(Self::Video),
            5 => Ok(Self::File),
            6 => Ok(Self::System),
            _ => Err("unsupported msg_type".to_string()),
        }
    }
}

impl Serialize for MessageType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i16((*self).into())
    }
}

impl<'de> Deserialize<'de> for MessageType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i16::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum OutboxStatus {
    #[sea_orm(num_value = 0)]
    Pending,
    #[sea_orm(num_value = 2)]
    Sent,
    #[sea_orm(num_value = 3)]
    Retrying,
    #[sea_orm(num_value = 4)]
    Failed,
}

impl From<OutboxStatus> for i16 {
    fn from(value: OutboxStatus) -> Self {
        match value {
            OutboxStatus::Pending => 0,
            OutboxStatus::Sent => 2,
            OutboxStatus::Retrying => 3,
            OutboxStatus::Failed => 4,
        }
    }
}

impl TryFrom<i16> for OutboxStatus {
    type Error = String;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Pending),
            2 => Ok(Self::Sent),
            3 => Ok(Self::Retrying),
            4 => Ok(Self::Failed),
            _ => Err(format!("unsupported outbox status: {value}")),
        }
    }
}

impl Serialize for OutboxStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i16((*self).into())
    }
}

impl<'de> Deserialize<'de> for OutboxStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i16::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}
