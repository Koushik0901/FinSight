use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CategoryGroup {
    pub id: String,
    pub label: String,
    pub hint: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Category {
    pub id: String,
    pub group_id: String,
    pub label: String,
    pub color: String,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub archived: bool,
}
