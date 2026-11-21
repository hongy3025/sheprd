use super::*;
use sheprd_client::protocol::endpoint::EndpointServerWelcome;
use sheprd_client::{
    ClientEvent,
    protocol::{AgentStatus, endpoint::EndpointAgentStatusMessages},
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn connected_state(supported: bool) -> LiveState {
    let mut welcome: EndpointServerWelcome = serde_json::from_str(include_str!(
        "../../../../sheprd-protocol/tests/fixtures/endpoint-welcome-v1.json"
    ))
    .unwrap();
    if supported {
        welcome.capabilities.push("agent_status_messages".into());
    }
    let mut state = LiveState::default();
    state.apply(ClientEvent::Connected(welcome));
    state
}

fn status(boot_id: &str, revision: u64, text: &str) -> ClientEvent {
    ClientEvent::AgentStatusMessages(Arc::new(EndpointAgentStatusMessages {
        boot_id: boot_id.into(),
        revision,
        messages: BTreeMap::from([("w1:p1".into(), text.into())]),
    }))
}

#[test]
fn pending_projection_does_not_replace_visible_pair_until_surface_commit() {
    let mut state = connected_state(true);
    let old = snapshot();
    state.apply(status("boot-v1", old.revision, "old"));
    state.apply(ClientEvent::Snapshot(old.clone()));
    state.apply(ClientEvent::Surface(surface(&old)));
    assert_eq!(
        state
            .agent_sidebar
            .prepared()
            .unwrap()
            .preview("w1:p1")
            .unwrap()
            .0,
        "old"
    );

    state.supports_surface = true;
    let mut next = old.as_ref().clone();
    next.revision += 1;
    let next = Arc::new(next);
    state.apply(status("boot-v1", next.revision, "new"));
    state.apply(ClientEvent::Snapshot(next.clone()));

    assert!(Arc::ptr_eq(state.snapshot.as_ref().unwrap(), &old));
    assert_eq!(
        state
            .agent_sidebar
            .prepared()
            .unwrap()
            .preview("w1:p1")
            .unwrap()
            .0,
        "old"
    );

    state.apply(ClientEvent::Surface(surface(&next)));
    assert!(Arc::ptr_eq(state.snapshot.as_ref().unwrap(), &next));
    assert_eq!(
        state
            .agent_sidebar
            .prepared()
            .unwrap()
            .preview("w1:p1")
            .unwrap()
            .0,
        "new"
    );
}

#[test]
fn legacy_snapshot_keeps_the_agent_sidebar_extension_disabled() {
    let mut state = connected_state(false);
    state.apply(ClientEvent::Snapshot(snapshot()));

    assert!(state.agent_sidebar.prepared().is_none());
}

#[test]
fn stale_projection_never_appears_under_a_newer_snapshot() {
    let mut state = connected_state(true);
    state.apply(status("boot-v1", 8, "future"));
    state.apply(ClientEvent::Snapshot(snapshot()));

    let prepared = state.agent_sidebar.prepared().unwrap();
    assert!(prepared.preview("w1:p1").is_none());
    assert_eq!(
        state.snapshot.as_ref().unwrap().agents[0].agent_status,
        AgentStatus::Blocked
    );

    let mut next = snapshot().as_ref().clone();
    next.revision = 8;
    state.apply(ClientEvent::Snapshot(Arc::new(next)));
    assert_eq!(
        state
            .agent_sidebar
            .prepared()
            .unwrap()
            .preview("w1:p1")
            .unwrap()
            .0,
        "future"
    );
}

#[test]
fn disconnect_drops_the_extension_and_its_full_message() {
    let mut state = connected_state(true);
    let snapshot = snapshot();
    state.apply(status("boot-v1", snapshot.revision, "recap"));
    state.apply(ClientEvent::Snapshot(snapshot));
    assert!(state.agent_sidebar.prepared().is_some());

    state.apply(ClientEvent::Disconnected {
        reason: "test".into(),
        ssh: None,
    });

    assert!(state.agent_sidebar.prepared().is_none());
}
