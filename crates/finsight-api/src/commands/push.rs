//! Web Push for the installed PWA.
//!
//! FinSight already streams live events over SSE (`/api/events`), but that only
//! reaches the app while a tab is open and connected. Web Push is the only
//! channel that reaches a CLOSED app — the browser's push service holds the
//! message and wakes our service worker (`ui/public/push-sw.js`) to show it.
//!
//! ## Why the VAPID keypair is per-user
//!
//! VAPID identifies the application server to the push service. A server-wide
//! keypair would have to live outside any user's encrypted database — a
//! plaintext key file in the data dir. Generating it per user instead keeps it
//! inside that user's SQLCipher DB alongside everything else sensitive, at the
//! cost of nothing that matters: the push service only checks that the JWT is
//! signed by the key matching the `applicationServerKey` the subscription was
//! created with, and subscriptions are per-user anyway.

use crate::{
    error::{AppError, AppResult},
    ApiState,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use finsight_core::repos::{push as push_repo, run};
use finsight_core::settings;
use serde::{Deserialize, Serialize};
use specta::Type;

const VAPID_PRIVATE_PEM: &str = "push.vapid_private_pem";
const VAPID_PUBLIC_B64: &str = "push.vapid_public_b64";

/// The `sub` claim in the VAPID JWT. The spec wants a contact for the push
/// service to reach if this server misbehaves; a self-hosted instance has no
/// public contact, and push services accept any valid mailto.
const VAPID_SUBJECT: &str = "mailto:finsight@localhost";

/// How long the push service should hold an undelivered message. Four hours:
/// long enough to survive a phone being asleep overnight-ish, short enough that
/// a stale "your bill is due" doesn't surface days later.
const PUSH_TTL_SECONDS: u32 = 4 * 60 * 60;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PushStatus {
    /// Base64url VAPID public key for `pushManager.subscribe`.
    pub public_key: String,
    /// How many devices are currently registered for this user.
    pub device_count: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PushDevice {
    pub endpoint: String,
    pub label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// Outcome of a send. Reported rather than swallowed so the Settings screen can
/// tell "no devices registered" apart from "the push service rejected us".
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PushDeliveryReport {
    pub delivered: i64,
    /// Subscriptions the push service reported as permanently gone; these are
    /// deleted rather than retried forever.
    pub expired: i64,
    pub failed: i64,
}

/// The JSON body `push-sw.js` parses. Keep the field names in lockstep with it
/// — the contract is pinned by `ui/src/pwa/push.test.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPayload {
    pub title: String,
    pub body: String,
    /// Route to open on click.
    pub url: String,
    /// Collapse key: a newer message with the same tag replaces the older one
    /// instead of stacking another line on the lock screen.
    pub tag: String,
    /// Lets the worker refresh the app-icon badge while the app is closed —
    /// the one thing the foreground badge hook cannot do.
    pub badge_count: Option<i64>,
}

// ------------------------------------------------------------- VAPID keys ---

/// Fetch this user's VAPID keypair, generating and storing one on first use.
///
/// Returns `(private_pem, public_base64url)`. The public half is derived from
/// the private key rather than trusted from storage on every call, but it IS
/// cached so the client gets a byte-identical key each time — a changed
/// `applicationServerKey` silently invalidates every existing subscription.
async fn vapid_keypair(state: &ApiState) -> AppResult<(String, String)> {
    vapid_keypair_for_db(&state.db).await
}

/// Same, against a bare `Db`. The background sync scheduler holds only a `Db`
/// (no `ApiState`), and it is the one place that fires a push while the app is
/// genuinely closed — which is the whole point of the feature.
async fn vapid_keypair_for_db(db: &finsight_core::Db) -> AppResult<(String, String)> {
    let db = db.clone();

    run(&db, move |conn| {
        let existing_pem: Option<String> = settings::get(conn, VAPID_PRIVATE_PEM)?;
        let existing_pub: Option<String> = settings::get(conn, VAPID_PUBLIC_B64)?;
        if let (Some(pem), Some(pubkey)) = (existing_pem, existing_pub) {
            return Ok((pem, pubkey));
        }

        let (pem, pubkey) = generate_vapid_keypair()
            .map_err(|e| finsight_core::error::CoreError::InvalidState(e))?;
        settings::set(conn, VAPID_PRIVATE_PEM, &pem)?;
        settings::set(conn, VAPID_PUBLIC_B64, &pubkey)?;
        Ok((pem, pubkey))
    })
    .await
    .map_err(AppError::from)
}

/// Generate a P-256 keypair in the two shapes this feature needs: SEC1 PEM for
/// `web-push`'s VAPID signer, and the UNCOMPRESSED public point (65 bytes,
/// `0x04` prefix) base64url-encoded for the browser. Browsers reject the
/// compressed form, so `to_encoded_point(false)` is load-bearing.
fn generate_vapid_keypair() -> Result<(String, String), String> {
    use p256::elliptic_curve::sec1::ToEncodedPoint;
    use p256::pkcs8::LineEnding;
    use p256::SecretKey;

    let secret = SecretKey::random(&mut rand::rngs::OsRng);

    // SEC1 ("EC PRIVATE KEY") PEM specifically: web-push parses the VAPID key
    // with `sec1_decode`, which does not accept a PKCS#8 wrapper.
    let pem = secret
        .to_sec1_pem(LineEnding::LF)
        .map_err(|e| format!("vapid key encode: {e}"))?
        .to_string();

    let point = secret.public_key().to_encoded_point(false);
    let public_b64 = URL_SAFE_NO_PAD.encode(point.as_bytes());
    Ok((pem, public_b64))
}

// -------------------------------------------------------------- commands ---

pub async fn get_push_status(state: &ApiState) -> AppResult<PushStatus> {
    let (_priv, public_key) = vapid_keypair(state).await?;
    let db = (*state.db).clone();
    let device_count = run(&db, move |conn| push_repo::list(conn))
        .await
        .map_err(AppError::from)?
        .len() as i64;
    Ok(PushStatus {
        public_key,
        device_count,
    })
}

pub async fn save_push_subscription(
    state: &ApiState,
    endpoint: String,
    p256dh: String,
    auth: String,
    label: Option<String>,
) -> AppResult<()> {
    if endpoint.trim().is_empty() || p256dh.trim().is_empty() || auth.trim().is_empty() {
        return Err(AppError::new(
            "push.invalid_subscription",
            "The browser returned an incomplete push subscription.",
        ));
    }
    let db = (*state.db).clone();
    run(&db, move |conn| {
        push_repo::upsert(conn, &endpoint, &p256dh, &auth, label.as_deref()).map(|_| ())
    })
    .await
    .map_err(AppError::from)
}

pub async fn delete_push_subscription(state: &ApiState, endpoint: String) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| push_repo::delete_by_endpoint(conn, &endpoint))
        .await
        .map_err(AppError::from)
}

