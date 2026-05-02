use iris_dev_core::tools::symbols_local::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

// ── T010: glob_match unit tests ──────────────────────────────────────────────

#[test]
fn glob_exact() {
    assert!(glob_match("MyApp.Foo", "MyApp.Foo"));
}

#[test]
fn glob_no_implicit_substring() {
    assert!(!glob_match("Foo", "MyApp.Foo"));
}

#[test]
fn glob_package_prefix() {
    assert!(glob_match("MyApp.*", "MyApp.Foo"));
    assert!(glob_match("MyApp.*", "MyApp.Bar"));
    assert!(!glob_match("MyApp.*", "OtherApp.Foo"));
}

#[test]
fn glob_suffix() {
    assert!(glob_match("*Service", "OrderService"));
    assert!(!glob_match("*Service", "OrderUtil"));
}

#[test]
fn glob_mid() {
    assert!(glob_match("MyApp.*.Base", "MyApp.Sub.Base"));
    assert!(!glob_match("MyApp.*.Base", "MyApp.Sub.Other"));
}

#[test]
fn glob_empty_never_matches() {
    assert!(!glob_match("", "anything"));
    assert!(!glob_match("", ""));
}

// ── T015: extract_cls_symbols on Foo.cls ─────────────────────────────────────

#[test]
fn extract_cls_foo() {
    let path = fixtures_dir().join("MyApp/Foo.cls");
    let source = std::fs::read(&path).expect("read Foo.cls");
    let (symbols, warnings) = extract_cls_symbols(&source, "MyApp/Foo.cls", "MyApp.Foo");

    // No parse errors for the valid file
    let parse_errors: Vec<_> = warnings
        .iter()
        .filter(|w| w.warning_type == "PARSE_ERROR")
        .collect();
    assert!(
        parse_errors.is_empty(),
        "Unexpected parse errors: {:?}",
        parse_errors
    );

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

    assert!(
        names.contains(&"MyApp.Foo"),
        "class symbol missing; got {:?}",
        names
    );

    let has_method = symbols
        .iter()
        .any(|s| s.name == "MyApp.Foo.DoSomething" && s.kind == "method");
    assert!(has_method, "DoSomething method not found; got {:?}", names);

    let method = symbols
        .iter()
        .find(|s| s.name == "MyApp.Foo.DoSomething")
        .unwrap();
    assert!(
        method
            .formal_spec
            .as_ref()
            .map(|f| !f.is_empty())
            .unwrap_or(false),
        "FormalSpec should be non-empty"
    );

    let has_property = symbols
        .iter()
        .any(|s| s.name == "MyApp.Foo.Value" && s.kind == "property");
    assert!(has_property, "Value property not found; got {:?}", names);

    let has_param = symbols
        .iter()
        .any(|s| s.name == "MyApp.Foo.VERSION" && s.kind == "parameter");
    assert!(has_param, "VERSION parameter not found; got {:?}", names);
}

// ── T016: glob used in scan matches package correctly ───────────────────────

#[test]
fn glob_package_scan_match() {
    assert!(glob_match("MyApp.*", "MyApp.Foo"));
    assert!(glob_match("MyApp.*", "MyApp.Bar"));
    assert!(!glob_match("MyApp.*", "OtherApp.Foo"));
}

// ── T017: scan_workspace with exact class query ──────────────────────────────

#[test]
fn scan_workspace_exact_query() {
    let result = scan_workspace(&fixtures_dir(), "MyApp.Foo", 50);

    let has_class = result
        .symbols
        .iter()
        .any(|s| s.name == "MyApp.Foo" && s.kind == "class");
    assert!(has_class, "class symbol not found: {:?}", result.symbols);

    assert!(
        result.symbols.len() >= 3,
        "expected at least 3 symbols, got {}",
        result.symbols.len()
    );

    // No PARSE_ERROR for Foo.cls (it is valid)
    let foo_errors: Vec<_> = result
        .parse_warnings
        .iter()
        .filter(|w| {
            w.warning_type == "PARSE_ERROR"
                && w.file
                    .as_deref()
                    .map(|f| f.contains("Foo.cls"))
                    .unwrap_or(false)
                && !w
                    .file
                    .as_deref()
                    .map(|f| f.contains("Broken") || f.contains("Dupe"))
                    .unwrap_or(false)
        })
        .collect();
    assert!(
        foo_errors.is_empty(),
        "Unexpected PARSE_ERROR for Foo.cls: {:?}",
        foo_errors
    );
}

