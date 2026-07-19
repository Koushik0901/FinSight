//! Authenticated browser uploads for commands that historically received a
//! native desktop filesystem path. The server chooses the destination and
//! returns only an opaque token; RPC dispatch resolves that token inside the
//! authenticated user's staging directory.

use crate::auth::AuthedUser;
use crate::state::ServerState;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use finsight_api::error::AppError;
use serde::Serialize;
use std::sync::Arc;

pub const MAX_CSV_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

#[derive(Serialize)]
struct UploadResponse {
    path: String,
}

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (status, Json(AppError::new(code, message))).into_response()
}

pub async fn upload_csv(
    State(st): State<Arc<ServerState>>,
    user: AuthedUser,
    mut multipart: Multipart,
) -> Response {
    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return error(
                StatusCode::BAD_REQUEST,
                "import.missing_file",
                "attach one CSV file in the `file` field",
            )
        }
        Err(e) => {
            return error(
                StatusCode::BAD_REQUEST,
                "import.invalid_upload",
                e.to_string(),
            )
        }
    };
    if field.name() != Some("file") {
        return error(
            StatusCode::BAD_REQUEST,
            "import.missing_file",
            "attach one CSV file in the `file` field",
        );
    }
    let is_csv = field
        .file_name()
        .and_then(|name| std::path::Path::new(name).extension())
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"));
    if !is_csv {
        return error(
            StatusCode::BAD_REQUEST,
            "import.invalid_file_type",
            "the uploaded file must have a .csv extension",
        );
    }
    let bytes = match field.bytes().await {
        Ok(bytes) if !bytes.is_empty() => bytes,
        Ok(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "import.empty_file",
                "the CSV file is empty",
            )
        }
        Err(e) => {
            return error(
                StatusCode::BAD_REQUEST,
                "import.invalid_upload",
                e.to_string(),
            )
        }
    };

    let token = format!("{}.csv", uuid::Uuid::new_v4());
    let imports_dir = crate::registry::user_data_dir(&st.data_dir, &user.user_id).join("imports");
    let path = imports_dir.join(&token);
    let write = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        std::fs::create_dir_all(&imports_dir)?;
        std::fs::write(path, bytes)
    })
    .await;
    match write {
        Ok(Ok(())) => Json(UploadResponse { path: token }).into_response(),
        Ok(Err(e)) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "import.upload_failed",
            e.to_string(),
        ),
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "import.upload_failed",
            e.to_string(),
        ),
    }
}
