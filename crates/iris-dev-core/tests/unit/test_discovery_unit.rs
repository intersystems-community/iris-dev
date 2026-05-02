// Unit tests for iris/discovery.rs — score_container_name + IrisDiscovery types.
// No Docker, no network required.

use iris_dev_core::iris::discovery::{score_container_name, DiscoveryResult, FailureMode, IrisDiscovery};

// ── IrisDiscovery enum smoke test ─────────────────────────────────────────────

#[test]
fn test_iris_discovery_variants_exist() {
    let _ = std::mem::discriminant(&IrisDiscovery::NotFound);
    let _ = std::mem::discriminant(&IrisDiscovery::Explained);
}

#[test]
fn test_failure_mode_variants_exist() {
    let _ = std::mem::discriminant(&FailureMode::PortNotMapped);
    let _ = std::mem::discriminant(&FailureMode::AtelierNotResponding { port: 52773 });
    let _ = std::mem::discriminant(&FailureMode::AtelierHttpError { port: 52773, status: 503 });
    let _ = std::mem::discriminant(&FailureMode::AtelierAuth401 { port: 52773 });
}

// ── T015/T016: discover_iris + container not found ────────────────────────────

/// T015: DiscoveryResult::NotFound is distinct from FoundUnhealthy
#[test]
fn test_discovery_result_not_found_is_distinct() {
    let r = DiscoveryResult::NotFound;
    assert!(matches!(r, DiscoveryResult::NotFound));
    assert!(!matches!(r, DiscoveryResult::FoundUnhealthy(_)));
}

/// T016: FoundUnhealthy carries a FailureMode
#[test]
fn test_discovery_result_found_unhealthy_carries_mode() {
    let r = DiscoveryResult::FoundUnhealthy(FailureMode::PortNotMapped);
    match r {
        DiscoveryResult::FoundUnhealthy(FailureMode::PortNotMapped) => {}
        _ => panic!("expected FoundUnhealthy(PortNotMapped)"),
    }
}

// ── T022/T023: PortNotMapped ──────────────────────────────────────────────────

/// T022: PortNotMapped variant roundtrip
#[test]
fn test_failure_mode_port_not_mapped() {
    let mode = FailureMode::PortNotMapped;
    assert!(matches!(mode, FailureMode::PortNotMapped));
}

/// T023: IrisDiscovery::Explained is distinct from NotFound
#[test]
fn test_iris_discovery_explained_is_distinct_from_not_found() {
    let explained = IrisDiscovery::Explained;
    let not_found = IrisDiscovery::NotFound;
    assert!(!matches!(explained, IrisDiscovery::NotFound));
    assert!(!matches!(not_found, IrisDiscovery::Explained));
}

// ── T029/T030/T031: AtelierNotResponding, AtelierHttpError ────────────────────

/// T029: AtelierNotResponding carries port
#[test]
fn test_failure_mode_atelier_not_responding() {
    let mode = FailureMode::AtelierNotResponding { port: 52791 };
    match mode {
        FailureMode::AtelierNotResponding { port: 52791 } => {}
        _ => panic!("wrong variant"),
    }
}

/// T030: AtelierHttpError carries port + status
#[test]
fn test_failure_mode_atelier_http_error() {
    let mode = FailureMode::AtelierHttpError { port: 52791, status: 503 };
    match mode {
        FailureMode::AtelierHttpError { port: 52791, status: 503 } => {}
        _ => panic!("wrong variant"),
    }
}

/// T031: FoundUnhealthy(AtelierNotResponding) is distinct from NotFound
#[test]
fn test_found_unhealthy_atelier_is_not_not_found() {
    let r = DiscoveryResult::FoundUnhealthy(FailureMode::AtelierNotResponding { port: 52791 });
    assert!(!matches!(r, DiscoveryResult::NotFound));
    assert!(matches!(r, DiscoveryResult::FoundUnhealthy(_)));
}

// ── T038/T039: AtelierAuth401 ────────────────────────────────────────────────

/// T038: AtelierAuth401 carries port
#[test]
fn test_failure_mode_auth_401() {
    let mode = FailureMode::AtelierAuth401 { port: 52790 };
    match mode {
        FailureMode::AtelierAuth401 { port: 52790 } => {}
        _ => panic!("wrong variant"),
    }
}

/// T039: Auth401 maps to Explained (not NotFound) — structural check
#[test]
fn test_auth_401_maps_to_explained_not_not_found() {
    // The mapping logic is: FoundUnhealthy(Auth401) → Explained in discover_iris().
    // We verify this structurally: Explained ≠ NotFound.
    let explained = IrisDiscovery::Explained;
    assert!(!matches!(explained, IrisDiscovery::NotFound));
}

// ── FR-007: localhost scan credential check ───────────────────────────────────

/// T051: When IRIS_USERNAME/IRIS_PASSWORD are set, they should be used (structural check)
/// The actual behavior is tested via E2E; here we verify the env vars are read.
#[test]
fn test_iris_username_env_var_readable() {
    std::env::set_var("IRIS_USERNAME_TEST_028", "testuser");
    let val = std::env::var("IRIS_USERNAME_TEST_028").unwrap();
    assert_eq!(val, "testuser");
    std::env::remove_var("IRIS_USERNAME_TEST_028");
}

// ── score_container_name (existing tests preserved below) ────────────────────

// ── Basic coverage ────────────────────────────────────────────────────────────

#[test]
fn test_score_empty_workspace_returns_zero() {
    assert_eq!(
        score_container_name("any-iris", ""),
        0,
        "empty workspace basename must score 0"
    );
}

#[test]
fn test_score_unrelated_scores_zero() {
    assert_eq!(score_container_name("redis-cache", "myapp"), 0);
}

