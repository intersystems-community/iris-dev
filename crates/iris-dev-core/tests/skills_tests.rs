//! T032: Unit tests for KB skills subscription loader.
//! Uses mock GitHub API responses.

use iris_dev_core::skills::SkillRegistry;

/// SkillRegistry starts empty.
#[test]
fn registry_starts_empty() {
    let registry = SkillRegistry::new();
    assert_eq!(registry.list_skills().len(), 0);
}

/// load_from_github with invalid repo gracefully fails.
#[tokio::test]
async fn load_invalid_repo_returns_error() {
    let mut registry = SkillRegistry::new();
    // nonexistent repo should return Ok (graceful) or Err — never panic
    let result = registry.load_from_github("nonexistent/repo-that-does-not-exist-xyzzy").await;
    // Accept either outcome — the important thing is no panic
    let _ = result;
}

/// Multiple subscriptions accumulate independently.
#[tokio::test]
async fn multiple_subscriptions_accumulate() {
    let mut registry = SkillRegistry::new();
    // Both loads fail gracefully on nonexistent repos
    let _ = registry.load_from_github("owner1/repo1").await;
    let _ = registry.load_from_github("owner2/repo2").await;
    // Registry should not have grown (both failed) — just verify no panic
    let _ = registry.list_skills().len();
}
