//! Sidebar rows as swappable layouts.
//!
//! Render prepares what a row shows once, as typed data, and hands it to the
//! configured [`RowLayout`] through a [`Cell`]. The layout decides which of
//! those pieces appear and where: one can name the host and branch, another
//! only the name. The cell carries the row's state, so callers write
//! `cell.selected(focused).highlighted(menu_open).row()` and each layout
//! paints those states its own way.
//!
//! Rows are built and dropped within one render of the cached sidebar view, so
//! everything here borrows from the window rather than copying into owned
//! state: a layout costs one virtual call per row.

use super::{
    layout::SidebarLook,
    row::{RowBadge, RowIcon, RowLift, RowTree, Upstream},
};
use crate::{
    config::{FontConfig, LayoutMode, Theme},
    icons::AgentIcon,
    window::SheprdWindow,
};
use gpui::{App, ClickEvent, Context, Div, ElementId, ParentElement, Styled, Window};
use sheprd_client::protocol::AgentStatus;
use sheprd_session::Workspace;
use std::borrow::Cow;
use std::sync::Arc;

/// Read-only inputs every row of one render shares.
pub(super) struct RowContext<'a> {
    pub(super) indicators: super::agents::Indicators,
    pub(super) font: &'a FontConfig,
    pub(super) theme: &'a Theme,
    /// Density and style: spacing, highlight shape, and which details show.
    pub(super) look: SidebarLook,
    /// Full sidebar width, divider included.
    pub(super) width: f32,
    /// How far workspace rows step in under their host's header: zero while
    /// a single host shows no header to nest under.
    pub(super) nest: f32,
    /// The host's colour wash and dimming, shared by every row it owns.
    pub(super) mark: super::wash::HostMark,
    /// The host these rows live on, named only while several hosts are
    /// listed and this one is remote, so a single-host sidebar stays quiet.
    pub(super) host: Option<&'a str>,
}

impl RowContext<'_> {
    /// The indicators for a status mark in one of these rows: faded with
    /// the row's own fade and the host's dimming, which a mark drawn in the
    /// window's mark layer does not inherit.
    pub(super) fn status_indicators(&self) -> super::agents::Indicators {
        self.indicators
            .faded(self.indicators.fade() * self.mark.opacity())
    }
}

/// A click listener, erased so rows of every group share one type.
pub(super) type ClickHandler = dyn Fn(&ClickEvent, &mut Window, &mut App);

/// A repository's fold control. The layout draws it; render owns what
/// clicking it changes.
pub(super) struct Fold {
    pub(super) id: ElementId,
    /// Position of the row in its host's workspace list, for probes.
    pub(super) index: usize,
    pub(super) collapsed: bool,
    pub(super) toggle: Box<ClickHandler>,
}

/// The winning agent's status line shown inside an expanded workspace row.
pub(super) struct WorkspaceStatus<'a> {
    pub(super) agent: &'a str,
    pub(super) text: gpui::SharedString,
    pub(super) truncated: bool,
    pub(super) is_message: bool,
}

pub(super) struct WorkspaceRow<'a> {
    pub(super) workspace: &'a Workspace,
    /// What the row is called: a child's branch unless the user renamed it.
    pub(super) label: &'a str,
    pub(super) tree: RowTree,
    pub(super) icon: RowIcon,
    pub(super) fold: Option<Fold>,
    /// Whether the row belongs to a worktree group, which lines its badges up
    /// with the group's fold control.
    pub(super) grouped: bool,
    pub(super) badge: Option<RowBadge>,
    /// The checkout is being deleted.
    pub(super) removing: bool,
    /// Status the row shows. A collapsed group's head takes its most urgent member.
    pub(super) status: AgentStatus,
    /// Whether this workspace has at least one associated agent.
    pub(super) has_agents: bool,
    /// Daemon token rows, painted in the layout's own frame. Empty keeps its native lines.
    pub(super) lines: Vec<Vec<super::tokens::ResolvedToken>>,
    /// Prepared fork-only pane state; `pane_strip` controls its display.
    pub(super) summary: Option<&'a super::summary::WorkspaceSummary>,
    pub(super) pane_strip: bool,
    pub(super) summary_status: Option<WorkspaceStatus<'a>>,
}

