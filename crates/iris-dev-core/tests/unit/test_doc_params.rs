//! T015: Unit tests for IrisDocParams elicitation fields.

use iris_dev_core::tools::{IrisDocParams, DocMode};

#[test]
fn doc_params_elicitation_fields() {
    let p: IrisDocParams = serde_json::from_str(r#"{
        "mode": "put",
        "name": "MyApp.Patient.cls",
        "content": "Class MyApp.Patient {}",
        "elicitation_id": "abc-123",
        "elicitation_answer": "yes"
    }"#).unwrap();
    assert!(matches!(p.mode, DocMode::Put));
    assert_eq!(p.elicitation_id.as_deref(), Some("abc-123"));
    assert_eq!(p.elicitation_answer.as_deref(), Some("yes"));
}

#[test]
fn doc_params_no_elicitation_defaults_to_none() {
    let p: IrisDocParams = serde_json::from_str(r#"{"mode":"get","name":"MyApp.Patient.cls"}"#).unwrap();
    assert!(p.elicitation_id.is_none());
    assert!(p.elicitation_answer.is_none());
}