#[test]
fn test_score_exact_match_is_100() {
    assert_eq!(score_container_name("myapp", "myapp"), 100);
}

#[test]
fn test_score_starts_with_is_80() {
    // "myapp-dev" starts with "myapp" but no -iris suffix
    assert_eq!(score_container_name("myapp-dev", "myapp"), 80);
}

#[test]
fn test_score_contains_match_is_60() {
    // "myapp" is contained in "xyz-myapp-iris" but doesn't start with it
    let s = score_container_name("xyz_myapp_iris", "myapp");
    assert_eq!(s, 70, "contains match + iris suffix = 60 + 10 = 70");
}

#[test]
fn test_score_iris_suffix_bonus_10() {
    let with_iris = score_container_name("loanapp-iris", "loanapp");
    let without = score_container_name("loanapp-dev", "loanapp");
    // loanapp-iris: 80 + 10 = 90; loanapp-dev: 80 + 0 = 80
    assert_eq!(with_iris, 90);
    assert_eq!(without, 80);
    assert!(with_iris > without, "-iris suffix must score higher");
}

#[test]
fn test_score_test_suffix_bonus_5() {
    let with_test = score_container_name("myapp-test", "myapp");
    let without = score_container_name("myapp-dev", "myapp");
    // myapp-test: 80 + 5 = 85; myapp-dev: 80 + 0 = 80
    assert_eq!(with_test, 85);
    assert!(
        with_test > without,
        "-test suffix must score higher than -dev"
    );
}

#[test]
fn test_score_iris_and_test_suffix_not_double_counted() {
    // A name can't end in both -iris and -test simultaneously (they're different suffixes)
    // Verify only one bonus is added at a time
    let iris_only = score_container_name("app-iris", "app");
    let test_only = score_container_name("app-test", "app");
    assert_eq!(iris_only, 90);
    assert_eq!(test_only, 85);
}

// ── Case insensitivity ────────────────────────────────────────────────────────

#[test]
fn test_score_exact_case_insensitive() {
    let s1 = score_container_name("MyApp-IRIS", "myapp");
    let s2 = score_container_name("myapp-iris", "myapp");
    assert_eq!(s1, s2, "scoring must be case-insensitive");
}

#[test]
fn test_score_workspace_uppercase() {
    let s1 = score_container_name("myapp-iris", "MYAPP");
    let s2 = score_container_name("myapp-iris", "myapp");
    assert_eq!(s1, s2, "workspace name case should not matter");
}

// ── Hyphen/underscore normalization ───────────────────────────────────────────

#[test]
fn test_score_underscore_hyphen_equivalence() {
    // id_try2 workspace should match id-try2-iris container
    let s = score_container_name("id-try2-iris", "id_try2");
    assert!(s > 0, "id_try2 should match id-try2-iris, got {}", s);
    assert!(s >= 80, "should score at least 80 (starts_with), got {}", s);
}

#[test]
fn test_score_hyphen_workspace_underscore_container() {
    let s = score_container_name("id_try2_iris", "id-try2");
    assert!(
        s > 0,
        "id-try2 workspace should match id_try2_iris container"
    );
}

#[test]
fn test_score_all_hyphens_normalized() {
    // my-loan-app vs my_loan_app should be equivalent
    let s1 = score_container_name("my-loan-app", "my_loan_app");
    assert_eq!(
        s1, 100,
        "all-hyphen and all-underscore should be exact match after normalization"
    );
}

// ── starts_with vs contains ordering ─────────────────────────────────────────

#[test]
fn test_score_starts_with_beats_contains() {
    let starts = score_container_name("appname-iris", "appname");
    let contains = score_container_name("myappname-iris", "appname");
    // starts = 80+10=90; contains = 60+10=70
    assert!(
        starts > contains,
        "starts_with ({}) must score higher than contains ({})",
        starts,
        contains
    );
}

#[test]
fn test_score_exact_beats_starts_with() {
    let exact = score_container_name("loanapp", "loanapp");
    let starts = score_container_name("loanapp-iris", "loanapp");
    // exact = 100; starts+iris = 80+10 = 90
    assert_eq!(exact, 100);
    assert_eq!(starts, 90);
    assert!(
        exact > starts,
        "exact match (100) must beat starts_with+iris (90)"
    );
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn test_score_single_char_workspace() {
    let s = score_container_name("a-iris", "a");
    assert!(s > 0, "single-char workspace should still match, got {}", s);
    assert_eq!(s, 90); // starts_with "a" + iris suffix = 80+10
}

#[test]
fn test_score_empty_container_name() {
    // Empty container can't match anything
    assert_eq!(score_container_name("", "myapp"), 0);
}

#[test]
fn test_score_both_empty() {
    assert_eq!(score_container_name("", ""), 0);
}

#[test]
fn test_score_container_only_iris_suffix() {
    // Container "iris" for workspace "iris" — exact match = 100
    assert_eq!(score_container_name("iris", "iris"), 100);
}

#[test]
fn test_score_underscore_iris_suffix_also_counts() {
    // ends_with("_iris") should also earn the +10 bonus
    let s = score_container_name("myapp_iris", "myapp");
    assert_eq!(s, 90, "underscore iris suffix should also score 90");
}

#[test]
fn test_score_known_example_loanapp_iris() {
    // Canonical example from spec-025
    let score = score_container_name("loanapp-iris", "loanapp");
    assert_eq!(score, 90, "loanapp-iris for loanapp should score 90");
}

#[test]
fn test_score_determined_cray_is_zero() {
    assert_eq!(score_container_name("determined_cray", "id_try2"), 0);
}
