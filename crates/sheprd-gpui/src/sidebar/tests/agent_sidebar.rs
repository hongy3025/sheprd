use crate::sidebar::layout_tests::{fixture_window, snapshot};
use sheprd_client::ConnectTarget;
use sheprd_client::protocol::{
    AgentStatus,
    endpoint::{
        AGENT_STATUS_MESSAGES_CAPABILITY, EndpointAgentStatusMessages, EndpointServerWelcome,
    },
};
use std::{collections::BTreeMap, sync::Arc};

fn welcome() -> EndpointServerWelcome {
    EndpointServerWelcome {
        generation: 1,
        server_version: "1".into(),
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
fn only_capable_hosts_report_version_matched_bounded_summaries(cx: &mut gpui::TestAppContext) {
    let (legacy, cx) = cx.add_window_view(fixture_window);
    let legacy = legacy.update(cx, |view, _| crate::sidebar::agent_sidebar::state(view));
    assert!(!legacy.hosts[0].supported);
    assert!(legacy.hosts[0].workspaces.is_empty());
    assert!(legacy.hosts[0].agents.is_empty());

    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        let mut snapshot = snapshot(2);
        snapshot.agents[0].agent_status = AgentStatus::Idle;
        snapshot.agents[0].focused = true;
        snapshot.focused_pane_id = Some(snapshot.agents[0].pane_id.clone());
        let snapshot = Arc::new(snapshot);
        let pane = snapshot.agents[0].pane_id.clone();
        view.live.snapshot = Some(snapshot.clone());
        view.live.agent_sidebar.connected(&welcome());
        view.live
            .agent_sidebar
            .receive(Arc::new(EndpointAgentStatusMessages {
                boot_id: snapshot.boot_id.clone(),
                revision: snapshot.revision,
                messages: BTreeMap::from([(pane.clone(), "x".repeat(240))]),
            }));
        view.live.agent_sidebar.commit_snapshot(snapshot.clone());
        let endpoint = view.endpoints[0].id.clone();
        view.select_sidebar_agent(&endpoint, &pane);
        view
    });
    view.update(cx, |view, _| {
        view.endpoints.push(crate::endpoint::Endpoint::new(
            "legacy".into(),
            "Legacy".into(),
            ConnectTarget::Socket("legacy.sock".into()),
            true,
        ));
    });
    let state = view.update(cx, |view, _| crate::sidebar::agent_sidebar::state(view));
    let host = &state.hosts[0];
    assert!(!state.hosts[1].supported);
    assert!(state.hosts[1].workspaces.is_empty());
    assert!(state.hosts[1].agents.is_empty());
    assert!(host.supported);
    assert!(!host.workspaces.is_empty());
    let agent = host.agents.first().unwrap();
    let pane = agent.pane.clone();
    assert_eq!(agent.preview.as_deref().map(str::len), Some(200));
    assert!(agent.idle_recap);
    assert!(agent.focused);
    assert!(agent.linked);
    assert_eq!(
        state
            .selected
            .as_ref()
            .map(|selected| selected.workspace.as_str()),
        Some(agent.workspace.as_str())
    );

    let stale = view.update(cx, |view, _| {
        let snapshot = view.live.snapshot.as_ref().unwrap().clone();
        view.live
            .agent_sidebar
            .receive(Arc::new(EndpointAgentStatusMessages {
                boot_id: snapshot.boot_id.clone(),
                revision: snapshot.revision + 1,
                messages: BTreeMap::from([(pane.clone(), "newer".into())]),
            }));
        crate::sidebar::agent_sidebar::state(view)
    });
    assert_eq!(
        stale.hosts[0]
            .agents
            .iter()
            .find(|agent| agent.pane == pane)
            .unwrap()
            .preview
            .as_deref()
            .map(str::len),
        Some(200)
    );
}
