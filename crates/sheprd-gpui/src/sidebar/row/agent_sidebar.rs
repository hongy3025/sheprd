use super::super::{
    cell::{AgentRow, FocusBarTone, RowContext, WorkspaceStatus},
    summary::WorkspaceSummary,
};
use super::wrapped_status::WrappedStatusText;
use crate::{direction::Logical, i18n::fl, icons::AgentIcon, window::SheprdWindow};
use gpui::{prelude::*, *};
use sheprd_client::protocol::AgentStatus;
use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

/// Maximum individual pane dots shown inline before collapsing into `+N`.
const MAX_VISIBLE_DOTS: usize = 6;
/// Width of the focused agent's leading activation bar.
const FOCUS_BAR_WIDTH: f32 = 3.;
/// Fixed line count reserved for an agent row's wrapped status text.
pub(in crate::sidebar) const AGENT_STATUS_LINES: usize = 2;

/// Line height tailored for a secondary status line at `font_size`.
pub(in crate::sidebar) fn compact_line_height(font_size: f32) -> f32 {
    (font_size * 1.4).round().max(font_size + 3.)
}

/// Zero-layout probe overlay marking a row as linked without replacing its own
/// debug selector.
pub(in crate::sidebar) fn linked_marker(kind: &str, endpoint: &str, id: &str) -> Div {
    let selector = format!("linked-{kind}-{endpoint}-{id}");
    div().debug_selector(move || selector).absolute().inset_0()
}

/// Leading vertical accent bar on the focused workspace or agent row: theme
/// success green when tied to an agent, muted gray for a plain workspace with
/// no associated agents.
pub(in crate::sidebar) fn focus_bar(
    selector: String,
    tone: FocusBarTone,
    inset_s: f32,
    vertical_inset: f32,
    radius: f32,
    theme: &crate::config::Theme,
) -> Div {
    let color = match tone {
        FocusBarTone::Active => theme.ink(theme.palette[2]),
        FocusBarTone::Neutral => theme.ink(theme.muted),
    };
    div()
        .debug_selector(move || selector)
        .absolute()
        .inset_s(px(inset_s))
        .top(px(vertical_inset))
        .bottom(px(vertical_inset))
        .w(px(FOCUS_BAR_WIDTH))
        .when(radius > 0., |bar| bar.rounded_s(px(radius)))
        .bg(rgb(color))
}

/// Semantic status color for the Spaces sidebar's `pane_strip` badges.
/// Because agent brand icons replace status shapes, color alone must clearly
/// separate every `AgentStatus` at 12px across dark, light, and high-contrast
/// themes.
pub(in crate::sidebar) fn pane_badge_color(
    status: AgentStatus,
    theme: &crate::config::Theme,
) -> u32 {
    match status {
        AgentStatus::Blocked => theme.ink(0xf87171),
        AgentStatus::Working => theme.ink(0xfbbf24),
        AgentStatus::Done => theme.ink(0x4ade80),
        AgentStatus::Idle => theme.subtext(),
        AgentStatus::Unknown => theme.ink(theme.muted),
    }
}

/// Attention symbol used for `other_tabs_attention` in `pane_strip`.
fn pane_symbol_asset(status: AgentStatus) -> (&'static str, &'static str) {
    let name = super::super::agents::status_key(status);
    let path = crate::icons::status_icon(crate::config::StatusIcons::Tiles, status)
        .unwrap_or("icons/status-mark-unknown.svg");
    (name, path)
}

