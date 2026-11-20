use crate::{SheprdWindow, config::corners, i18n::fl, motion};
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, FocusHandle, KeyDownEvent,
    MouseButton, Role, ScrollHandle, SharedString, Window, div, prelude::*, px, rgb, rgba,
};

/// The message captured when an idle agent's recap marker was opened.
pub(crate) struct Recap {
    endpoint: String,
    generation: u64,
    boot: String,
    pane: String,
    message: SharedString,
    focus: FocusHandle,
    scroll: ScrollHandle,
    motion: motion::Toggle,
}

impl Recap {
    pub(crate) fn focus(&self) -> &FocusHandle {
        &self.focus
    }
}

impl SheprdWindow {
    /// Opens only the message paired with the current snapshot. It stays
    /// captured if that endpoint disconnects or starts a different boot.
    pub(crate) fn open_agent_recap(
        &mut self,
        endpoint_id: &str,
        pane: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.page.is_some() || self.agent_recap.is_some() {
            return false;
        }
        let source = self
            .endpoints
            .iter()
            .enumerate()
            .find(|(_, endpoint)| endpoint.id == endpoint_id)
            .and_then(|(index, endpoint)| {
                if index == self.selected_endpoint
                    && endpoint.generation != self.selected_generation
                {
                    return None;
                }
                let live = if index == self.selected_endpoint {
                    &self.live
                } else {
                    &endpoint.live
                };
                let snapshot = live.snapshot.as_ref()?;
                let prepared = live.agent_sidebar.prepared()?;
                if !prepared.agent(pane)?.idle_recap {
                    return None;
                }
                Some((
                    endpoint.id.clone(),
                    endpoint.generation,
                    snapshot.boot_id.clone(),
                    prepared.full_message(pane)?.to_owned(),
                ))
            });
        let Some((endpoint, generation, boot, message)) = source else {
            return false;
        };
        let focus = cx.focus_handle();
        let mut transition = motion::Toggle::default();
        transition.set(true, motion::now(), false);
        let recap = Recap {
            endpoint,
            generation,
            boot,
            pane: pane.to_owned(),
            message: SharedString::from(message),
            focus,
            scroll: ScrollHandle::new(),
            motion: transition,
        };
        window.focus(&recap.focus, cx);
        self.agent_recap = Some(recap);
        cx.notify();
        true
    }

    /// Begins leaving the captured message, restoring focus to the window
    /// without sending the close keystroke to a terminal.
    pub(crate) fn close_agent_recap(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(recap) = self.agent_recap.as_mut() else {
            return false;
        };
        if !recap.motion.is_open() {
            return true;
        }
        recap.motion.set(false, motion::now(), false);
        let endpoint = recap.endpoint.clone();
        let generation = recap.generation;
        let boot = recap.boot.clone();
        let pane = recap.pane.clone();
        window.focus(&self.focus, cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(motion::duration(motion::LEAVE))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.agent_recap.as_ref().is_some_and(|recap| {
                    !recap.motion.is_open()
                        && recap.endpoint == endpoint
                        && recap.generation == generation
                        && recap.boot == boot
                        && recap.pane == pane
                }) {
                    this.agent_recap = None;
                    cx.notify();
                }
            });
        })
        .detach();
        true
    }

    pub(crate) fn render_agent_recap(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let recap = self.agent_recap.as_ref()?;
        let closing = !recap.motion.is_open();
        let phase = if closing { "out" } else { "in" };
        let animation = || {
            Animation::new(motion::duration(if closing {
                motion::LEAVE
            } else {
                motion::ENTER
            }))
            .with_easing(|k| 1. - (1. - k).powi(3))
        };
        let shown = move |k: f32| {
            let k = if cfg!(test) { 1. } else { k };
            if closing { 1. - k } else { k }
        };
        let title = fl!("sidebar-agent-recap-title", pane = recap.pane.clone());
        let online = self.endpoints.iter().enumerate().any(|(index, endpoint)| {
            let selected = index == self.selected_endpoint;
            let live = if selected { &self.live } else { &endpoint.live };
            endpoint.id == recap.endpoint
                && endpoint.generation == recap.generation
                && (!selected || endpoint.generation == self.selected_generation)
                && live.status.is_connected()
                && live
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.boot_id == recap.boot)
        });
        let source_label = if online {
            fl!("sidebar-agent-recap-source-online")
        } else {
            fl!("sidebar-agent-recap-source-offline")
        };
        let source_selector = if online {
            "agent-recap-source-online"
        } else {
            "agent-recap-source-offline"
        };
        let backdrop = (self.theme.background << 8) | 0xec;
        let message = super::row::label_text(recap.message.as_ref());
        let scroll = div()
            .id("agent-recap-body")
            .debug_selector(|| "agent-recap-body".into())
            .flex_1()
            .min_h_0()
            .max_h(px(480.))
            .overflow_y_scroll()
            .track_scroll(&recap.scroll)
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .child(div().w_full().min_w_0().text_size(px(14.)).child(message));
        let button = div()
            .id("agent-recap-close")
            .debug_selector(|| "agent-recap-close".into())
            .role(Role::Button)
            .tab_index(0)
            .track_focus(&recap.focus)
            .cursor_pointer()
            .px(px(10.))
            .py(px(6.))
            .text_color(rgb(self.theme.foreground))
            .child(fl!("sidebar-agent-recap-close"))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                this.close_agent_recap(window, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    window.prevent_default();
                    this.close_agent_recap(window, cx);
                }
            }));
        let panel = div()
            .id("agent-recap-panel")
            .debug_selector(|| "agent-recap-panel".into())
            .flex()
            .flex_col()
            .min_h_0()
            .w_full()
            .max_w(px(720.))
            .max_h(px(560.))
            .mx(px(20.))
            .p(px(18.))
            .gap(px(12.))
            .rounded(px(corners::CONTROL))
            .border_1()
            .border_color(rgb(self.theme.muted))
            .bg(rgb(self.theme.surface))
            .text_color(rgb(self.theme.foreground))
            .shadow_lg()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(16.))
                            .child(title.clone()),
                    )
                    .child(button),
            )
            .child(
                div()
                    .debug_selector(move || source_selector.to_owned())
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(rgb(self.theme.muted))
                    .child(source_label),
            )
            .child(scroll)
            .with_animation(
                SharedString::from(format!(
                    "agent-recap-panel-{generation}-{phase}",
                    generation = recap.generation
                )),
                animation(),
                move |panel, k| {
                    let shown = shown(k);
                    panel.opacity(shown).mt(px((1. - shown) * 8.))
                },
            );
        let shown_overlay = shown;
        Some(
            div()
                .id(SharedString::from(format!(
                    "agent-recap-{}",
                    recap.generation
                )))
                .debug_selector(|| "agent-recap".into())
                .absolute()
                .inset_0()
                .occlude()
                .role(Role::Dialog)
                .aria_label(title)
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(backdrop))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    this.close_agent_recap(window, cx);
                }))
                .child(panel)
                .with_animation(
                    SharedString::from(format!("agent-recap-backdrop-{phase}")),
                    animation(),
                    move |overlay, k| overlay.opacity(shown_overlay(k)),
                )
                .into_any_element(),
        )
    }
}
