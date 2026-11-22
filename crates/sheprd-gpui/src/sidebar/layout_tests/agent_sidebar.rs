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
fn status_text_clicks_do_not_open_recap(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = sidebar_view(cx, true);
    draw_sidebar(cx);

    // Status text itself in Spaces and Agents is plain text, not a recap link.
    let ws_status = cx.debug_bounds("workspace-status-w1").unwrap();
    cx.simulate_click(ws_status.center(), Modifiers::default());
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap").is_none());

    let message = cx.debug_bounds("agent-message-w1:p1").unwrap();
    cx.simulate_click(message.center(), Modifiers::default());
    draw_sidebar(cx);
    assert!(cx.debug_bounds("agent-recap").is_none());
}

#[gpui::test]
fn recap_click_is_modal_and_does_not_navigate_or_send_input(cx: &mut gpui::TestAppContext) {
    let (view, cx) = sidebar_view(cx, true);
    draw_sidebar(cx);

    // The explicit recap pill still opens the modal without navigating or typing.
    let pill = cx.debug_bounds("agent-recap-agent-w1:p1").unwrap();
    cx.simulate_click(pill.center(), Modifiers::default());
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
    let pill = cx.debug_bounds("agent-recap-agent-w1:p1").unwrap();
    cx.simulate_click(pill.center(), Modifiers::default());
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

#[gpui::test]
#[allow(clippy::unwrap_used, clippy::expect_used)]
fn workspace_summary_agent_status_and_focus_bar_fit_inside_every_layout_row(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
            "../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
        ))
        .unwrap();
        snapshot.revision += 1;
        snapshot.agent_view_label = None;
        snapshot.agent_order.clear();
        // Workspace 1: two omp agents — one idle with recap (unfocused), one idle without message (focused).
        let mut second_agent = snapshot.agents[0].clone();
        snapshot.agents[0].agent = Some("omp".into());
        snapshot.agents[0].display_agent = Some("omp".into());
        snapshot.agents[0].agent_status = AgentStatus::Idle;
        snapshot.agents[0].focused = false;
        second_agent.pane_id = "w1:p2".into();
        second_agent.agent = Some("omp".into());
        second_agent.display_agent = Some("omp".into());
        second_agent.agent_status = AgentStatus::Idle;
        second_agent.focused = true;
        let mut second_pane = snapshot.panes[0].clone();
        second_pane.pane_id = "w1:p2".into();
        second_pane.focused = true;
        snapshot.panes[0].focused = false;
        snapshot.panes.push(second_pane);
        snapshot.focused_pane_id = Some("w1:p2".into());

        // Workspace 2: a working agent with a live task message.
        let mut ws2 = snapshot.workspaces[0].clone();
        ws2.workspace_id = "w2".into();
        ws2.active_tab_id = "w2:t1".into();
        ws2.number = 2;
        ws2.label = "other".into();
        ws2.focused = false;
        ws2.agent_status = AgentStatus::Working;
        snapshot.workspaces.push(ws2);
        let mut tab2 = snapshot.tabs[0].clone();
        tab2.tab_id = "w2:t1".into();
        tab2.workspace_id = "w2".into();
        tab2.focused = false;
        snapshot.tabs.push(tab2);
        let mut ws2_pane = snapshot.panes[0].clone();
        ws2_pane.pane_id = "w2:p1".into();
        ws2_pane.workspace_id = "w2".into();
        ws2_pane.tab_id = "w2:t1".into();
        ws2_pane.focused = false;
        snapshot.panes.push(ws2_pane);
        let mut ws2_agent = second_agent.clone();
        ws2_agent.pane_id = "w2:p1".into();
        ws2_agent.workspace_id = "w2".into();
        ws2_agent.tab_id = "w2:t1".into();
        ws2_agent.agent_status = AgentStatus::Working;
        ws2_agent.focused = false;

        snapshot.agents.push(second_agent);
        snapshot.agents.push(ws2_agent);

        let welcome = EndpointServerWelcome {
            generation: 1,
            server_version: "test".into(),
            snapshot_codec: String::new(),
            surface_codec: String::new(),
            input_codec: String::new(),
            blob_codec: String::new(),
            methods: Vec::new(),
            capabilities: vec![AGENT_STATUS_MESSAGES_CAPABILITY.into()],
            error: None,
        };
        view.live.agent_sidebar.connected(&welcome);
        view.live
            .agent_sidebar
            .receive(Arc::new(EndpointAgentStatusMessages {
                boot_id: snapshot.boot_id.clone(),
                revision: snapshot.revision,
                messages: BTreeMap::from([
                    (
                        "w1:p1".into(),
                        "Sheprd 仓库（Rust workspace）正在优化智能体侧栏两行自动回绕与长文本截断展示，确保超过两百字符的回顾内容不会垂直裁切。"
                            .repeat(5),
                    ),
                    ("w2:p1".into(), "Implementing sidebar polish".into()),
                ]),
            }));
        let snapshot = Arc::new(snapshot);
        view.live.agent_sidebar.commit_snapshot(snapshot.clone());
        view.live.snapshot = Some(snapshot);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.endpoints[0].live = view.live.clone();
        view.config.usage.inline = true;
        view.config.sidebar_layout.spaces = SpaceLayout::default();
        view
    });
    cx.simulate_resize(size(px(800.), px(900.)));

    for mode in [
        LayoutMode::from(Density::Normal),
        LayoutMode::Superset,
        LayoutMode::Orca,
        LayoutMode::Minimal,
    ] {
        view.update(cx, |view, cx| {
            view.config.layout.mode = mode;
            view.sidebar_width = Some(240.);
            cx.notify();
        });
        draw_sidebar(cx);

        // Active workspace and all its agents link automatically before any click.
        assert!(
            cx.debug_bounds("linked-workspace-local-w1").is_some(),
            "{mode:?}"
        );
        assert!(
            cx.debug_bounds("linked-agent-local-w1:p1").is_some(),
            "{mode:?}"
        );
        assert!(
            cx.debug_bounds("linked-agent-local-w1:p2").is_some(),
            "{mode:?}"
        );
        assert!(
            cx.debug_bounds("linked-agent-local-w2:p1").is_none(),
            "{mode:?}"
        );

        // Workspace summary (pane strip + winner status) sits inside the workspace row itself.
        let ws_row = cx.debug_bounds("row-repo").expect("workspace row");
        for selector in ["pane-strip-w1", "workspace-status-w1"] {
            let child = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{mode:?}: {selector}"));
            assert!(
                child.top() >= ws_row.top()
                    && child.bottom() <= ws_row.bottom()
                    && child.left() >= ws_row.left()
                    && child.right() <= ws_row.right(),
                "{mode:?}: {selector} {child:?} escapes workspace row {ws_row:?}"
            );
        }

        // Agent with live message and agent with fallback status both sit inside their own rows.
        let agent1 = cx.debug_bounds("row-agent-w1:p1").expect("agent 1 row");
        let msg1 = cx.debug_bounds("agent-message-w1:p1").expect("agent 1 msg");
        assert!(
            msg1.top() >= agent1.top()
                && msg1.bottom() <= agent1.bottom()
                && msg1.left() >= agent1.left()
                && msg1.right() <= agent1.right(),
            "{mode:?}: message {msg1:?} escapes agent row {agent1:?}"
        );

        let agent2 = cx.debug_bounds("row-agent-w1:p2").expect("agent 2 row");
        let fallback2 = cx
            .debug_bounds("agent-status-fallback-w1:p2")
            .expect("agent 2 fallback status");
        assert!(
            fallback2.top() >= agent2.top()
                && fallback2.bottom() <= agent2.bottom()
                && fallback2.left() >= agent2.left()
                && fallback2.right() <= agent2.right(),
            "{mode:?}: fallback {fallback2:?} escapes agent row {agent2:?}"
        );
        // Both wrapped long messages and fallback status labels reserve the same 2-line height,
        // and the agent mark/name shares the first line of the status block.
        assert_eq!(
            msg1.size.height, fallback2.size.height,
            "{mode:?}: agent status area should always reserve 2 lines"
        );
        assert!(
            msg1.size.height >= px(24.),
            "{mode:?}: expected 2-line status height, got {:?}",
            msg1.size.height
        );
        let agent_name_selector = if mode == LayoutMode::from(Density::Normal) {
            "detail-agent-w1:p1"
        } else {
            "name-agent-w1:p1"
        };
        let agent_name = cx
            .debug_bounds(agent_name_selector)
            .unwrap_or_else(|| panic!("{mode:?}: missing {agent_name_selector}"));
        assert!(
            agent_name.top() >= msg1.top() && agent_name.bottom() <= msg1.bottom(),
            "{mode:?}: agent mark/name {agent_name:?} should share line 1 of status {msg1:?}"
        );
        if mode != LayoutMode::Superset {
            let icon = cx
                .debug_bounds("agent-icon-agent-w1:p1")
                .unwrap_or_else(|| panic!("{mode:?}: missing inline agent icon"));
            assert_eq!(
                icon.top(),
                msg1.top(),
                "{mode:?}: agent icon should align with line 1 of status"
            );
        }

        // Focused workspace shows the leading green focus bar; unfocused workspace does not.
        let ws_bar = cx
            .debug_bounds("workspace-focus-bar-w1")
            .expect("focused workspace bar");
        assert!(
            ws_bar.top() >= ws_row.top()
                && ws_bar.bottom() <= ws_row.bottom()
                && ws_bar.left() >= ws_row.left(),
            "{mode:?}: workspace focus bar {ws_bar:?} outside focused workspace {ws_row:?}"
        );
        assert!(
            cx.debug_bounds("workspace-focus-bar-w2").is_none(),
            "{mode:?}: unfocused workspace must not have a focus bar"
        );

        // Focused agent shows the leading green focus bar; unfocused linked agent does not.
        let bar = cx
            .debug_bounds("agent-focus-bar-w1:p2")
            .expect("focused agent bar");
        assert!(
            bar.top() >= agent2.top()
                && bar.bottom() <= agent2.bottom()
                && bar.left() >= agent2.left(),
            "{mode:?}: focus bar {bar:?} outside focused agent {agent2:?}"
        );
        assert!(
            cx.debug_bounds("agent-focus-bar-w1:p1").is_none(),
            "{mode:?}: unfocused agent must not have a focus bar"
        );

        // Workspace 2 shows its winning working agent's live status message,
        // and its single-agent pane badge still renders inside the row.
        let ws2_row = cx.debug_bounds("row-other").expect("workspace 2 row");
        for selector in [
            "pane-strip-w2",
            "pane-dot-agent-w2-0-omp-working",
            "pane-dot-winner-w2-0",
        ] {
            let child = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{mode:?}: {selector}"));
            assert!(
                child.top() >= ws2_row.top()
                    && child.bottom() <= ws2_row.bottom()
                    && child.left() >= ws2_row.left()
                    && child.right() <= ws2_row.right(),
                "{mode:?}: {selector} {child:?} escapes workspace 2 row {ws2_row:?}"
            );
        }
        assert!(
            cx.update(|_, cx| cx
                .global::<TextProbes>()
                .0
                .contains_key("Implementing sidebar polish")),
            "{mode:?}: workspace 2 winner status message should be painted"
        );
    }

    // Clicking workspace 2 moves the linked highlight to w2 + w2:p1, and a repeat click keeps it.
    let ws2_row = cx.debug_bounds("row-other").expect("workspace 2 row");
    for _ in 0..2 {
        cx.simulate_click(ws2_row.center(), Modifiers::default());
        draw_sidebar(cx);
        assert!(cx.debug_bounds("linked-workspace-local-w2").is_some());
        assert!(cx.debug_bounds("linked-agent-local-w2:p1").is_some());
        assert!(cx.debug_bounds("linked-agent-local-w1:p1").is_none());
        assert!(cx.debug_bounds("linked-agent-local-w1:p2").is_none());
    }
}