fn pane_dot_icon(
    workspace_id: &str,
    index: usize,
    agent: Option<AgentIcon>,
    status_name: &str,
    symbol_size: f32,
    color: u32,
) -> AnyElement {
    match agent {
        Some(AgentIcon::Omo) => {
            let selector = format!("pane-dot-agent-{workspace_id}-{index}-omo-{status_name}");
            let inner_size = (symbol_size - 2.).max(6.);
            div()
                .debug_selector(move || selector)
                .size(px(symbol_size))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .bg(rgba((color << 8) | 0x26))
                .border_1()
                .border_color(rgb(color))
                .child(
                    img(AgentIcon::Omo.path())
                        .size(px(inner_size))
                        .flex_none()
                        .object_fit(ObjectFit::Contain),
                )
                .into_any_element()
        }
        Some(agent) => {
            let selector = format!(
                "pane-dot-agent-{workspace_id}-{index}-{}-{status_name}",
                agent.identity()
            );
            svg()
                .debug_selector(move || selector)
                .path(agent.path())
                .size(px(symbol_size))
                .flex_none()
                .text_color(rgb(color))
                .into_any_element()
        }
        None => {
            let selector = format!("pane-dot-symbol-{workspace_id}-{index}-unknown");
            svg()
                .debug_selector(move || selector)
                .path("icons/status-mark-unknown.svg")
                .size(px(symbol_size))
                .flex_none()
                .text_color(rgb(color))
                .into_any_element()
        }
    }
}

