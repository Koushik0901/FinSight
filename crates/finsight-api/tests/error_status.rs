use finsight_api::error::AppError;

#[test]
fn validation_maps_to_400() {
    let e = AppError::new("validation", "bad input");
    assert_eq!(e.http_status(), 400);
}

#[test]
fn auth_maps_to_401() {
    let e = AppError::new("auth.required", "need login");
    assert_eq!(e.http_status(), 401);
}

#[test]
fn conflict_maps_to_409() {
    let e = AppError::new("conflict", "already exists");
    assert_eq!(e.http_status(), 409);
}

#[test]
fn unprocessable_maps_to_422() {
    let e = AppError::new("unprocessable", "cannot process");
    assert_eq!(e.http_status(), 422);
}

#[test]
fn internal_maps_to_500() {
    let e = AppError::new("internal", "boom");
    assert_eq!(e.http_status(), 500);
}

#[test]
fn unknown_command_maps_to_404() {
    let e = AppError::new("rpc.unknown_command", "no such cmd");
    assert_eq!(e.http_status(), 404);
}

#[test]
fn core_invalid_state_maps_to_400() {
    // CoreError::Validation surfaces as core.invalid_state over the wire
    let e = AppError::new("core.invalid_state", "validation: missing field");
    assert_eq!(e.http_status(), 400);
}

#[test]
fn bad_arg_maps_to_400() {
    let e = AppError::new("rpc.bad_arg", "argument `x`: missing");
    assert_eq!(e.http_status(), 400);
}

#[test]
fn invalid_import_upload_maps_to_400() {
    let e = AppError::new("rpc.invalid_import_upload", "bad csv token");
    assert_eq!(e.http_status(), 400);
}

#[test]
fn fallback_is_500() {
    let e = AppError::new("something.unexpected", "oops");
    assert_eq!(e.http_status(), 500);
}
