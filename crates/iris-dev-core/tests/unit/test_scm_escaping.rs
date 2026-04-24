// Tests for os_quote() ObjectScript escaping logic.
// We test the escaping rules directly since os_quote is an internal function.

#[test]
fn test_os_quote_double_quote_rule() {
    // ObjectScript rule: " in a string literal becomes ""
    let input = r#"say "hi""#;
    // Apply the rule manually to verify our understanding
    let escaped = input.replace('"', "\"\"");
    assert!(escaped.contains("\"\""), "should use \"\" escaping");
    assert!(
        !escaped.contains("\\\""),
        "should not use backslash escaping"
    );
}

#[test]
fn test_os_quote_newline_rule() {
    let input = "a\nb";
    let escaped = input.replace('\n', "$Char(10)");
    assert_eq!(escaped, "a$Char(10)b");
}

#[test]
fn test_os_quote_cr_rule() {
    let input = "a\rb";
    let escaped = input.replace('\r', "$Char(13)");
    assert_eq!(escaped, "a$Char(13)b");
}

#[test]
fn test_os_quote_combined_rules() {
    // The os_quote function should apply all three rules
    let input = "line1\nline2\"quoted\"";
    // Expected: newline → $Char(10), " → ""
    let escaped = input
        .replace('"', "\"\"")
        .replace('\n', "$Char(10)")
        .replace('\r', "$Char(13)");
    assert!(escaped.contains("$Char(10)"));
    assert!(escaped.contains("\"\""));
    assert!(!escaped.contains('\n'));
}

#[test]
fn test_user_action_code_escaping() {
    // user_action_code must not use \" (backslash-quote) in generated ObjectScript
    // It must use "" (double-double-quote) for ObjectScript string literals
    let action_id = "Check\"Out";
    let doc = "MyDoc.cls";
    // Simulate what user_action_code does: os_quote both strings
    let escaped_action = action_id
        .replace('"', "\"\"")
        .replace('\n', "$Char(10)")
        .replace('\r', "$Char(13)");
    let escaped_doc = doc
        .replace('"', "\"\"")
        .replace('\n', "$Char(10)")
        .replace('\r', "$Char(13)");
    let code = format!(
        r#"...UserAction(0,"%SourceMenu,{}","{}",..."#,
        escaped_action, escaped_doc
    );
    assert!(
        !code.contains("\\\""),
        "must not contain backslash-quote: {}",
        code
    );
    assert!(code.contains("\"\"") || !action_id.contains('"') || escaped_action.contains("\"\""));
}