/// Compact inline pane strip (tab badge when `>= 2` tabs, plus pane agent
/// icons tinted by status when `>= 2` panes) sized to fit within `max_width`
/// on a workspace's first line.
pub(in crate::sidebar) fn pane_strip(
    workspace_id: &str,
    summary: &WorkspaceSummary,
    max_width: f32,
    cx: &RowContext<'_>,
) -> Option<(f32, Div)> {
    let show_tabs = summary.tab_count >= 2;
    let show_dots = summary.pane_dots.len() >= 2;
    if (!show_tabs && !show_dots) || max_width <= 0. {
        return None;
    }

    let font = cx.font;
    let theme = cx.theme;
    let line_h = super::line_height(font);
    let small = (font.size * 0.8).round().max(9.);
    let small_glyph = super::glyph_width(font) * small / font.size;
    let symbol_size = super::scaled(font, 12.).max(10.);
    let underline_w = (symbol_size * 0.75).round().max(6.);
    let underline_bottom = ((line_h - symbol_size) * 0.5 - 2.5).max(0.);
    let dot_gap = 3.;
    let group_gap = 4.;
    let text_width = |text: &str| (UnicodeWidthStr::width(text) as f32 * small_glyph).ceil() + 2.;

    let mut used = 0.;
    let mut tab_part = None;
    if show_tabs {
        let label = summary.tab_count.to_string();
        let label_width = text_width(&label);
        let attention = summary.other_tabs_attention;
        let attention_width = if attention.is_some() {
            symbol_size + dot_gap
        } else {
            0.
        };
        let total = label_width + attention_width;
        if total <= max_width {
            used += total;
            tab_part = Some((label, label_width, attention));
        }
    }

    let mut dots_part = None;
    if show_dots {
        let prefix = if tab_part.is_some() { group_gap } else { 0. };
        let budget = (max_width - used - prefix).max(0.);
        let total_panes = summary.pane_dots.len();
        let all_width =
            total_panes as f32 * symbol_size + total_panes.saturating_sub(1) as f32 * dot_gap;
        if total_panes <= MAX_VISIBLE_DOTS && all_width <= budget {
            used += prefix + all_width;
            dots_part = Some((total_panes, 0usize, 0.));
        } else {
            let max_visible = MAX_VISIBLE_DOTS.min(total_panes.saturating_sub(1));
            let mut chosen = None;
            for visible in (0..=max_visible).rev() {
                let overflow = total_panes - visible;
                let overflow_text = format!("+{overflow}");
                let overflow_width = text_width(&overflow_text);
                let dots_width = if visible > 0 {
                    visible as f32 * symbol_size + visible as f32 * dot_gap
                } else {
                    0.
                };
                if dots_width + overflow_width <= budget {
                    chosen = Some((
                        visible,
                        overflow,
                        overflow_width,
                        dots_width + overflow_width,
                    ));
                    break;
                }
            }
            if let Some((visible, overflow, overflow_width, width)) = chosen {
                used += prefix + width;
                dots_part = Some((visible, overflow, overflow_width));
            }
        }
    }

    if tab_part.is_none() && dots_part.is_none() {
        return None;
    }

    let strip_selector = format!("pane-strip-{workspace_id}");
    let mut strip = div()
        .debug_selector(move || strip_selector)
        .w(px(used))
        .h(px(line_h))
        .flex_none()
        .row()
        .items_center()
        .justify_end()
        .overflow_hidden();

    let has_tabs = tab_part.is_some();
    if let Some((label, label_width, attention)) = tab_part {
        let tab_color = match attention {
            Some(AgentStatus::Blocked) => pane_badge_color(AgentStatus::Blocked, theme),
            Some(AgentStatus::Done) => pane_badge_color(AgentStatus::Done, theme),
            _ => theme.muted,
        };
        let tab_selector = format!("workspace-tab-count-{workspace_id}");
        let mut group = div()
            .flex_none()
            .row()
            .items_center()
            .gap(px(dot_gap))
            .child(
                div()
                    .debug_selector(move || tab_selector)
                    .w(px(label_width))
                    .flex_none()
                    .text_size(px(small))
                    .text_color(rgb(tab_color))
                    .child(super::label_text(&label)),
            );
        if let Some(status) = attention {
            let attention_selector = format!("other-tabs-attention-{workspace_id}");
            let (_, path) = pane_symbol_asset(status);
            group = group.child(
                div()
                    .debug_selector(move || attention_selector)
                    .size(px(symbol_size))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(path)
                            .size(px(symbol_size))
                            .flex_none()
                            .text_color(rgb(pane_badge_color(status, theme))),
                    ),
            );
        }
        strip = strip.child(group);
    }

    if let Some((visible, overflow, overflow_width)) = dots_part {
        let mut group = div()
            .flex_none()
            .row()
            .items_center()
            .gap(px(dot_gap))
            .when(has_tabs, |group| group.ms(px(group_gap)));
        for (index, dot) in summary.pane_dots.iter().take(visible).enumerate() {
            let dot_status = dot.status.unwrap_or(AgentStatus::Unknown);
            let color = pane_badge_color(dot_status, theme);
            let status_name = super::super::agents::status_key(dot_status);
            let dot_selector = format!("pane-dot-{workspace_id}-{index}");
            let element = div()
                .debug_selector(move || dot_selector)
                .relative()
                .w(px(symbol_size))
                .h(px(line_h))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(pane_dot_icon(
                    workspace_id,
                    index,
                    dot.agent,
                    status_name,
                    symbol_size,
                    color,
                ))
                .when(dot.winner, |slot| {
                    let winner_selector = format!("pane-dot-winner-{workspace_id}-{index}");
                    slot.child(
                        div()
                            .debug_selector(move || winner_selector)
                            .absolute()
                            .bottom(px(underline_bottom))
                            .w(px(underline_w))
                            .h(px(1.5))
                            .rounded_full()
                            .bg(rgb(color)),
                    )
                });
            group = group.child(element);
        }
        if overflow > 0 {
            let text = format!("+{overflow}");
            let overflow_selector = format!("pane-overflow-{workspace_id}");
            group = group.child(
                div()
                    .debug_selector(move || overflow_selector)
                    .w(px(overflow_width))
                    .flex_none()
                    .text_size(px(small))
                    .text_color(rgb(theme.muted))
                    .child(super::label_text(&text)),
            );
        }
        strip = strip.child(group);
    }

    Some((used, strip))
}

