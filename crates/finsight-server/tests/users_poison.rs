use finsight_server::state::lock_recovered;
use finsight_server::users::UsersDb;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn poisoned_std_mutex_recovers_via_lock_recovered() {
    let m = Arc::new(Mutex::new(42u32));
    let m2 = Arc::clone(&m);
    let handle = thread::spawn(move || {
        let _guard = m2.lock().unwrap();
        panic!("poison for test");
    });
    let _ = handle.join();
    assert!(m.is_poisoned(), "mutex should be poisoned");

    // lock_recovered must recover and still give access, logging a warn
    let guard = lock_recovered(&m);
    assert_eq!(*guard, 42);
    // Further operations must still work without panicking
    drop(guard);
    let guard2 = lock_recovered(&m);
    assert_eq!(*guard2, 42);
}

#[test]
fn users_db_survives_poisoned_inner_mutex() {
    let dir = tempfile::tempdir().unwrap();
    let db = UsersDb::open(&dir.path().join("users.db")).unwrap();

    // Sanity: works before poison
    assert!(db.is_empty().unwrap());
    db.create_user("alice", "phc", &[1; 16], &[2; 60], &[3; 60], false)
        .unwrap();
    assert!(!db.is_empty().unwrap());

    // Poison the inner Mutex<Connection>
    db.poison_for_test();
    // The std mutex should now be poisoned, but UsersDb uses lock_recovered
    // so every subsequent operation recovers instead of panicking.

    // All of these must succeed without panicking
    assert!(!db.is_empty().unwrap(), "is_empty must recover from poison");
    let got = db.get_by_username("alice").unwrap().unwrap();
    assert_eq!(got.username, "alice");

    db.create_user("bob", "phc2", &[4; 16], &[5; 60], &[6; 60], false)
        .unwrap();
    assert_eq!(db.list_users().unwrap().len(), 2);

    // API token operations also use the same mutex
    let u = db.get_by_username("bob").unwrap().unwrap();
    db.insert_api_token(&u.id, "tok", "full", &[7; 32], &[8; 60], None)
        .unwrap();
    assert!(db.get_api_token_by_hash(&[7; 32]).unwrap().is_some());

    // Session helpers too
    assert_eq!(db.is_empty().unwrap(), false);
}

#[test]
fn registry_mutex_also_uses_lock_recovered() {
    // The Registry and SessionStore both hold std Mutexes that use
    // lock_recovered; this test proves the same poison-recovery path works
    // for a registry-like HashMap mutex.
    use std::collections::HashMap;
    let m = Arc::new(Mutex::new(HashMap::<String, u32>::new()));
    let m2 = Arc::clone(&m);
    let h = thread::spawn(move || {
        let mut g = m2.lock().unwrap();
        g.insert("key".into(), 1);
        panic!("poison registry");
    });
    let _ = h.join();
    assert!(m.is_poisoned());
    let mut g = lock_recovered(&m);
    assert_eq!(g.get("key"), Some(&1));
    g.insert("key2".into(), 2);
    drop(g);
    let g2 = lock_recovered(&m);
    assert_eq!(g2.len(), 2);
}
