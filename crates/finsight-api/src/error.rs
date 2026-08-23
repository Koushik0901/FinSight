use finsight_core::CoreError;
use serde::Serialize;
use serde_json::Value;
use specta::Type;

/// Frontend-facing error. `code` is machine-readable (e.g. `core.db.locked`);
/// `message` is human-readable; `details` is structured context for logging
/// and possible inline rendering.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Map the machine-readable `code` to an HTTP status. The mapping is
    /// intentionally coarse — the code is the primary discriminator for the
    /// frontend, the status is only for HTTP semantics and observability.
    pub fn http_status(&self) -> u16 {
        let c = self.code.as_str();
        if c == "rpc.unknown_command" {
            return 404;
        }
        if c == "validation"
            || c.starts_with("validation.")
            || c.starts_with("validation:")
            || c == "core.invalid_state"
            || c == "rpc.bad_arg"
            || c == "rpc.invalid_import_upload"
            || c == "auth.invalid_input"
            || c == "auth.weak_password"
        {
            return 400;
        }
        if c == "conflict"
            || c.starts_with("conflict.")
            || c == "auth.already_setup"
            || c == "auth.username_taken"
        {
            return 409;
        }
        if c == "unprocessable" || c.starts_with("unprocessable") {
            return 422;
        }
        // Internal-type auth codes must not be treated as 401.
        if c == "auth.db"
            || c == "auth.crypto"
            || c == "auth.runtime"
            || c == "auth.migration_failed"
            || c == "core.db"
            || c == "core.pool"
            || c == "core.migration"
            || c == "core.keychain"
        {
            return 500;
        }
        if c == "auth.admin_required" {
            return 403;
        }
        if c == "auth.too_many_attempts" {
            return 429;
        }
        if c == "auth" || c.starts_with("auth.") || c.starts_with("auth:") {
            return 401;
        }
        if c == "internal"
            || c.starts_with("internal")
            || c.starts_with("core.")
            || c.starts_with("rpc.")
            || c.starts_with("simplefin.")
        {
            return 500;
        }
        500
    }
}

/// Explicit conversion from CoreError. Mapping preserves the cause kind so
/// the frontend can distinguish "db locked" from "keychain denied" etc.
impl From<CoreError> for AppError {
    fn from(e: CoreError) -> Self {
        let code = match &e {
            CoreError::Keychain(_) => "core.keychain",
            CoreError::Database(_) => "core.db",
            CoreError::Pool(_) => "core.pool",
            CoreError::Migration(_) => "core.migration",
            CoreError::InvalidState(_) => "core.invalid_state",
            // Validation failures historically surfaced as invalid_state over
            // the wire; keep that code so frontend handling is unchanged.
            CoreError::Validation(_) => "core.invalid_state",
        };
        AppError::new(code, e.to_string())
    }
}

impl From<finsight_providers::ProviderError> for AppError {
    fn from(err: finsight_providers::ProviderError) -> Self {
        match err {
            finsight_providers::ProviderError::InvalidAccessUrl => {
                AppError::new("simplefin.invalid_url", err.to_string())
            }
            finsight_providers::ProviderError::TokenClaimFailed => {
                AppError::new("simplefin.token_used", err.to_string())
            }
            finsight_providers::ProviderError::Forbidden => {
                AppError::new("simplefin.access_revoked", err.to_string())
            }
            finsight_providers::ProviderError::AccountNotFound => {
                AppError::new("simplefin.account_not_found", err.to_string())
            }
            finsight_providers::ProviderError::ServerError(ref msg)
                if msg == "payment required" =>
            {
                AppError::new("simplefin.payment_required", err.to_string())
            }
            _ => AppError::new("simplefin.server_error", err.to_string()),
        }
    }
}

// Deliberately NOT a blanket From<E: Display> — we want each error source
// to map to a meaningful machine-readable code.

pub type AppResult<T> = std::result::Result<T, AppError>;