/// Winning agent name + live status message (or fallback state label), placed
/// directly inside a workspace row or card.
pub(in crate::sidebar) fn workspace_status_line(
    workspace_id: &str,
    status: &WorkspaceStatus<'_>,
    width: f32,
    font_size: f32,
    line_h: f32,
    cx: &RowContext<'_>,
) -> Div {
    let font = cx.font;
    let theme = cx.theme;
    let glyph = super::glyph_width(font) * font_size / font.size;
    let agent_width = ((UnicodeWidthStr::width(status.agent) as f32 * glyph).ceil() + 2.)
        .min((width * 0.4).max(0.));
    let sep_width = (3. * glyph).ceil().min((width - agent_width).max(0.));
    let message_width = (width - agent_width - sep_width).max(0.);
    let selector = format!("workspace-status-{workspace_id}");
    let message_color = if status.is_message {
        theme.foreground
    } else {
        theme.subtext()
    };

    div()
        .debug_selector(move || selector)
        .w(px(width))
        .h(px(line_h))
        .flex_none()
        .row()
        .items_center()
        .overflow_hidden()
        .text_size(px(font_size))
        .child(
            div()
                .w(px(agent_width))
                .flex_none()
                .overflow_hidden()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme.primary()))
                .child(super::label_text(status.agent)),
        )
        .when(sep_width > 0., |line| {
            line.child(
                div()
                    .w(px(sep_width))
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(" · "),
            )
        })
        .when(message_width > 0., |line| {
            let text: Cow<'_, str> = if status.truncated {
                Cow::Owned(format!("{}…", status.text))
            } else {
                Cow::Borrowed(status.text.as_ref())
            };
            line.child(
                div()
                    .w(px(message_width))
                    .flex_none()
                    .overflow_hidden()
                    .truncate()
                    .text_color(rgb(message_color))
                    .child(super::label_text(&text)),
            )
        })
}

/// Optional inline agent mark + name prefix rendered on line 1 of an agent's
/// status block (`[icon] agent · <status line 1>`), matching `spaces`.
pub(in crate::sidebar) struct AgentInlineHeader {
    pub(in crate::sidebar) show_icon: bool,
    pub(in crate::sidebar) icon_size: f32,
    pub(in crate::sidebar) icon_reserve: f32,
    pub(in crate::sidebar) name_selector: String,
}