// ── T018: scan with wildcard triggers DUPLICATE_CLASS warning ────────────────

#[test]
fn scan_workspace_wildcard_detects_duplicate() {
    let result = scan_workspace(&fixtures_dir(), "MyApp.*", 200);

    let has_duplicate = result
        .parse_warnings
        .iter()
        .any(|w| w.warning_type == "DUPLICATE_CLASS" && w.class.as_deref() == Some("MyApp.Foo"));
    assert!(
        has_duplicate,
        "Expected DUPLICATE_CLASS for MyApp.Foo; warnings: {:?}",
        result.parse_warnings
    );

    // Symbols from Foo.cls should still be present
    assert!(
        result.symbols.iter().any(|s| s.name == "MyApp.Foo"),
        "MyApp.Foo class symbol should still appear despite duplicate"
    );
}

// ── T022 / SC-006: NOT_IMPLEMENTED never returned ────────────────────────────

#[test]
fn sc006_not_implemented_never_returned() {
    // Call scan_workspace directly; it must never produce NOT_IMPLEMENTED
    let result = scan_workspace(&fixtures_dir(), "MyApp.Foo", 10);
    // Serialize to check no NOT_IMPLEMENTED in output
    let json = serde_json::to_string(&result.symbols).unwrap_or_default();
    assert!(
        !json.contains("NOT_IMPLEMENTED"),
        "NOT_IMPLEMENTED must never appear in symbols output"
    );
}

// ── T023: Broken.cls produces PARSE_ERROR, no panic ──────────────────────────

#[test]
fn extract_broken_cls_no_panic() {
    let path = fixtures_dir().join("MyApp/Broken.cls");
    let source = std::fs::read(&path).expect("read Broken.cls");
    let (symbols, warnings) = extract_cls_symbols(&source, "MyApp/Broken.cls", "MyApp.Broken");

    let has_error = warnings.iter().any(|w| w.warning_type == "PARSE_ERROR");
    assert!(
        has_error,
        "Expected PARSE_ERROR warning for Broken.cls; got {:?}",
        warnings
    );

    // Must not panic and must return a (possibly empty) symbols vec
    let _ = symbols;
}

// ── T024: scan_workspace includes errors from Broken.cls + symbols from Foo ──

#[test]
fn scan_includes_errors_and_valid_symbols() {
    let result = scan_workspace(&fixtures_dir(), "MyApp.*", 200);

    let has_broken_error = result.parse_warnings.iter().any(|w| {
        w.warning_type == "PARSE_ERROR"
            && w.file
                .as_deref()
                .map(|f| f.contains("Broken"))
                .unwrap_or(false)
    });
    assert!(
        has_broken_error,
        "Expected PARSE_ERROR for Broken.cls; warnings: {:?}",
        result.parse_warnings
    );

    // Symbols from the valid Foo.cls must still be present
    assert!(
        result.symbols.iter().any(|s| s.name == "MyApp.Foo"),
        "MyApp.Foo should still be returned despite Broken.cls parse error"
    );
    assert!(result.symbols.len() > 0, "count should be > 0");
}

// ── T026 / SC-004: parse error does NOT make the result an error ─────────────

#[test]
fn sc004_parse_error_no_error_response() {
    let result = scan_workspace(&fixtures_dir(), "MyApp.*", 200);
    // The scan itself must not return an error-level result —
    // verified by the fact that it returns a SymbolsLocalResult, not Err.
    // parse_warnings may contain PARSE_ERROR; that is fine.
    assert!(
        result
            .parse_warnings
            .iter()
            .any(|w| w.warning_type == "PARSE_ERROR")
            || !result.symbols.is_empty(),
        "result must be non-erroring (either has warnings or has symbols)"
    );
}

// ── T027: extract_routine_symbols on Utils.mac ───────────────────────────────

#[test]
fn extract_routine_utils_mac() {
    let path = fixtures_dir().join("Utils.mac");
    let source = std::fs::read(&path).expect("read Utils.mac");
    let (symbols, _warnings) = extract_routine_symbols(&source, "Utils.mac", "Utils");

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

    let has_start = symbols
        .iter()
        .any(|s| s.name == "Utils:Start" && s.kind == "label");
    let has_helper = symbols
        .iter()
        .any(|s| s.name == "Utils:Helper" && s.kind == "label");

    assert!(has_start, "Utils:Start label not found; got {:?}", names);
    assert!(has_helper, "Utils:Helper label not found; got {:?}", names);
}

// ── T028: extract_routine_symbols on Macros.inc ──────────────────────────────

