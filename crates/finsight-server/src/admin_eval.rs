//! Debug-only, admin-gated diagnostic route: runs `finsight-eval`'s private,
//! local precision-eval (`finsight_eval::categorization::private_eval`) over
//! the CALLING admin's own real `categorizations` corrections
//! (`source = 'user'`).
//!
//! This is the reframed alternative to issue #89's literal ask — "a labeled
//! merchant-diverse transaction corpus checked into the public repo" cannot
//! be honestly built by an agent (any labels it invented would be
//! fabricated; the repo owner's real transactions becoming a public
//! artifact would be a privacy incident). This route instead measures a
//! self-hosted instance's OWN real, human-made corrections — real ground
//! truth that never leaves that instance.
//!
//! Deliberately NOT wired through `/api/rpc/{cmd}` — no `bindings.ts` entry,
//! no `finsight-bindings` wrapper, no `dispatch.rs` match arm, not part of
//! the generated command contract `tests/parity.rs` enforces. This is a
//! diagnostic surface, not a shared command the Copilot or any remote caller
//! should ever be able to invoke or forward. It reuses the SAME
//! authenticated-runtime lookup `/api/rpc/{cmd}` uses
//! (`registry::get_or_bootstrap`), so it costs no new key-handling code and
//! never touches a DB key the session hasn't already unwrapped.
//!
//! The response body is `text/plain`, rendered via `PrivateEvalResult`'s
//! `Display` impl — which always bundles the N/merchant counts (and the
//! small-N caveat, when it applies) together with any percentage. There is
//! no JSON field a caller could pluck a bare number from.

use crate::auth::AdminUser;
use crate::state::ServerState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use finsight_api::error::AppError;
use finsight_core::repos::run;
use std::sync::Arc;

pub async fn private_category_eval(
    State(st): State<Arc<ServerState>>,
    admin: AdminUser,
) -> Response {
    let user = admin.0;
    let rt = match st
        .registry
        .get_or_bootstrap(&st.data_dir, &user.user_id, &user.db_key_hex)
        .await
    {
        Ok(rt) => rt,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError::new("admin.runtime", e.to_string())),
            )
                .into_response()
        }
    };
    st.registry.touch(&user.user_id);

    let db = (*rt.api.db).clone();
    let result = run(&db, move |conn| {
        finsight_eval::categorization::private_eval::run_private_eval(conn).map_err(Into::into)
    })
    .await;

    match result {
        Ok(report) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            report.to_string(),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError::new("admin.private_eval", e.to_string())),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    /// Happy path: a fresh admin account has zero `source='user'`
    /// categorizations, so the route must succeed (not 500) and say so
    /// plainly rather than rendering a bare/misleading number.
    #[tokio::test]
    async fn admin_with_no_corrections_gets_an_explicit_n_equals_zero() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let cookie = crate::router::tests::setup_and_login(&app).await;

        let res = app
            .oneshot(
                Request::get("/api/admin/private-category-eval")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(axum::http::header::CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("N=0"), "got: {body}");
        assert!(
            !body.contains('%'),
            "a zero-N report must not render a percentage, got: {body}"
        );
    }

    #[tokio::test]
    async fn admin_route_requires_authentication() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let res = app
            .oneshot(
                Request::get("/api/admin/private-category-eval")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn non_admin_user_is_forbidden() {
        let state = crate::router::tests::test_state();
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let admin_cookie = crate::router::tests::setup_and_login(&app).await;

        // Admin creates a second, non-admin user ("bob").
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/auth/users")
                    .header("content-type", "application/json")
                    .header("cookie", admin_cookie)
                    .body(Body::from(
                        r#"{"username":"bob","password":"bobs-password-1"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Bob logs in and hits the admin-only diagnostic route.
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"username":"bob","password":"bobs-password-1"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bob_cookie = res
            .headers()
            .get(axum::http::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let res = app
            .oneshot(
                Request::get("/api/admin/private-category-eval")
                    .header("cookie", bob_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
