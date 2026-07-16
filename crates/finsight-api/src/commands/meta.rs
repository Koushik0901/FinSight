use crate::error::AppResult;
use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppReady {
    pub version: String,
}

pub async fn app_ready() -> AppResult<AppReady> {
    Ok(AppReady {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