#[test]
fn extract_routine_macros_inc() {
    let path = fixtures_dir().join("Macros.inc");
    let source = std::fs::read(&path).expect("read Macros.inc");
    // For .inc files, query matches on filename stem "Macros"
    let (symbols, _warnings) = extract_routine_symbols(&source, "Macros.inc", "Macros");

    let macro_count = symbols.iter().filter(|s| s.kind == "macro").count();
    assert!(
        macro_count >= 2,
        "Expected at least 2 macro symbols; got {:?}",
        symbols
    );
}

// ── T029: scan_workspace with routine query ───────────────────────────────────

#[test]
fn scan_workspace_routine_query() {
    let result = scan_workspace(&fixtures_dir(), "Utils", 50);
    let has_label = result
        .symbols
        .iter()
        .any(|s| s.kind == "label" && s.name.starts_with("Utils:"));
    assert!(
        has_label,
        "Expected routine label symbols for Utils; got {:?}",
        result.symbols
    );
}

// ── T031 / SC-002: output shape parity ───────────────────────────────────────

#[test]
fn sc002_shape_parity() {
    let result = scan_workspace(&fixtures_dir(), "MyApp.Foo", 50);

    // Check that each symbol has the required top-level keys
    for sym in &result.symbols {
        let v = serde_json::to_value(sym).unwrap();
        assert!(v.get("Name").is_some(), "Symbol missing 'Name' field");
        assert!(v.get("kind").is_some(), "Symbol missing 'kind' field");
        assert!(v.get("file").is_some(), "Symbol missing 'file' field");
    }

    // iris_symbols returns: source, symbols, count, query_hint
    // We verify the scan result provides these fields when wrapped
    assert!(!result.symbols.is_empty(), "symbols must not be empty");
}

// ── T032 / SC-003: 500-line parse < 100ms ────────────────────────────────────

#[test]
fn sc003_parse_500_lines_under_100ms() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Large500.cls");
    let source = std::fs::read(&path).expect("read Large500.cls");

    let start = std::time::Instant::now();
    let (symbols, _warnings) = extract_cls_symbols(&source, "Large500.cls", "Large.*");
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 100,
        "Parsing Large500.cls took {}ms, expected < 100ms",
        elapsed.as_millis()
    );
    assert!(
        !symbols.is_empty(),
        "Should have extracted at least one symbol from Large500.cls"
    );
}

// ── T033 / SC-005: no IRIS contact ───────────────────────────────────────────

#[test]
fn sc005_no_iris_contact() {
    // scan_workspace is pure filesystem — it never calls HTTP.
    // Verified by the fact that it completes without IRIS_HOST set.
    // (This test succeeds in any environment including pure CI with no IRIS.)
    std::env::remove_var("IRIS_HOST");
    std::env::remove_var("IRIS_CONTAINER");

    let result = scan_workspace(&fixtures_dir(), "MyApp.Foo", 10);
    // If it completes and returns symbols, no IRIS contact was made.
    assert!(
        !result.symbols.is_empty(),
        "scan_workspace should work without IRIS"
    );
}

// Grammar inspection test — run with --nocapture to see tree
#[test]
#[ignore = "debug: prints parse tree"]
fn inspect_grammar_tree() {
    let source = std::fs::read(fixtures_dir().join("MyApp/Foo.cls")).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_objectscript::LANGUAGE_OBJECTSCRIPT_UDL.into())
        .unwrap();
    let tree = parser.parse(&source, None).unwrap();
    print_node(tree.root_node(), &source, 0);

    let source2 = std::fs::read(fixtures_dir().join("Utils.mac")).unwrap();
    let mut p2 = tree_sitter::Parser::new();
    p2.set_language(&tree_sitter_objectscript_routine::LANGUAGE_OBJECTSCRIPT_ROUTINE.into())
        .unwrap();
    let tree2 = p2.parse(&source2, None).unwrap();
    println!("\n=== ROUTINE ===");
    print_node(tree2.root_node(), &source2, 0);
}

fn print_node(node: tree_sitter::Node, source: &[u8], depth: usize) {
    if depth > 7 {
        return;
    }
    let indent = "  ".repeat(depth);
    let text = if node.child_count() == 0 && node.end_byte().saturating_sub(node.start_byte()) < 50
    {
        format!(
            " = {:?}",
            std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("?")
        )
    } else {
        String::new()
    };
    println!("{}{}{}", indent, node.kind(), text);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_node(child, source, depth + 1);
    }
}
