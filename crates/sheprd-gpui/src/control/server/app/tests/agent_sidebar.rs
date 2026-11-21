use super::*;
use crate::sidebar::layout_tests::fixture_window;
use sheprd_client::protocol::{
    AgentStatus,
    endpoint::{
        AGENT_STATUS_MESSAGES_CAPABILITY, EndpointAgentStatusMessages, EndpointServerWelcome,
    },
};
use std::{collections::BTreeMap, sync::Arc};

fn recap_welcome() -> EndpointServerWelcome {
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

fn recap_fixture(
    window: &mut Window,
    cx: &mut gpui::Context<SheprdWindow>,
    status: AgentStatus,
    message: &str,
) -> SheprdWindow {
    let mut view = fixture_window(window, cx);
    let mut snapshot = crate::sidebar::layout_tests::snapshot(1);
    snapshot.agents[0].agent_status = status;
    let snapshot = Arc::new(snapshot);
    let pane = snapshot.agents[0].pane_id.clone();
    view.live.snapshot = Some(snapshot.clone());
    view.live.agent_sidebar.connected(&recap_welcome());
    view.live
        .agent_sidebar
        .receive(Arc::new(EndpointAgentStatusMessages {
            boot_id: snapshot.boot_id.clone(),
            revision: snapshot.revision,
            messages: BTreeMap::from([(pane, message.to_owned())]),
        }));
    view.live.agent_sidebar.commit_snapshot(snapshot);
    view
}

#[gpui::test]
fn sidebar_query_refuses_hosts_without_the_negotiated_projection(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = cx.add_window_view(fixture_window);
    for op in [UiOp::AgentSidebar, UiOp::AgentRecap { pane: "p0".into() }] {
        let response = cx.cx.update(|cx| ui(call(op), cx));
        assert_eq!(code(&response), Some(ErrorCode::Unsupported));
    }
}

#[gpui::test]
fn recap_caller_never_targets_another_host_with_the_same_pane_id(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = recap_fixture(window, cx, AgentStatus::Idle, "local recap");
        let mut remote = crate::endpoint::Endpoint::new(
            "remote".into(),
            "Remote".into(),
            ConnectTarget::Socket("foreign.sock".into()),
            true,
        );
        let mut snapshot = crate::sidebar::layout_tests::snapshot(1);
        snapshot.agents[0].agent_status = AgentStatus::Idle;
        let snapshot = Arc::new(snapshot);
        let pane = snapshot.agents[0].pane_id.clone();
        remote.live.snapshot = Some(snapshot.clone());
        remote.live.agent_sidebar.connected(&recap_welcome());
        remote
            .live
            .agent_sidebar
            .receive(Arc::new(EndpointAgentStatusMessages {
                boot_id: snapshot.boot_id.clone(),
                revision: snapshot.revision,
                messages: BTreeMap::from([(pane, "remote recap".into())]),
            }));
        remote.live.agent_sidebar.commit_snapshot(snapshot);
        remote.live.status = crate::state::ConnectionStatus::Connected;
        view.endpoints.push(remote);
        view
    });
    let pane = view.read_with(cx, |view, _| {
        view.live.snapshot.as_ref().unwrap().agents[0]
            .pane_id
            .clone()
    });
    let caller = Caller {
        pane_id: Some(pane.clone()),
        workspace_id: Some("w0".into()),
        daemon_socket: Some("foreign.sock".into()),
        ..Caller::default()
    };
    let response = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            super::super::agent_sidebar::recap(view, &caller, pane.clone(), window, cx)
        })
    });
    assert!(matches!(
        response,
        Response::App {
            reply: Reply::AgentRecap {
                endpoint,
                message,
                ..
            }
        } if endpoint == "remote" && message == "remote recap"
    ));
    let missing_host = Caller {
        pane_id: Some(pane.clone()),
        workspace_id: Some("w0".into()),
        daemon_socket: Some("missing.sock".into()),
        ..Caller::default()
    };
    let response = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            super::super::agent_sidebar::recap(view, &missing_host, pane, window, cx)
        })
    });
    assert_eq!(code(&response), Some(ErrorCode::NotFound));
}

#[gpui::test]
fn recap_rejects_invalid_pane_ids_before_host_lookup(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = cx.add_window_view(fixture_window);
    let response = cx.cx.update(|cx| {
        ui(
            call(UiOp::AgentRecap {
                pane: String::new(),
            }),
            cx,
        )
    });
    assert_eq!(code(&response), Some(ErrorCode::InvalidRequest));
}

#[gpui::test]
fn recap_returns_full_cached_idle_message_and_refuses_a_second_modal(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx
        .add_window_view(|window, cx| recap_fixture(window, cx, AgentStatus::Idle, "full\nrecap"));
    let (endpoint, pane) = view.read_with(cx, |view, _| {
        (
            view.endpoints[0].id.clone(),
            view.live.snapshot.as_ref().unwrap().agents[0]
                .pane_id
                .clone(),
        )
    });
    let response = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            super::super::agent_sidebar::recap(view, &Caller::default(), pane.clone(), window, cx)
        })
    });
    assert!(matches!(
        response,
        Response::App {
            reply: Reply::AgentRecap {
                endpoint: actual_endpoint,
                pane: actual_pane,
                message,
            }
        } if actual_endpoint == endpoint && actual_pane == pane && message == "full\nrecap"
    ));
    let response = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            super::super::agent_sidebar::recap(view, &Caller::default(), pane, window, cx)
        })
    });
    assert_eq!(code(&response), Some(ErrorCode::Busy));
}

#[gpui::test]
fn a_non_idle_agent_without_a_recap_returns_no_result(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        recap_fixture(window, cx, AgentStatus::Working, "working message")
    });
    let pane = view.read_with(cx, |view, _| {
        view.live.snapshot.as_ref().unwrap().agents[0]
            .pane_id
            .clone()
    });
    let response = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            super::super::agent_sidebar::recap(view, &Caller::default(), pane, window, cx)
        })
    });
    assert_eq!(code(&response), Some(ErrorCode::NoResult));
}

#[test]
fn escaped_previews_respect_control_message_limit() {
    use crate::control::protocol::app::{AgentSidebarState, SidebarAgent, SidebarHost};

    let state = AgentSidebarState {
        selected: None,
        hosts: vec![SidebarHost {
            id: "host".into(),
            supported: true,
            workspaces: Vec::new(),
            agents: (0..400)
                .map(|index| SidebarAgent {
                    pane: format!("pane-{index}"),
                    workspace: "workspace".into(),
                    status: AgentStatus::Working,
                    preview: Some("\"\\界".repeat(200)),
                    idle_recap: false,
                    focused: false,
                    linked: false,
                })
                .collect(),
        }],
    };
    let response = super::super::agent_sidebar::bounded_response(Response::App {
        reply: Reply::AgentSidebar { state },
    });
    let encoded = serde_json::to_vec(&response).unwrap();
    assert!(
        encoded.len() <= crate::control::protocol::MAX_MESSAGE,
        "{} bytes",
        encoded.len()
    );
    assert_eq!(code(&response), Some(ErrorCode::Failed));
}