#[gpui::test]
#[allow(clippy::unwrap_used, clippy::expect_used)]
fn pane_strip_uses_agent_icons_with_status_colors_and_aligned_winner_underline(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
            "../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
        ))
        .unwrap();
        snapshot.revision += 1;
        snapshot.agent_view_label = None;
        snapshot.agent_order.clear();

        let base_pane = snapshot.panes[0].clone();
        let base_agent = snapshot.agents[0].clone();
        snapshot.panes.clear();
        snapshot.agents.clear();

        // Five panes covering Done (winner), Working, Idle, Unknown (Omo PNG), and non-agent.
        let panes = [
            (Some("claude"), Some(AgentStatus::Done)),
            (Some("omp"), Some(AgentStatus::Working)),
            (Some("agy"), Some(AgentStatus::Idle)),
            (Some("omo"), Some(AgentStatus::Unknown)),
            (None, None),
        ];
        for (index, (kind, status)) in panes.into_iter().enumerate() {
            let pane_id = format!("w1:p{}", index + 1);
            let mut pane = base_pane.clone();
            pane.pane_id = pane_id.clone();
            pane.focused = index == 2;
            snapshot.panes.push(pane);
            if let (Some(kind), Some(status)) = (kind, status) {
                let mut agent = base_agent.clone();
                agent.pane_id = pane_id;
                agent.agent = Some(kind.into());
                agent.agent_status = status;
                agent.focused = index == 2;
                snapshot.agents.push(agent);
            }
        }
        snapshot.workspaces[0].agent_status = AgentStatus::Done;
        snapshot.focused_pane_id = Some("w1:p3".into());

        let welcome = EndpointServerWelcome {
            generation: 1,
            server_version: "test".into(),
            snapshot_codec: String::new(),
            surface_codec: String::new(),
            input_codec: String::new(),
            blob_codec: String::new(),
            methods: Vec::new(),
            capabilities: vec![AGENT_STATUS_MESSAGES_CAPABILITY.into()],
            error: None,
        };
        view.live.agent_sidebar.connected(&welcome);
        let snapshot = Arc::new(snapshot);
        view.live.agent_sidebar.commit_snapshot(snapshot.clone());
        view.live.snapshot = Some(snapshot);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.endpoints[0].live = view.live.clone();
        view.config.status_icons = crate::config::StatusIcons::Solid;
        view.config.layout.mode = LayoutMode::Orca;
        view.config.sidebar_layout.spaces = SpaceLayout::default();
        view.sidebar_width = Some(260.);
        view
    });
    cx.simulate_resize(size(px(800.), px(900.)));
    draw_sidebar(cx);

    let done = cx
        .debug_bounds("pane-dot-agent-w1-0-claude-done")
        .expect("done agent icon");
    let working = cx
        .debug_bounds("pane-dot-agent-w1-1-omp-working")
        .expect("working agent icon");
    let idle = cx
        .debug_bounds("pane-dot-agent-w1-2-agy-idle")
        .expect("idle agent icon");
    let unknown = cx
        .debug_bounds("pane-dot-agent-w1-3-omo-unknown")
        .expect("unknown omo agent badge");
    let shell = cx
        .debug_bounds("pane-dot-symbol-w1-4-unknown")
        .expect("non-agent pane uses unknown symbol");

    // Winner underline sits on the Done pane without shifting its icon baseline.
    let underline = cx
        .debug_bounds("pane-dot-winner-w1-0")
        .expect("winner underline");
    assert!(cx.debug_bounds("pane-dot-winner-w1-1").is_none());
    assert!(cx.debug_bounds("pane-dot-winner-w1-2").is_none());
    assert_eq!(done.top(), working.top());
    assert_eq!(done.top(), idle.top());
    assert_eq!(done.top(), unknown.top());
    assert_eq!(done.top(), shell.top());
    assert!(underline.top() >= done.center().y);

    // Blocked outranks Done and moves the winner underline to the blocked agent icon.
    view.update(cx, |view, cx| {
        let mut next = (**view.live.snapshot.as_ref().unwrap()).clone();
        next.revision += 1;
        next.agents[1].agent_status = AgentStatus::Blocked;
        next.workspaces[0].agent_status = AgentStatus::Blocked;
        let next = Arc::new(next);
        view.live.agent_sidebar.commit_snapshot(next.clone());
        view.live.snapshot = Some(next);
        view.endpoints[0].live = view.live.clone();
        cx.notify();
    });
    draw_sidebar(cx);
    assert!(cx.debug_bounds("pane-dot-agent-w1-1-omp-blocked").is_some());
    assert!(cx.debug_bounds("pane-dot-winner-w1-1").is_some());
    assert!(cx.debug_bounds("pane-dot-winner-w1-0").is_none());
}

