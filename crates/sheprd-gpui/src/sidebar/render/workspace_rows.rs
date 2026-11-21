use crate::{
    config::SpaceLayout,
    sidebar::{
        agents::state_label,
        selection::Selection,
        status_text,
        summary::{Prepared, WorkspaceSummary},
    },
};
use gpui::SharedString;
use sheprd_client::protocol::{AgentStatus, ClientShellSnapshot};
use std::borrow::Cow;

pub(super) struct Extras<'a> {
    pub(super) linked: bool,
    pub(super) summary: Option<&'a WorkspaceSummary>,
    pub(super) status_text: Option<SharedString>,
}

pub(super) fn extras<'a>(
    prepared: Option<&'a Prepared>,
    snapshot: Option<&'a ClientShellSnapshot>,
    workspace_id: &str,
    layout: &SpaceLayout,
    selection: &Selection,
    endpoint_id: &str,
    generation: u64,
) -> Extras<'a> {
    let linked = snapshot.is_some_and(|snapshot| {
        selection.matches(endpoint_id, generation, &snapshot.boot_id, workspace_id)
    });
    let Some(summary) = prepared.and_then(|prepared| prepared.workspace(workspace_id)) else {
        return Extras {
            linked,
            summary: None,
            status_text: None,
        };
    };
    let status_text = layout.status_text.then(|| {
        summary.winner.and_then(|winner| {
            let agent = snapshot?.agents.get(winner.agent_index)?;
            let fallback = status_text(winner.status);
            let label = match state_label(agent, &fallback) {
                Cow::Borrowed(_) => fallback,
                Cow::Owned(label) => label,
            };
            Some(SharedString::from(label))
        })
    });
    Extras {
        linked,
        summary: Some(summary),
        status_text: status_text.flatten(),
    }
}

pub(super) fn accessible_name(
    label: &str,
    linked: bool,
    status: AgentStatus,
    muted: bool,
    host: Option<&str>,
    summary: Option<&WorkspaceSummary>,
    pane_strip: bool,
    summary_text: Option<&str>,
) -> String {
    crate::sidebar::accessibility::workspace_details_name(
        crate::sidebar::accessibility::workspace_name(label, linked, status, muted, host),
        summary
            .filter(|_| pane_strip)
            .map(|summary| (summary.tab_count, summary.pane_dots.len())),
        summary_text,
    )
}
