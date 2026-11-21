use super::super::cell::{AgentRow, RowContext, RowLayout, WorkspaceRow};
use super::{ICON_RESERVE, RowIcon, RowTree};
use crate::direction::Logical;
use crate::i18n::fl;
use crate::window::SheprdWindow;
use gpui::{prelude::*, *};
use unicode_width::UnicodeWidthStr;

pub(in crate::sidebar) fn workspace(
    row: &WorkspaceRow<'_>,
    layout: &dyn RowLayout,
    cx: &RowContext<'_>,
) -> Option<Div> {
    let summary = row.summary.filter(|_| row.pane_strip);
    let status_text = row.summary_text.as_ref();
    let status_label = status_text.map(AsRef::as_ref);
    if summary.is_none() && status_label.is_none() {
        return None;
    }

    let font = cx.font;
    let density = cx.look.density;
    let gap = density.gap();
    let icon_width = if matches!(&row.icon, RowIcon::None) {
        0.
    } else {
        super::scaled(font, ICON_RESERVE)
    };
    let icon_gap = if icon_width > 0. { gap } else { 0. };
    let status_slot = cx.indicators.width(font);
    let indent = cx.nest
        + if matches!(&row.tree, RowTree::None) {
            0.
        } else {
            density.child_indent() + status_slot - super::STATUS_WIDTH
        };
    let left = cx.look.content_x() + indent + status_slot + gap + icon_width + icon_gap;
    let available =
        (cx.look.content_width(cx.width) - indent - status_slot - gap - icon_width - icon_gap)
            .max(0.);
    let text_width = |text: &str| {
        (UnicodeWidthStr::width(text) as f32 * super::glyph_width(font))
            .ceil()
            .max(0.)
    };
    let tab_text = summary.map(|summary| fl!("sidebar-tab-count", count = summary.tab_count));
    let diameter = super::scaled(font, 6.);
    let attention = summary
        .and_then(|summary| summary.other_tabs_attention)
        .filter(|_| available >= diameter + gap * 4.);
    let attention_width = attention.map(|_| diameter).unwrap_or_default();
    let labels_room = (available - attention_width - gap * 4.).max(0.);
    let pane_count = summary.map_or(0, |summary| summary.pane_dots.len());
    let overflow_width = if pane_count > 1 {
        (pane_count.ilog10() as f32 + 2.) * super::glyph_width(font)
    } else {
        0.
    };
    let pane_reserve = if pane_count > 1 {
        diameter + gap + overflow_width
    } else if pane_count == 1 {
        diameter
    } else {
        0.
    };
    let text_room = (labels_room - pane_reserve).max(0.);
    let status_width = status_label
        .map(text_width)
        .unwrap_or_default()
        .min(text_room * 0.45);
    let tab_width = tab_text
        .as_deref()
        .map(text_width)
        .unwrap_or_default()
        .min((text_room - status_width).max(0.));
    let pane_room = (labels_room - status_width - tab_width).max(0.);
    let pane_step = diameter + gap;
    let (visible_panes, overflow) = summary.map_or((0, 0), |summary| {
        let count = summary.pane_dots.len();
        let max_without_overflow = (pane_room / pane_step).floor() as usize;
        if count <= max_without_overflow {
            (count, 0)
        } else {
            let visible = ((pane_room - overflow_width).max(0.) / pane_step).floor() as usize;
            let visible = visible.min(count);
            (visible, count.saturating_sub(visible))
        }
    });
    let line_height = super::line_height(font);
    let theme = cx.theme;
    let mut line = div()
        .w_full()
        .min_w_0()
        .h(px(line_height))
        .row()
        .items_center()
        .gap(px(gap))
        .ps(px(left))
        .pe(px(cx.look.content_x()))
        .overflow_hidden();

    if summary.is_some() {
        let selector = format!("pane-strip-{}", row.workspace.id);
        line = line.debug_selector(move || selector);
    }
    if let (Some(summary), Some(tab_text)) = (summary, tab_text) {
        let selector = format!("workspace-tab-count-{}", row.workspace.id);
        line = line.child(
            div()
                .debug_selector(move || selector)
                .w(px(tab_width))
                .min_w_0()
                .truncate()
                .flex_none()
                .text_color(rgb(theme.muted))
                .child(tab_text),
        );
        if let Some(status) = attention {
            let selector = format!("other-tabs-attention-{}", row.workspace.id);
            line = line.child(
                div()
                    .debug_selector(move || selector)
                    .size(px(super::scaled(font, 6.)))
                    .rounded_full()
                    .flex_none()
                    .bg(rgb(cx.indicators.color(status))),
            );
        }
        for (index, dot) in summary.pane_dots.iter().take(visible_panes).enumerate() {
            let selector = format!("pane-dot-{}-{index}", row.workspace.id);
            let color = dot
                .status
                .map_or(theme.muted, |status| cx.indicators.color(status));
            line = line.child(
                div()
                    .debug_selector(move || selector)
                    .size(px(diameter))
                    .rounded_full()
                    .flex_none()
                    .bg(rgb(color))
                    .when(dot.winner, |dot| {
                        dot.border_1().border_color(rgb(theme.foreground))
                    }),
            );
        }
        if overflow > 0 {
            let text = format!("+{overflow}");
            let overflow_width = text_width(&text);
            let selector = format!("pane-overflow-{}", row.workspace.id);
            line = line.child(
                div()
                    .debug_selector(move || selector)
                    .w(px(overflow_width))
                    .min_w_0()
                    .truncate()
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(super::label_text(&text)),
            );
        }
    }

    if let Some(text) = status_text {
        let selector = format!("workspace-status-{}", row.workspace.id);
        let color = summary
            .and_then(|summary| summary.winner)
            .map_or(theme.foreground, |winner| {
                cx.indicators.color(winner.status)
            });
        line = line.child(
            div()
                .debug_selector(move || selector)
                .w(px(status_width))
                .min_w_0()
                .flex_none()
                .truncate()
                .text_color(rgb(color))
                .child(text.clone()),
        );
    }

    Some(
        line.relative()
            .children(layout.trunk_below(row.label, row.tree, cx)),
    )
}