#[test]
#[allow(clippy::unwrap_used)]
fn pane_badge_colors_meet_contrast_and_stay_distinct_across_themes() {
    use crate::config::Theme;
    use crate::contrast::Contrast;
    use crate::sidebar::row::agent_sidebar::pane_badge_color;

    let statuses = [
        AgentStatus::Blocked,
        AgentStatus::Working,
        AgentStatus::Done,
        AgentStatus::Idle,
        AgentStatus::Unknown,
    ];
    for name in Theme::BUILTIN_NAMES {
        for contrast in [Contrast::Standard, Contrast::High] {
            let theme = Theme::builtin(name).unwrap().with_contrast(contrast);
            for status in statuses {
                let color = pane_badge_color(status, &theme);
                for background in [theme.background, theme.surface, theme.active] {
                    let ratio = crate::contrast::ratio(color, background);
                    assert!(
                        ratio >= contrast.mark_ratio(),
                        "{name} {contrast:?} {status:?} on {background:06x}: {ratio}"
                    );
                }
            }
            let primary = [
                pane_badge_color(AgentStatus::Blocked, &theme),
                pane_badge_color(AgentStatus::Working, &theme),
                pane_badge_color(AgentStatus::Done, &theme),
                pane_badge_color(AgentStatus::Idle, &theme),
            ];
            let unique: std::collections::HashSet<_> = primary.into_iter().collect();
            assert_eq!(unique.len(), 4, "{name} {contrast:?}: {primary:?}");
        }
    }
}