impl<'a> WorkspaceRow<'a> {
    /// The checked-out branch, if Git reported a non-blank one.
    pub(super) fn branch(&self) -> Option<&'a str> {
        self.workspace
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|branch| !branch.is_empty())
    }

    /// Commits to push and pull against the branch's upstream, if any.
    pub(super) fn upstream(&self) -> Option<Upstream> {
        Upstream::new(self.workspace.ahead_behind)
    }

    pub(super) fn status(&self) -> AgentStatus {
        self.status
    }
}

pub(super) struct AgentRow<'a> {
    /// Stable probe key, unique across both lists.
    pub(super) key: String,
    pub(super) name: &'a str,
    pub(super) icon: AgentIcon,
    pub(super) status: AgentStatus,
    /// Its workspace and, when that earns a place, its tab. Missing once the
    /// workspace has gone.
    pub(super) place: Option<(&'a str, Option<&'a str>)>,
    /// Its pane's name: Rename's label, else the pane's title, and never the
    /// agent name again. Safe, bounded text.
    pub(super) pane: Option<String>,
    /// The daemon's `state_text` word when its sidebar config asks for it, so
    /// the GUI and the TUI name the status the same way: the agent's own
    /// state label when its integration set one.
    pub(super) status_text: Option<Cow<'a, str>>,
    /// Daemon token rows, painted in the layout's own frame. Empty keeps its native lines.
    pub(super) lines: Vec<Vec<super::tokens::ResolvedToken>>,
    /// Whether the agent belongs to a capable Herdr 1 host with agent sidebar
    /// presentation enabled.
    pub(super) agent_sidebar: bool,
    /// Bounded preview of the matching pane's last agent message.
    pub(super) message: Option<&'a str>,
    pub(super) message_truncated: bool,
    pub(super) idle_recap: bool,
    pub(super) recap_target: Option<Arc<(String, String)>>,
    /// Localized status label shown on the status line when a capable host has
    /// no live message for this pane.
    pub(super) fallback_status: Option<gpui::SharedString>,
}

/// What a row shows.
pub(super) enum RowData<'a> {
    Workspace(WorkspaceRow<'a>),
    Agent(AgentRow<'a>),
}

/// How a row is marked. Hovering needs no state: layouts style it through
/// the row's hover group, so it costs nothing per frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RowState {
    /// The focused workspace or agent.
    pub(super) selected: bool,
    /// A row related to the selected agent workspace, without claiming focus.
    pub(super) linked: bool,
    /// Marked as if hovered while the pointer is elsewhere, such as the row a
    /// menu was opened for.
    pub(super) highlighted: bool,
    /// Whether a workspace is being dragged, and whether this is the row it
    /// carries. Rows the carried one passes stop answering hover; the carried
    /// row floats over them, so it must paint an opaque, lifted surface.
    pub(super) lift: RowLift,
}

/// Color tone of the leading activation bar on a focused sidebar row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sidebar) enum FocusBarTone {
    /// Focused agent row, or focused workspace with at least one agent.
    Active,
    /// Focused workspace with no associated agents.
    Neutral,
}

/// Which focused sidebar row carries the leading activation bar.
#[derive(Clone, Copy)]
pub(super) enum FocusBarTarget<'a> {
    Workspace { id: &'a str, tone: FocusBarTone },
    Agent(&'a str),
}

impl FocusBarTarget<'_> {
    pub(super) fn selector(self) -> String {
        match self {
            Self::Workspace {
                id,
                tone: FocusBarTone::Active,
            } => format!("workspace-focus-bar-{id}"),
            Self::Workspace {
                id,
                tone: FocusBarTone::Neutral,
            } => format!("workspace-focus-bar-neutral-{id}"),
            Self::Agent(pane_id) => format!("agent-focus-bar-{pane_id}"),
        }
    }

    pub(super) fn tone(self) -> FocusBarTone {
        match self {
            Self::Workspace { tone, .. } => tone,
            Self::Agent(_) => FocusBarTone::Active,
        }
    }
}