pub(in crate::sidebar) fn agent_message(
    row: &AgentRow<'_>,
    cx: &RowContext<'_>,
    window_cx: Option<&mut Context<SheprdWindow>>,
) -> Option<Div> {
    let message = row.message?;
    let font = cx.font;
    let gap = cx.look.density.gap();
    let left = cx.look.content_x()
        + cx.nest
        + cx.indicators.width(font)
        + gap
        + super::scaled(font, 16.)
        + gap;
    let pane_id = row.key.strip_prefix("agent-").unwrap_or(&row.key);
    let selector = SharedString::from(format!("agent-message-{pane_id}"));
    let message = div()
        .id(selector.clone())
        .debug_selector(move || selector.to_string())
        .flex()
        .flex_1()
        .min_w_0()
        .truncate()
        .child(super::label_text(message))
        .when(row.message_truncated, |line| line.child("…"));
    let message = match (row.idle_recap, row.recap_target.as_ref(), window_cx) {
        (true, Some(target), Some(window_cx)) => {
            let click_target = target.clone();
            let key_target = target.clone();
            message
                .role(Role::Button)
                .aria_label(fl!("sidebar-agent-recap-open", pane = target.1.clone()))
                .tab_index(0)
                .cursor_pointer()
                .on_click(window_cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_agent_recap(&click_target.0, &click_target.1, window, cx);
                }))
                .on_key_down(
                    window_cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            window.prevent_default();
                            this.open_agent_recap(&key_target.0, &key_target.1, window, cx);
                        }
                    }),
                )
        }
        _ => message,
    };
    Some(
        div()
            .w_full()
            .min_w_0()
            .h(px(super::line_height(font)))
            .row()
            .items_center()
            .gap(px(gap))
            .ps(px(left))
            .pe(px(cx.look.content_x()))
            .text_color(rgb(cx.theme.foreground))
            .child(message)
            .when(row.idle_recap, |line| {
                line.child(
                    div()
                        .debug_selector(|| format!("agent-recap-{}", row.key))
                        .flex_none()
                        .text_color(rgb(cx.theme.muted))
                        .child(fl!("sidebar-agent-recap-available")),
                )
            }),
    )
}
