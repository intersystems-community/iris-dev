// Unit tests for scm.rs internal logic (no IRIS needed).
// Tests KNOWN_MENU_ITEMS and ScmParams deserialization.

use iris_dev_core::tools::scm::{ScmParams, KNOWN_MENU_ITEMS};

#[test]
fn test_known_menu_items_not_empty() {
    assert!(
        !KNOWN_MENU_ITEMS.is_empty(),
        "KNOWN_MENU_ITEMS must have entries"
    );
    assert!(
        KNOWN_MENU_ITEMS.contains(&"CheckOut"),
        "must contain CheckOut"
    );
    assert!(
        KNOWN_MENU_ITEMS.contains(&"CheckIn"),
        "must contain CheckIn"
    );
}

#[test]
fn test_known_menu_items_count() {
    assert!(
        KNOWN_MENU_ITEMS.len() >= 5,
        "expect at least 5 known menu items, got {}",
        KNOWN_MENU_ITEMS.len()
    );
}

#[test]
fn test_known_menu_items_contains_standard_actions() {
    let expected = ["CheckOut", "UndoCheckOut", "CheckIn", "GetLatest", "Status"];
    for item in &expected {
        assert!(
            KNOWN_MENU_ITEMS.contains(item),
            "KNOWN_MENU_ITEMS missing: {}",
            item
        );
    }
}

#[test]
fn test_scm_params_action_required() {
    let p: ScmParams = serde_json::from_str(r#"{"action": "status"}"#).unwrap();
    assert_eq!(p.action, "status");
    assert_eq!(p.namespace, "USER"); // default_namespace
    assert!(p.document.is_none());
    assert!(p.action_id.is_none());
    assert!(p.answer.is_none());
    assert!(p.elicitation_id.is_none());
}

#[test]
fn test_scm_params_full() {
    let p: ScmParams = serde_json::from_str(
        r#"{
            "action": "execute",
            "document": "MyClass.cls",
            "action_id": "CheckOut",
            "namespace": "MYNAMESPACE"
        }"#,
    )
    .unwrap();
    assert_eq!(p.action, "execute");
    assert_eq!(p.document.as_deref(), Some("MyClass.cls"));
    assert_eq!(p.action_id.as_deref(), Some("CheckOut"));
    assert_eq!(p.namespace, "MYNAMESPACE");
}

#[test]
fn test_scm_params_with_elicitation_fields() {
    let p: ScmParams = serde_json::from_str(
        r#"{
            "action": "execute",
            "answer": "yes",
            "elicitation_id": "eid-abc123"
        }"#,
    )
    .unwrap();
    assert_eq!(p.action, "execute");
    assert_eq!(p.answer.as_deref(), Some("yes"));
    assert_eq!(p.elicitation_id.as_deref(), Some("eid-abc123"));
    assert_eq!(p.namespace, "USER");
}

#[test]
fn test_scm_params_missing_action_fails() {
    let result: Result<ScmParams, _> = serde_json::from_str(r#"{"document": "Foo.cls"}"#);
    assert!(
        result.is_err(),
        "action is required — should fail without it"
    );
}