pub async fn list_push_devices(state: &ApiState) -> AppResult<Vec<PushDevice>> {
    let db = (*state.db).clone();
    let rows = run(&db, move |conn| push_repo::list(conn))
        .await
        .map_err(AppError::from)?;
    Ok(rows
        .into_iter()
        .map(|s| PushDevice {
            endpoint: s.endpoint,
            label: s.label,
            created_at: s.created_at.to_rfc3339(),
            last_used_at: s.last_used_at.map(|d| d.to_rfc3339()),
        })
        .collect())
}

/// Send a notification the user asked for, so they can confirm the whole chain
/// (permission, subscription, VAPID signing, push service, worker) actually
/// works on their device. Push cannot be verified from the server side alone —
/// a 201 from the push service only means it accepted the message for delivery.
pub async fn send_test_push(state: &ApiState) -> AppResult<PushDeliveryReport> {
    send_push(
        state,
        PushPayload {
            title: "FinSight".into(),
            body: "Notifications are working on this device.".into(),
            url: "/".into(),
            tag: "finsight-test".into(),
            badge_count: None,
        },
    )
    .await
}

// ------------------------------------------------------------ the sender ---

/// Deliver `payload` to every device this user registered.
///
/// Never returns Err for a per-device failure: one dead subscription must not
/// abort delivery to the user's other devices, so failures are counted in the
/// report instead. A 404/410 from the push service means the subscription is
/// permanently gone, and the row is deleted — otherwise every future send
/// retries a device that will never come back.
pub async fn send_push(state: &ApiState, payload: PushPayload) -> AppResult<PushDeliveryReport> {
    send_push_for_db(&state.db, payload).await
}

