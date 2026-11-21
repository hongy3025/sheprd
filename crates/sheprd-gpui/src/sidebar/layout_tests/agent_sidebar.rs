use super::*;
use crate::config::{Density, LayoutMode, SpaceLayout};
use sheprd_client::protocol::{
    AgentStatus, ClientShellSnapshot,
    endpoint::{
        AGENT_STATUS_MESSAGES_CAPABILITY, EndpointAgentStatusMessages, EndpointServerWelcome,
    },
};
use std::{collections::BTreeMap, sync::Arc};

const MESSAGE: &str = "The latest run completed its focused checks.";

fn sidebar_view(
    cx: &mut gpui::TestAppContext,
    supported: bool,
) -> (Entity<SheprdWindow>, &mut gpui::VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
            "../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
        ))
        .unwrap();
        snapshot.revision += 1;
        snapshot.agents[0].agent_status = AgentStatus::Idle;
        for index in 2..=100 {
            let mut pane = snapshot.panes[0].clone();
            pane.pane_id = format!("w1:p{index}");
            pane.focused = false;
            snapshot.panes.push(pane);
        }
        let welcome = EndpointServerWelcome {
            generation: 1,
            server_version: "test".into(),
            snapshot_codec: String::new(),
            surface_codec: String::new(),
            input_codec: String::new(),
            blob_codec: String::new(),
            methods: Vec::new(),
            capabilities: if supported {
                vec![AGENT_STATUS_MESSAGES_CAPABILITY.into()]
            } else {
                Vec::new()
            },
            error: None,
        };
        view.live.agent_sidebar.connected(&welcome);
        if supported {
            view.live
                .agent_sidebar
                .receive(Arc::new(EndpointAgentStatusMessages {
                    boot_id: snapshot.boot_id.clone(),
                    revision: snapshot.revision,
                    messages: BTreeMap::from([("w1:p1".into(), MESSAGE.into())]),
                }));
        }
        let snapshot = Arc::new(snapshot);
        view.live.agent_sidebar.commit_snapshot(snapshot.clone());
        view.live.snapshot = Some(snapshot);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.config.usage.inline = true;
        view.config.sidebar_layout.spaces = SpaceLayout::default();
        view
    });
    cx.simulate_resize(size(px(800.), px(900.)));
    (view, cx)
}

fn draw_sidebar(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| {
        cx.default_global::<TextProbes>().0.clear();
        full_draw(window, cx).clear(cx);
    });
}

#[gpui::test]
fn summary_and_message_fit_every_layout_and_truncate_many_panes(cx: &mut gpui::TestAppContext) {
    let (view, cx) = sidebar_view(cx, true);
    for mode in [
        LayoutMode::from(Density::Normal),
        LayoutMode::Superset,
        LayoutMode::Orca,
        LayoutMode::Minimal,
    ] {
        view.update(cx, |view, cx| {
            view.config.layout.mode = mode;
            view.sidebar_width = Some(160.);
            cx.notify();
        });
        draw_sidebar(cx);
        for selector in [
            "pane-strip-w1",
            "pane-dot-w1-0",
            "pane-overflow-w1",
            "workspace-status-w1",
            "agent-message-w1:p1",
        ] {
            assert!(cx.debug_bounds(selector).is_some(), "{mode:?}: {selector}");
        }
        assert!(
            cx.update(|_, cx| cx.global::<TextProbes>().0.contains_key(MESSAGE)),
            "{mode:?}: message text"
        );
    }
}

#[gpui::test]
fn summary_flags_are_independent_and_unsupported_hosts_keep_old_rows(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = sidebar_view(cx, true);
    draw_sidebar(cx);
    assert!(cx.debug_bounds("pane-strip-w1").is_some());
    assert!(cx.debug_bounds("workspace-status-w1").is_some());

    view.update(cx, |view, cx| {
        view.config.sidebar_layout.spaces.pane_strip = false;
        cx.notify();
    });
    draw_sidebar(cx);
    assert!(cx.debug_bounds("pane-strip-w1").is_none());
    assert!(cx.debug_bounds("workspace-status-w1").is_some());

    view.update(cx, |view, cx| {
        view.config.sidebar_layout.spaces.pane_strip = true;
        view.config.sidebar_layout.spaces.status_text = false;
        cx.notify();
    });
    draw_sidebar(cx);
    assert!(cx.debug_bounds("pane-strip-w1").is_some());
    assert!(cx.debug_bounds("workspace-status-w1").is_none());

    let (legacy, cx) = sidebar_view(cx, false);
    draw_sidebar(cx);
    assert!(cx.debug_bounds("pane-strip-w1").is_none());
    assert!(cx.debug_bounds("workspace-status-w1").is_none());
    assert!(cx.debug_bounds("agent-message-w1:p1").is_none());
    assert!(cx.debug_bounds("name-repo").is_some());
    drop(legacy);
}

#[gpui::test]
fn related_workspace_and_agent_rows_share_only_the_sidebar_highlight(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = sidebar_view(cx, true);
    view.update(cx, |view, cx| {
        let boot = view.live.snapshot.as_ref().unwrap().boot_id.clone();
        view.sidebar_selection
            .select(crate::sidebar::selection::Target {
                endpoint: crate::endpoint::LOCAL.into(),
                generation: 0,
                boot,
                workspace: "w1".into(),
            });
        cx.notify();
    });
    draw_sidebar(cx);
    assert!(cx.debug_bounds("linked-workspace-local-w1").is_some());
    assert!(cx.debug_bounds("linked-agent-local-w1:p1").is_some());
}

#[gpui::test]
fn recap_click_is_modal_and_does_not_navigate_or_send_input(cx: &mut gpui::TestAppContext) {
    let (view, cx) = sidebar_view(cx, true);
    draw_sidebar(cx);
    let message = cx.debug_bounds("agent-message-w1:p1").unwrap();
    cx.simulate_click(message.center(), Modifiers::default());
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap").is_some());

    cx.simulate_input("must stay in recap");
    cx.simulate_keystrokes("enter");
    cx.dispatch_action(crate::actions::Paste);
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap").is_some());
    view.read_with(cx, |view, _| {
        assert!(view.pending_navigation.is_none());
        assert_eq!(view.pending_input.len(), 0);
    });

    cx.simulate_keystrokes("escape");
    cx.executor().advance_clock(crate::motion::LEAVE * 2);
    cx.run_until_parked();
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap").is_none());
}

#[gpui::test]
fn an_open_recap_keeps_its_captured_message_after_a_boot_change(cx: &mut gpui::TestAppContext) {
    let (view, cx) = sidebar_view(cx, true);
    draw_sidebar(cx);
    let message = cx.debug_bounds("agent-message-w1:p1").unwrap();
    cx.simulate_click(message.center(), Modifiers::default());
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap").is_some());

    view.update(cx, |view, cx| {
        view.endpoints[0].generation += 1;
        let mut snapshot = (**view.live.snapshot.as_ref().unwrap()).clone();
        snapshot.boot_id = "next-boot".into();
        view.live.snapshot = Some(Arc::new(snapshot));
        view.live.status = crate::state::ConnectionStatus::Disconnected;
        view.live.agent_sidebar.disconnect();
        view.endpoints[0].live = view.live.clone();
        cx.notify();
    });
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap-source-offline").is_some());
    assert!(
        cx.update(|_, cx| cx.global::<TextProbes>().0.contains_key(MESSAGE)),
        "the captured recap remains visible"
    );
}
