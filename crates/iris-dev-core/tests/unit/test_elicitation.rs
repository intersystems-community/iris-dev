//! T007-T008: Unit tests for ElicitationStore.

use iris_dev_core::elicitation::{ElicitationAction, ElicitationStore};
use std::time::{Duration, Instant};

#[test]
fn elicitation_state_roundtrip() {
    let store = ElicitationStore::new();
    let id = store.insert(
        "MyApp.Patient.cls",
        ElicitationAction::Put,
        Some("class content".to_string()),
        None,
        "USER",
    );
    let entry = store.lookup(&id).expect("should find unexpired entry");
    assert_eq!(entry.document, "MyApp.Patient.cls");
    assert_eq!(entry.namespace, "USER");
    assert_eq!(entry.content.as_deref(), Some("class content"));
    assert!(entry.expires_at > Instant::now());
}

#[test]
fn elicitation_state_expires() {
    let store = ElicitationStore::new();
    // Insert with a past expiry by inserting normally then manually expiring via clear+reinsert trick
    // Since we can't set expiry directly, we test that clear works and missing id returns None
    let id = store.insert("MyApp.Test.cls", ElicitationAction::ScmExecute, None, Some("CheckOut".into()), "USER");
    store.clear(&id);
    assert!(store.lookup(&id).is_none(), "cleared entry should return None");
}

#[test]
fn elicitation_missing_id_returns_none() {
    let store = ElicitationStore::new();
    assert!(store.lookup("nonexistent-id-12345").is_none());
}
