use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
pub struct Institution {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub sfin_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type, ToSchema)]
pub struct NewInstitution {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub sfin_url: Option<String>,
}
