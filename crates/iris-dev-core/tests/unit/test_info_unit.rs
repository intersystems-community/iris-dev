// Unit tests for tools/info.rs — struct deserialization and pure logic.
// No IRIS connection required.

use iris_dev_core::tools::info::{DebugParams, GenerateParams, InfoParams, MacroParams};

// ── InfoParams ───────────────────────────────────────────────────────────────

#[test]
fn test_info_params_what_field_metadata() {
    let p: InfoParams = serde_json::from_str(r#"{"what": "metadata"}"#).unwrap();
    assert_eq!(p.what, "metadata");
}

#[test]
fn test_info_params_namespace_default() {
    let p: InfoParams = serde_json::from_str(r#"{"what": "documents"}"#).unwrap();
    assert_eq!(p.namespace, "USER");
}

#[test]
fn test_info_params_namespace_override() {
    let p: InfoParams =
        serde_json::from_str(r#"{"what": "namespace", "namespace": "MYNS"}"#).unwrap();
    assert_eq!(p.namespace, "MYNS");
}

#[test]
fn test_info_params_doc_type_optional() {
    let p: InfoParams = serde_json::from_str(r#"{"what": "documents"}"#).unwrap();
    assert!(p.doc_type.is_none(), "doc_type should be None when absent");
}

#[test]
fn test_info_params_doc_type_present() {
    let p: InfoParams =
        serde_json::from_str(r#"{"what": "documents", "doc_type": "CLS"}"#).unwrap();
    assert_eq!(p.doc_type.as_deref(), Some("CLS"));
}

#[test]
fn test_info_params_name_optional() {
    let p: InfoParams = serde_json::from_str(r#"{"what": "sa_schema"}"#).unwrap();
    assert!(p.name.is_none());
}

#[test]
fn test_info_params_name_present() {
    let p: InfoParams = serde_json::from_str(r#"{"what": "sa_schema", "name": "MyCube"}"#).unwrap();
    assert_eq!(p.name.as_deref(), Some("MyCube"));
}

#[test]
fn test_info_params_all_what_variants_deserialize() {
    for what in &[
        "metadata",
        "namespace",
        "documents",
        "modified",
        "jobs",
        "csp_apps",
        "csp_debug",
        "sa_schema",
    ] {
        let json = format!(r#"{{"what": "{}"}}"#, what);
        let p: InfoParams = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to deserialize what={}: {}", what, e));
        assert_eq!(&p.what, what);
    }
}

#[test]
fn test_info_params_debug_format() {
    let p: InfoParams = serde_json::from_str(r#"{"what": "jobs"}"#).unwrap();
    let debug = format!("{:?}", p);
    assert!(debug.contains("InfoParams"));
    assert!(debug.contains("jobs"));
}

// ── MacroParams ──────────────────────────────────────────────────────────────

#[test]
fn test_macro_params_action_list() {
    let p: MacroParams = serde_json::from_str(r#"{"action": "list"}"#).unwrap();
    assert_eq!(p.action, "list");
}

#[test]
fn test_macro_params_action_signature() {
    let p: MacroParams = serde_json::from_str(r#"{"action": "signature"}"#).unwrap();
    assert_eq!(p.action, "signature");
}

#[test]
fn test_macro_params_namespace_default() {
    let p: MacroParams = serde_json::from_str(r#"{"action": "list"}"#).unwrap();
    assert_eq!(p.namespace, "USER");
}

#[test]
fn test_macro_params_namespace_override() {
    let p: MacroParams =
        serde_json::from_str(r#"{"action": "list", "namespace": "PROD"}"#).unwrap();
    assert_eq!(p.namespace, "PROD");
}

#[test]
fn test_macro_params_name_optional() {
    let p: MacroParams = serde_json::from_str(r#"{"action": "list"}"#).unwrap();
    assert!(p.name.is_none());
}

#[test]
fn test_macro_params_name_present() {
    let p: MacroParams =
        serde_json::from_str(r#"{"action": "definition", "name": "$$MyMacro"}"#).unwrap();
    assert_eq!(p.name.as_deref(), Some("$$MyMacro"));
}

#[test]
fn test_macro_params_args_default_empty() {
    let p: MacroParams = serde_json::from_str(r#"{"action": "expand"}"#).unwrap();
    assert!(p.args.is_empty(), "args should default to empty vec");
}

#[test]
fn test_macro_params_args_present() {
    let p: MacroParams =
        serde_json::from_str(r#"{"action": "expand", "args": ["a", "b"]}"#).unwrap();
    assert_eq!(p.args, vec!["a", "b"]);
}

#[test]
fn test_macro_params_all_actions_deserialize() {
    for action in &["list", "signature", "location", "definition", "expand"] {
        let json = format!(r#"{{"action": "{}"}}"#, action);
        let p: MacroParams = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed for action={}: {}", action, e));
        assert_eq!(&p.action, action);
    }
}

// ── DebugParams ──────────────────────────────────────────────────────────────

#[test]
fn test_debug_params_action_map_int() {
    let p: DebugParams = serde_json::from_str(r#"{"action": "map_int"}"#).unwrap();
    assert_eq!(p.action, "map_int");
}

#[test]
fn test_debug_params_namespace_default() {
    let p: DebugParams = serde_json::from_str(r#"{"action": "error_logs"}"#).unwrap();
    assert_eq!(p.namespace, "USER");
}

#[test]
fn test_debug_params_limit_default() {
    let p: DebugParams = serde_json::from_str(r#"{"action": "capture"}"#).unwrap();
    assert_eq!(p.limit, 20, "default limit should be 20");
}

#[test]
fn test_debug_params_limit_override() {
    let p: DebugParams = serde_json::from_str(r#"{"action": "error_logs", "limit": 50}"#).unwrap();
    assert_eq!(p.limit, 50);
}

#[test]
fn test_debug_params_error_string_optional() {
    let p: DebugParams = serde_json::from_str(r#"{"action": "map_int"}"#).unwrap();
    assert!(p.error_string.is_none());
}

#[test]
fn test_debug_params_error_string_present() {
    let p: DebugParams = serde_json::from_str(
        r#"{"action": "map_int", "error_string": "<UNDEFINED>x+3^MyApp.Foo.1"}"#,
    )
    .unwrap();
    assert_eq!(
        p.error_string.as_deref(),
        Some("<UNDEFINED>x+3^MyApp.Foo.1")
    );
}

#[test]
fn test_debug_params_class_name_optional() {
    let p: DebugParams = serde_json::from_str(r#"{"action": "source_map"}"#).unwrap();
    assert!(p.class_name.is_none());
}

#[test]
fn test_debug_params_class_name_present() {
    let p: DebugParams =
        serde_json::from_str(r#"{"action": "source_map", "class_name": "MyApp.Foo"}"#).unwrap();
    assert_eq!(p.class_name.as_deref(), Some("MyApp.Foo"));
}

#[test]
fn test_debug_params_all_actions_deserialize() {
    for action in &["map_int", "error_logs", "capture", "source_map"] {
        let json = format!(r#"{{"action": "{}"}}"#, action);
        let p: DebugParams = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed for action={}: {}", action, e));
        assert_eq!(&p.action, action);
    }
}

// ── GenerateParams ───────────────────────────────────────────────────────────

#[test]
fn test_generate_params_description_required() {
    let p: GenerateParams =
        serde_json::from_str(r#"{"description": "a Patient class with Name"}"#).unwrap();
    assert_eq!(p.description, "a Patient class with Name");
}

#[test]
fn test_generate_params_gen_type_default_class() {
    let p: GenerateParams = serde_json::from_str(r#"{"description": "anything"}"#).unwrap();
    assert_eq!(p.gen_type, "class", "gen_type should default to 'class'");
}

#[test]
fn test_generate_params_gen_type_test() {
    let p: GenerateParams =
        serde_json::from_str(r#"{"description": "tests", "gen_type": "test"}"#).unwrap();
    assert_eq!(p.gen_type, "test");
}

#[test]
fn test_generate_params_namespace_default() {
    let p: GenerateParams = serde_json::from_str(r#"{"description": "something"}"#).unwrap();
    assert_eq!(p.namespace, "USER");
}

#[test]
fn test_generate_params_namespace_override() {
    let p: GenerateParams =
        serde_json::from_str(r#"{"description": "x", "namespace": "PROD"}"#).unwrap();
    assert_eq!(p.namespace, "PROD");
}

#[test]
fn test_generate_params_class_name_optional() {
    let p: GenerateParams = serde_json::from_str(r#"{"description": "x"}"#).unwrap();
    assert!(p.class_name.is_none());
}

#[test]
fn test_generate_params_class_name_present() {
    let p: GenerateParams = serde_json::from_str(
        r#"{"description": "tests for Foo", "gen_type": "test", "class_name": "MyApp.Foo"}"#,
    )
    .unwrap();
    assert_eq!(p.class_name.as_deref(), Some("MyApp.Foo"));
}

#[test]
fn test_generate_params_debug_format() {
    let p: GenerateParams = serde_json::from_str(r#"{"description": "hello"}"#).unwrap();
    let debug = format!("{:?}", p);
    assert!(debug.contains("GenerateParams"));
}