/// Arranges a row's content and marks its state. Implementations are
/// stateless and read only what they are given, so render stays free of I/O
/// and allocation beyond the elements themselves. How a dragged row looks is
/// part of the layout too: `state.lift` says which row is carried.
pub(super) trait RowLayout {
    fn workspace(&self, row: WorkspaceRow<'_>, state: RowState, cx: &RowContext<'_>) -> Div;
    fn agent(
        &self,
        row: AgentRow<'_>,
        state: RowState,
        cx: &RowContext<'_>,
        window_cx: Option<&mut Context<SheprdWindow>>,
    ) -> Div;
    /// Leading activation bar for the focused workspace or agent row, shaped
    /// to match the layout's highlight frame.
    fn focus_bar(&self, target: FocusBarTarget<'_>, cx: &RowContext<'_>) -> Div {
        super::row::agent_sidebar::focus_bar(
            target.selector(),
            target.tone(),
            cx.look.inset(),
            cx.look.spacing() / 2.,
            cx.look.density.radius,
            cx.theme,
        )
    }
    /// What carries a worktree child's tree guide on through the lines under
    /// its row (note, ports) to the next sibling. Only layouts that draw
    /// guides have one to continue.
    fn trunk_below(&self, _key: &str, _tree: RowTree, _cx: &RowContext<'_>) -> Option<Div> {
        None
    }
}

/// One row waiting for its state: build it, mark it, then take the element.
#[must_use = "a cell draws nothing until `row` takes its element"]
pub(super) struct Cell<'a> {
    layout: &'a dyn RowLayout,
    data: RowData<'a>,
    state: RowState,
    cx: &'a RowContext<'a>,
}

impl<'a> Cell<'a> {
    pub(super) fn new(
        layout: &'a dyn RowLayout,
        data: RowData<'a>,
        cx: &'a RowContext<'a>,
    ) -> Self {
        Self {
            layout,
            data,
            state: RowState::default(),
            cx,
        }
    }

    pub(super) fn selected(mut self, selected: bool) -> Self {
        self.state.selected = selected;
        self
    }

    pub(super) fn linked(mut self, linked: bool) -> Self {
        self.state.linked = linked;
        self
    }

    pub(super) fn highlighted(mut self, highlighted: bool) -> Self {
        self.state.highlighted = highlighted;
        self
    }

    pub(super) fn lift(mut self, lift: RowLift) -> Self {
        self.state.lift = lift;
        self
    }

    /// The row element, laid out for the state the cell was given. The rail
    /// style's bar is added here, on the row's leading edge, so every layout
    /// shows it in the same place without drawing it itself.
    #[cfg(test)]
    pub(super) fn row(self) -> Div {
        self.render(None)
    }

    pub(super) fn interactive_row(self, cx: &mut Context<SheprdWindow>) -> Div {
        self.render(Some(cx))
    }

    fn render(self, window_cx: Option<&mut Context<SheprdWindow>>) -> Div {
        let (row, rail, focus_bar) = match self.data {
            RowData::Workspace(row) => {
                let rail = (!row.removing)
                    .then(|| self.cx.indicators.rail(row.status, row.label))
                    .flatten();
                let focus_bar = (self.state.selected && !row.removing).then(|| {
                    let tone = if row.has_agents {
                        FocusBarTone::Active
                    } else {
                        FocusBarTone::Neutral
                    };
                    self.layout.focus_bar(
                        FocusBarTarget::Workspace {
                            id: &row.workspace.id,
                            tone,
                        },
                        self.cx,
                    )
                });
                (
                    self.layout.workspace(row, self.state, self.cx),
                    rail,
                    focus_bar,
                )
            }
            RowData::Agent(row) => {
                let rail = self.cx.indicators.rail(row.status, &row.key);
                let focus_bar = self.state.selected.then(|| {
                    let pane_id = row.key.strip_prefix("agent-").unwrap_or(&row.key);
                    self.layout
                        .focus_bar(FocusBarTarget::Agent(pane_id), self.cx)
                });
                (
                    self.layout.agent(row, self.state, self.cx, window_cx),
                    rail,
                    focus_bar,
                )
            }
        };
        let row = match rail {
            Some(rail) => row.relative().child(rail),
            None => row,
        };
        match focus_bar {
            Some(bar) => row.relative().child(bar),
            None => row,
        }
    }
}

/// The rows a layout draws: Herdr's for every density, or a design of its own.
pub(super) fn layout_for(mode: LayoutMode) -> &'static dyn RowLayout {
    match mode {
        LayoutMode::Classic { .. } => &super::layouts::Herdr,
        LayoutMode::Superset => &super::layouts::Superset,
        LayoutMode::Orca => &super::layouts::Orca,
        LayoutMode::Minimal => &super::layouts::Minimal,
        // Herdr's own rows, nested under each device's header.
        LayoutMode::Devices => &super::layouts::Herdr,
    }
}