/// See `vapid_keypair_for_db` — the `Db`-only entry point, for the background
/// scheduler.
pub async fn send_push_for_db(
    db: &finsight_core::Db,
    payload: PushPayload,
) -> AppResult<PushDeliveryReport> {
    use web_push::{
        ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushMessageBuilder,
    };

    let (private_pem, _public) = vapid_keypair_for_db(db).await?;
    let db = db.clone();
    let subs = run(&db, move |conn| push_repo::list(conn))
        .await
        .map_err(AppError::from)?;

    if subs.is_empty() {
        return Ok(PushDeliveryReport::default());
    }

    let body = serde_json::to_vec(&payload)
        .map_err(|e| AppError::new("push.encode", format!("push payload encode: {e}")))?;

    let client = reqwest::Client::new();
    let mut report = PushDeliveryReport::default();
    let mut expired_endpoints: Vec<String> = Vec::new();

    for sub in subs {
        let info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);

        let signature = match VapidSignatureBuilder::from_pem(private_pem.as_bytes(), &info)
            .and_then(|mut b| {
                b.add_claim("sub", VAPID_SUBJECT);
                b.build()
            }) {
            Ok(sig) => sig,
            Err(e) => {
                tracing::warn!(error = %e, "vapid signing failed");
                report.failed += 1;
                continue;
            }
        };

        let mut builder = WebPushMessageBuilder::new(&info);
        builder.set_payload(ContentEncoding::Aes128Gcm, &body);
        builder.set_vapid_signature(signature);
        // Hold the message for a while if the device is offline, but not so
        // long that a stale financial alert arrives days later.
        builder.set_ttl(PUSH_TTL_SECONDS);

        let message = match builder.build() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = %e, "web push message build failed");
                report.failed += 1;
                continue;
            }
        };

        // web-push's own HTTP clients are optional features pulling in hyper or
        // isahc; the workspace already has reqwest, so send the built message
        // ourselves. This mirrors the crate's `request_builder::build_request`:
        // TTL always, and — when there is a payload — its content encoding,
        // length, an octet-stream content type, and the crypto headers (which
        // is where the VAPID `Authorization` lands).
        let mut req = client
            .post(message.endpoint.to_string())
            .header("ttl", message.ttl.to_string());

        if let Some(payload) = message.payload {
            req = req
                .header("content-encoding", payload.content_encoding.to_str())
                .header("content-length", payload.content.len().to_string())
                .header("content-type", "application/octet-stream");
            for (name, value) in payload.crypto_headers.into_iter() {
                req = req.header(name, value);
            }
            req = req.body(payload.content);
        } else {
            req = req.header("content-length", "0");
        }

        match req.send().await {
            Ok(res) if res.status().is_success() => {
                report.delivered += 1;
                let endpoint = sub.endpoint.clone();
                let db2 = db.clone();
                // Best-effort liveness stamp; a failure here is not a delivery
                // failure and must not be reported as one.
                let _ = run(&db2, move |conn| push_repo::mark_used(conn, &endpoint)).await;
            }
            Ok(res) if res.status().as_u16() == 404 || res.status().as_u16() == 410 => {
                report.expired += 1;
                expired_endpoints.push(sub.endpoint.clone());
            }
            Ok(res) => {
                tracing::warn!(status = %res.status(), "push service rejected a notification");
                report.failed += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "push delivery failed");
                report.failed += 1;
            }
        }
    }

    for endpoint in expired_endpoints {
        let db2 = db.clone();
        let _ = run(&db2, move |conn| push_repo::delete_by_endpoint(conn, &endpoint)).await;
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("push.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    /// Browsers reject a compressed point for `applicationServerKey`, and
    /// web-push's signer rejects a PKCS#8 wrapper — both halves have to come out
    /// in exactly one shape, so pin both.
    #[test]
    fn generated_keypair_is_sec1_pem_and_an_uncompressed_p256_point() {
        let (pem, public_b64) = generate_vapid_keypair().unwrap();

        assert!(
            pem.starts_with("-----BEGIN EC PRIVATE KEY-----"),
            "must be SEC1, not PKCS#8: {pem}"
        );

        let bytes = URL_SAFE_NO_PAD.decode(&public_b64).expect("base64url");
        assert_eq!(bytes.len(), 65, "uncompressed P-256 point is 65 bytes");
        assert_eq!(bytes[0], 0x04, "0x04 prefix marks the uncompressed form");
        // Must survive the browser's base64url decoder untouched.
        assert!(!public_b64.contains('+') && !public_b64.contains('/') && !public_b64.contains('='));
    }

    #[test]
    fn each_generation_produces_a_distinct_key() {
        let (_, a) = generate_vapid_keypair().unwrap();
        let (_, b) = generate_vapid_keypair().unwrap();
        assert_ne!(a, b);
    }

    /// The key must be stable: `applicationServerKey` is baked into every
    /// subscription, so regenerating it would silently orphan every device.
    #[tokio::test]
    async fn vapid_keypair_is_generated_once_and_then_reused() {
        let (_d, db) = fresh_db();
        let first = vapid_keypair_for_db(&db).await.unwrap();
        let second = vapid_keypair_for_db(&db).await.unwrap();
        assert_eq!(first, second);
    }

    /// No devices must be a cheap no-op, not an error and not a network call —
    /// this path runs after every background sync for users who never opted in.
    #[tokio::test]
    async fn sending_with_no_registered_devices_reports_nothing_and_touches_no_network() {
        let (_d, db) = fresh_db();
        let report = send_push_for_db(
            &db,
            PushPayload {
                title: "t".into(),
                body: "b".into(),
                url: "/".into(),
                tag: "tag".into(),
                badge_count: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(report.delivered, 0);
        assert_eq!(report.expired, 0);
        assert_eq!(report.failed, 0);
    }

    /// The payload is the contract with push-sw.js; the worker reads camelCase.
    #[test]
    fn payload_serializes_with_the_field_names_the_service_worker_reads() {
        let json = serde_json::to_value(PushPayload {
            title: "New activity".into(),
            body: "2 new transactions".into(),
            url: "/inbox".into(),
            tag: "finsight-sync".into(),
            badge_count: Some(7),
        })
        .unwrap();

        assert_eq!(json["title"], "New activity");
        assert_eq!(json["body"], "2 new transactions");
        assert_eq!(json["url"], "/inbox");
        assert_eq!(json["tag"], "finsight-sync");
        assert_eq!(json["badgeCount"], 7);
    }
}