#[gpui::test]
#[allow(clippy::unwrap_used, clippy::expect_used)]
fn focused_workspace_without_agents_uses_neutral_gray_focus_bar(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
            "../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
        ))
        .unwrap();
        snapshot.revision += 1;
        snapshot.agents.clear();
        snapshot.workspaces[0].focused = true;
        snapshot.workspaces[0].agent_status = AgentStatus::Unknown;
        let snapshot = Arc::new(snapshot);
        view.live.snapshot = Some(snapshot);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.endpoints[0].live = view.live.clone();
        view.sidebar_width = Some(240.);
        view
    });
    cx.simulate_resize(size(px(800.), px(900.)));

    for mode in [
        LayoutMode::from(Density::Normal),
        LayoutMode::Superset,
        LayoutMode::Orca,
        LayoutMode::Minimal,
    ] {
        view.update(cx, |view, cx| {
            view.config.layout.mode = mode;
            cx.notify();
        });
        draw_sidebar(cx);

        let ws_row = cx.debug_bounds("row-repo").expect("workspace row");
        let bar = cx
            .debug_bounds("workspace-focus-bar-neutral-w1")
            .unwrap_or_else(|| panic!("{mode:?}: missing neutral focus bar"));
        assert!(
            bar.top() >= ws_row.top()
                && bar.bottom() <= ws_row.bottom()
                && bar.left() >= ws_row.left(),
            "{mode:?}: neutral focus bar {bar:?} outside workspace row {ws_row:?}"
        );
        assert!(
            cx.debug_bounds("workspace-focus-bar-w1").is_none(),
            "{mode:?}: workspace without agents must not use green focus bar"
        );
        assert!(
            cx.debug_bounds("pane-strip-w1").is_none(),
            "{mode:?}: single plain-shell workspace must not show pane strip"
        );
    }

    let theme = crate::config::Theme::default();
    let mut active = crate::sidebar::row::agent_sidebar::focus_bar(
        "active".into(),
        crate::sidebar::cell::FocusBarTone::Active,
        0.,
        0.,
        0.,
        &theme,
    );
    let mut neutral = crate::sidebar::row::agent_sidebar::focus_bar(
        "neutral".into(),
        crate::sidebar::cell::FocusBarTone::Neutral,
        0.,
        0.,
        0.,
        &theme,
    );
    assert_eq!(
        active.style().background,
        Some(gpui::rgb(theme.ink(theme.palette[2])).into())
    );
    assert_eq!(
        neutral.style().background,
        Some(gpui::rgb(theme.ink(theme.muted)).into())
    );
}
