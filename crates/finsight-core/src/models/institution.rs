use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Institution {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub sfin_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewInstitution {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub sfin_url: Option<String>,
}
