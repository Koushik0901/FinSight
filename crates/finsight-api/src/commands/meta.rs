use crate::error::AppResult;
use serde::Serialize;
use specta::Type;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all="camelCase")]
pub struct AppReady {
    pub version: String,
}

#[utoipa::path(post, path = "/api/rpc/app_ready", responses((status = 200, body = AppReady)))]
pub async fn app_ready() -> AppResult<AppReady> {
    Ok(AppReady {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}