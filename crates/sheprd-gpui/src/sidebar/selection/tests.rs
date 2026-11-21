use super::*;

fn target() -> Target {
    Target {
        endpoint: "desktop-a".into(),
        generation: 4,
        boot: "boot-a".into(),
        workspace: "workspace-a".into(),
    }
}

#[test]
fn selecting_the_same_target_is_idempotent() {
    let mut selection = Selection::default();
    selection.select(target());
    let stored = selection.target.as_ref().unwrap() as *const Target;
    selection.select(target());
    assert_eq!(stored, selection.target.as_ref().unwrap() as *const Target);
}

#[test]
fn a_link_requires_the_complete_endpoint_snapshot_identity() {
    let mut selection = Selection::default();
    selection.select(target());
    assert!(selection.matches("desktop-a", 4, "boot-a", "workspace-a"));
    assert!(!selection.matches("desktop-b", 4, "boot-a", "workspace-a"));
    assert!(!selection.matches("desktop-a", 5, "boot-a", "workspace-a"));
    assert!(!selection.matches("desktop-a", 4, "boot-b", "workspace-a"));
    assert!(!selection.matches("desktop-a", 4, "boot-a", "workspace-b"));
}

#[test]
fn retention_clears_only_a_target_the_snapshot_dropped() {
    let mut selection = Selection::default();
    selection.select(target());
    selection.retain(|target| target.workspace == "workspace-a");
    assert!(selection.target.is_some());
    selection.retain(|target| target.workspace != "workspace-a");
    assert!(selection.target.is_none());
}
