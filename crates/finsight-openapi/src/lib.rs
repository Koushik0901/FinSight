use utoipa::OpenApi;

/// FinSight OpenAPI specification.
///
/// Initially an empty spec with only `info` — Task 2 will register every
/// `finsight-api` handler's `#[utoipa::path]` and `ToSchema` DTOs here.
/// The single source of truth is `build_openapi()`, consumed by the Axum
/// route `GET /api/openapi.json` and the exporter binary.
#[derive(OpenApi)]
#[openapi(info(title = "FinSight API", version = "0.1.0"), paths())]
struct ApiDoc;

/// Build the current OpenAPI document.
///
/// Pure function — no I/O, no `ApiState` yet. Later tasks will thread
/// schemas in from `finsight-api` without changing this signature.
pub fn build_openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_is_version_3x() {
        let spec = build_openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let v = json["openapi"].as_str().unwrap_or_default();
        assert!(
            v.starts_with("3."),
            "OpenAPI version must be 3.x, got {v:?}"
        );
    }

    #[test]
    fn openapi_has_expected_info() {
        let spec = build_openapi();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["info"]["title"], "FinSight API");
        assert_eq!(json["info"]["version"], "0.1.0");
    }

    #[test]
    fn openapi_serializes_to_valid_json() {
        let spec = build_openapi();
        let json_str = serde_json::to_string(&spec).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(v.get("openapi").is_some());
        assert!(v.get("info").is_some());
    }
}