/// Agent status message preview (or localized fallback status on capable hosts),
/// placed directly inside the agent row or card so it shares its fill and border.
/// Always occupies [`AGENT_STATUS_LINES`] lines with automatic wrapping and
/// 2-line clamping so agent rows keep uniform height and long text never clips.
/// When `header` is `Some`, line 1 begins with `[icon] <agent> · ` just like
/// `spaces` while line 2 wraps across the full width beneath it.
pub(in crate::sidebar) fn agent_status_line(
    row: &AgentRow<'_>,
    header: Option<AgentInlineHeader>,
    width: f32,
    font_size: f32,
    line_h: f32,
    cx: &RowContext<'_>,
    window_cx: Option<&mut Context<SheprdWindow>>,
) -> Option<Div> {
    let font = cx.font;
    let theme = cx.theme;
    let pane_id = row.key.strip_prefix("agent-").unwrap_or(&row.key);
    let glyph = super::glyph_width(font) * font_size / font.size;
    let block_h = line_h * AGENT_STATUS_LINES as f32;

    let (text, text_color, selector, show_recap) = if let Some(message) = row.message {
        let display: SharedString = if row.message_truncated {
            SharedString::from(format!("{message}…"))
        } else {
            SharedString::from(message.to_owned())
        };
        (
            display,
            theme.foreground,
            format!("agent-message-{pane_id}"),
            row.idle_recap,
        )
    } else {
        let fallback = row.fallback_status.as_ref()?;
        (
            fallback.clone(),
            theme.subtext(),
            format!("agent-status-fallback-{pane_id}"),
            false,
        )
    };

    let (prefix_el, prefix_width) = match header {
        Some(header) => {
            let icon_reserve = if header.show_icon {
                header.icon_reserve.min(width.max(0.))
            } else {
                0.
            };
            let max_agent_w = ((width - icon_reserve) * 0.4).max(0.);
            let agent_width =
                ((UnicodeWidthStr::width(row.name) as f32 * glyph).ceil() + 2.).min(max_agent_w);
            let sep_width = (3. * glyph)
                .ceil()
                .min((width - icon_reserve - agent_width).max(0.));
            let prefix_w = icon_reserve + agent_width + sep_width;
            let icon_selector = format!("agent-icon-{}", row.key);
            let name_selector = header.name_selector;
            let prefix = div()
                .absolute()
                .top_0()
                .inset_s(px(0.))
                .w(px(prefix_w))
                .h(px(line_h))
                .flex_none()
                .row()
                .items_center()
                .overflow_hidden()
                .text_size(px(font_size))
                .line_height(px(line_h))
                .when(header.show_icon && icon_reserve > 0., |line| {
                    line.child(
                        div()
                            .debug_selector(move || icon_selector)
                            .w(px(icon_reserve))
                            .h(px(line_h))
                            .flex_none()
                            .flex()
                            .items_center()
                            .child(row.icon.element(header.icon_size, theme.primary())),
                    )
                })
                .child(
                    div()
                        .debug_selector(move || name_selector)
                        .w(px(agent_width))
                        .flex_none()
                        .overflow_hidden()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme.primary()))
                        .child(super::label_text(row.name)),
                )
                .when(sep_width > 0., |line| {
                    line.child(
                        div()
                            .w(px(sep_width))
                            .flex_none()
                            .text_color(rgb(theme.muted))
                            .child(" · "),
                    )
                });
            (Some(prefix), prefix_w)
        }
        None => (None, 0.),
    };

    let recap_label = show_recap.then(|| fl!("sidebar-agent-recap-available"));
    let pill_font = (font_size * 0.85).round().max(9.);
    let pill_glyph = super::glyph_width(font) * pill_font / font.size;
    let pill_pad = 4.;
    let pill_gap = 4.;
    let pill_width = recap_label
        .as_deref()
        .map(|label| {
            (UnicodeWidthStr::width(label) as f32 * pill_glyph).ceil() + pill_pad * 2. + 2.
        })
        .unwrap_or(0.);
    let show_pill = recap_label.is_some() && width >= prefix_width + pill_width + pill_gap;
    let trailing_reserve = if show_pill { pill_width + pill_gap } else { 0. };

    let recap_pill = recap_label.filter(|_| show_pill).map(|label| {
        let recap_selector = format!("agent-recap-{}", row.key);
        let pill = div()
            .debug_selector(move || recap_selector)
            .w(px(pill_width))
            .h(px(line_h))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .px(px(pill_pad))
            .rounded(px(3.))
            .bg(rgba((theme.foreground << 8) | 0x12))
            .text_size(px(pill_font))
            .text_color(rgb(theme.subtext()))
            .child(label);
        let inner = match (row.idle_recap, row.recap_target.as_ref(), window_cx) {
            (true, Some(target), Some(window_cx)) => {
                let click_target = target.clone();
                let key_target = target.clone();
                let button_id = SharedString::from(format!("agent-recap-hit-{pane_id}"));
                pill.id(button_id)
                    .role(Role::Button)
                    .aria_label(fl!("sidebar-agent-recap-open", pane = target.1.clone()))
                    .tab_index(0)
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.9))
                    .on_click(window_cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.open_agent_recap(&click_target.0, &click_target.1, window, cx);
                    }))
                    .on_key_down(window_cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                cx.stop_propagation();
                                window.prevent_default();
                                this.open_agent_recap(&key_target.0, &key_target.1, window, cx);
                            }
                        },
                    ))
                    .into_any_element()
            }
            _ => pill.into_any_element(),
        };
        div()
            .absolute()
            .top_0()
            .inset_e(px(0.))
            .flex_none()
            .child(inner)
    });

    Some(
        div()
            .debug_selector(move || selector)
            .relative()
            .w(px(width))
            .h(px(block_h))
            .flex_none()
            .overflow_hidden()
            .child(WrappedStatusText {
                text,
                first_line_indent: px(prefix_width),
                first_line_trailing: px(trailing_reserve),
                font_size: px(font_size),
                line_height: px(line_h),
                color: text_color,
            })
            .children(prefix_el)
            .children(recap_pill),
    )
}
