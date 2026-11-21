use crate::sidebar::{layout_tests::fixture_window, selection::Target};
use gpui::TestAppContext;
use sheprd_client::protocol::endpoint::{
    AGENT_STATUS_MESSAGES_CAPABILITY, EndpointAgentStatusMessages, EndpointServerWelcome,
};
use std::{collections::BTreeMap, sync::Arc};

fn welcome() -> EndpointServerWelcome {
    EndpointServerWelcome {
        generation: 1,
        server_version: "test".into(),
        snapshot_codec: "snapshot".into(),
        surface_codec: "surface".into(),
        input_codec: "input".into(),
        blob_codec: "blob".into(),
        methods: Vec::new(),
        capabilities: vec![AGENT_STATUS_MESSAGES_CAPABILITY.into()],
        error: None,
    }
}

#[gpui::test]
fn changing_the_linked_workspace_invalidates_hosted_state(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    let before = view.read_with(cx, |view, _| view.hosted_stamp());
    let target = view.read_with(cx, |view, _| {
        let endpoint = &view.endpoints[0];
        let snapshot = view.live.snapshot.as_ref().unwrap();
        Target {
            endpoint: endpoint.id.clone(),
            generation: endpoint.generation,
            boot: snapshot.boot_id.clone(),
            workspace: snapshot.workspaces[0].workspace_id.clone(),
        }
    });
    view.update(cx, |view, cx| {
        view.sidebar_selection.select(target);
        cx.notify();
    });
    let after = view.read_with(cx, |view, _| view.hosted_stamp());
    assert!(before != after, "linked selection is part of hosted state");
}

#[gpui::test]
fn matching_message_and_disconnect_invalidate_hosted_projection(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        let snapshot = view.live.snapshot.as_ref().unwrap().clone();
        view.live.agent_sidebar.connected(&welcome());
        view.live.agent_sidebar.commit_snapshot(snapshot);
        view
    });
    let before = view.read_with(cx, |view, _| view.hosted_stamp());
    let message = "\"\\界".repeat(240);
    view.update(cx, |view, cx| {
        let snapshot = view.live.snapshot.as_ref().unwrap().clone();
        let pane = snapshot.agents[0].pane_id.clone();
        view.live
            .agent_sidebar
            .receive(Arc::new(EndpointAgentStatusMessages {
                boot_id: snapshot.boot_id.clone(),
                revision: snapshot.revision,
                messages: BTreeMap::from([(pane, message.clone())]),
            }));
        view.live.agent_sidebar.commit_snapshot(snapshot);
        cx.notify();
    });
    let with_message = view.read_with(cx, |view, _| view.hosted_stamp());
    assert!(before != with_message);
    let hosted = view.update(cx, |view, cx| view.hosted_state(cx));
    let preview = hosted.main["agent_sidebar"]["hosts"][0]["agents"][0]["preview"].as_str();
    let expected = message.chars().take(200).collect::<String>();
    assert_eq!(preview, Some(expected.as_str()));

    view.update(cx, |view, cx| {
        view.live.agent_sidebar.disconnect();
        view.live.status = crate::state::ConnectionStatus::Disconnected;
        cx.notify();
    });
    let disconnected = view.read_with(cx, |view, _| view.hosted_stamp());
    assert!(with_message != disconnected);
    let hosted = view.update(cx, |view, cx| view.hosted_state(cx));
    assert_eq!(hosted.main["agent_sidebar"]["hosts"][0]["supported"], false);
    assert_eq!(
        hosted.main["agent_sidebar"]["hosts"][0]["agents"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
}
