use serde::{Deserialize, Serialize};

pub mod device;
pub mod easytier_network;

#[derive(Deserialize, Serialize, Clone)]
pub struct PageSelectQuery {
    pub page_size: u64,
    pub page_number: u64,
}

#[derive(Serialize, Clone, Deserialize)]
pub struct PageSelectQueryResult<T>
where
    T: Serialize + Clone,
{
    pub page_size: u64,
    pub page_number: u64,
    pub total_pages: u64,
    pub data_list: Vec<T>,
}

impl Default for PageSelectQuery {
    fn default() -> Self {
        Self {
            page_size: 10,
            page_number: 1,
        }
    }
}
