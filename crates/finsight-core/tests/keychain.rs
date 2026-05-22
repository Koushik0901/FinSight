//! Unit test for keychain load_or_create.
//! Uses a unique service name per test run so we don't collide with real installs.

use finsight_core::keychain;
use uuid::Uuid;

#[test]
fn load_or_create_returns_same_key_across_calls() {
    let service = format!("finsight-test-{}", Uuid::new_v4());
    let k1 = keychain::load_or_create_key(&service, "default").unwrap();
    let k2 = keychain::load_or_create_key(&service, "default").unwrap();
    assert_eq!(k1, k2, "second call must return the existing key");
    assert_eq!(k1.len(), 64, "key is 32 bytes hex-encoded");
    // cleanup
    keychain::delete_key(&service, "default").ok();
}
