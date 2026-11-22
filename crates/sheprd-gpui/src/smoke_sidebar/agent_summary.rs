use super::*;
use sheprd_client::protocol::{
    AgentStatus, ClientShellSnapshot,
    endpoint::{
        AGENT_STATUS_MESSAGES_CAPABILITY, EndpointAgentStatusMessages, EndpointServerWelcome,
    },
};
use std::{collections::BTreeMap, sync::Arc};

const MESSAGE: &str = "进展摘要需要在狭窄侧栏中保持可读，并安全截断。";

pub(super) async fn run(handle: WindowHandle<SheprdWindow>, cx: &mut AsyncApp) -> Result<()> {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))?;
    snapshot.revision += 1;
    snapshot.agents[0].agent_status = AgentStatus::Idle;
    for index in 2..=100 {
        let mut pane = snapshot.panes[0].clone();
        pane.pane_id = format!("w1:p{index}");
        pane.focused = false;
        snapshot.panes.push(pane);
    }
    let full_message = MESSAGE.repeat(24);
    let preview = format!("{}…", full_message.chars().take(200).collect::<String>());
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
    let messages = Arc::new(EndpointAgentStatusMessages {
        boot_id: snapshot.boot_id.clone(),
        revision: snapshot.revision,
        messages: BTreeMap::from([("w1:p1".into(), full_message.clone())]),
    });
    let snapshot = Arc::new(snapshot);
    let timer = cx.background_executor().clone();

    for (index, mode) in [
        crate::config::LayoutMode::from(crate::config::Density::Normal),
        crate::config::LayoutMode::Superset,
        crate::config::LayoutMode::Orca,
        crate::config::LayoutMode::Minimal,
    ]
    .into_iter()
    .enumerate()
    {
        timer.timer(Duration::from_millis(100)).await;
        AnyWindowHandle::from(handle).update(cx, |root, window, cx| -> Result<()> {
            let view = root
                .downcast::<SheprdWindow>()
                .map_err(|_| anyhow!("unexpected root"))?;
            view.update(cx, |view, cx| {
                view.live.agent_sidebar.connected(&welcome);
                view.live.agent_sidebar.receive(messages.clone());
                view.live.agent_sidebar.commit_snapshot(snapshot.clone());
                view.live.snapshot = Some(snapshot.clone());
                view.live.status = crate::state::ConnectionStatus::Connected;
                view.endpoints.truncate(1);
                view.selected_endpoint = 0;
                view.selected_generation = view.endpoints[0].generation;
                view.endpoints[0].collapsed = false;
                view.endpoints[0].collapsed_repos.clear();
                view.collapsed_repos.clear();
                view.endpoints[0].live = view.live.clone();
                view.activity = Default::default();
                view.config.usage.inline = true;
                view.config.sidebar_layout.spaces.pane_strip = true;
                view.config.sidebar_layout.spaces.status_text = true;
                view.config.layout.mode = mode;
                view.sidebar_width = Some(160.);
                view.theme_light = index % 2 != 0;
                let mut theme = crate::config::Theme::default();
                if view.theme_light {
                    theme.background = 0xf7f7f7;
                    theme.foreground = 0x202124;
                    theme.surface = 0xffffff;
                    theme.active = 0xe3e9f2;
                    theme.muted = 0x5f6368;
                    theme.sidebar = Some(0xf1f3f4);
                }
                view.theme = theme.styled_for(&view.config);
                cx.notify();
            });
            window.resize(fixture_size(640., 500.));
            cx.default_global::<sidebar::layout_tests::PaintedProbes>()
                .0
                .clear();
            window.refresh();
            window.draw(cx).clear(cx);
            cx.default_global::<sidebar::layout_tests::PaintedProbes>()
                .check()?;
            let probes = &cx.global::<sidebar::layout_tests::PaintedProbes>().0;
            let overflow = probes
                .iter()
                .find(|(text, _)| {
                    text.starts_with('+')
                        && text.len() > 1
                        && text[1..].bytes().all(|b| b.is_ascii_digit())
                })
                .map(|(_, probe)| probe)
                .context("pane overflow was not painted")?;
            if overflow.clipped || !overflow.glyph_text.starts_with('+') {
                bail!("{mode}: pane overflow clipped: {overflow:?}");
            }
            let message = probes
                .get(&preview)
                .context("long CJK agent message was not painted")?;
            if !message.glyph_text.ends_with('\u{2026}') {
                bail!("{mode}: long CJK agent message not ellipsized: {message:?}");
            }
            eprintln!("SIDEBAR verified agent summary layout={mode}");
            Ok(())
        })??;
    }
    timer.timer(Duration::from_millis(100)).await;
    AnyWindowHandle::from(handle).update(cx, |root, window, cx| -> Result<()> {
        use sidebar::layout_tests::PaintedProbes;

        let view = root
            .downcast::<SheprdWindow>()
            .map_err(|_| anyhow!("unexpected root"))?;
        let recap_label = crate::i18n::fl!("sidebar-agent-recap-available");
        let point = cx
            .global::<PaintedProbes>()
            .0
            .get(&recap_label)
            .context("missing recap pill target")?
            .bounds
            .center();
        let modifiers = Modifiers::default();
        for event in [
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point,
                pressed_button: None,
                modifiers,
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point,
                modifiers,
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: point,
                modifiers,
                click_count: 1,
            }),
        ] {
            window.dispatch_event(event, cx);
        }
        if view.read(cx).agent_recap.is_none() {
            bail!("clicking the idle agent message did not open its recap");
        }
        window.draw(cx).clear(cx);

        let before = view.read(cx).input_probe;
        let key = KeyDownEvent {
            keystroke: crate::keymap::parse_keystroke("a")?,
            is_held: false,
            prefer_character_input: false,
        };
        window.dispatch_event(PlatformInput::KeyDown(key), cx);
        window.dispatch_event(
            PlatformInput::Scroll(ScrollEvent {
                position: point,
                delta: ScrollDelta::Pixels(gpui::point(px(0.), px(-40.))),
                ..Default::default()
            }),
            cx,
        );
        let after = view.read(cx).input_probe;
        if after.keys != before.keys
            || after.scrolls != before.scrolls
            || after.actions != before.actions
            || after.text != before.text
        {
            bail!("recap keyboard or wheel input reached the terminal");
        }
        let escape = KeyDownEvent {
            keystroke: crate::keymap::parse_keystroke("escape")?,
            is_held: false,
            prefer_character_input: false,
        };
        window.dispatch_event(PlatformInput::KeyDown(escape), cx);
        Ok(())
    })??;
    timer
        .timer(crate::motion::duration(crate::motion::LEAVE) * 2)
        .await;
    AnyWindowHandle::from(handle).update(cx, |root, _, cx| -> Result<()> {
        let view = root
            .downcast::<SheprdWindow>()
            .map_err(|_| anyhow!("unexpected root"))?;
        if view.read(cx).agent_recap.is_some() {
            bail!("Escape did not close the native recap");
        }
        Ok(())
    })??;
    eprintln!("SIDEBAR verified recap keyboard and wheel isolation");
    Ok(())
}
