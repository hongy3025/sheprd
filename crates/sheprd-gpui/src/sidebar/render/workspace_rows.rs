use crate::{
    config::SpaceLayout,
    endpoint::Endpoint,
    sidebar::{
        agents::{agent_name, state_label},
        cell::WorkspaceStatus,
        selection::Target,
        status_text,
        summary::{Prepared, WorkspaceSummary},
    },
};
use gpui::{Div, SharedString};
use sheprd_client::protocol::{AgentStatus, ClientShellSnapshot};
use std::borrow::Cow;

pub(super) struct Extras<'a> {
    pub(super) linked: bool,
    pub(super) summary: Option<&'a WorkspaceSummary>,
    pub(super) status: Option<WorkspaceStatus<'a>>,
    pub(super) accessible_status: Option<String>,
}

impl Extras<'_> {
    pub(super) fn marker(&self, endpoint_id: &str, id: &str) -> Option<Div> {
        self.linked.then(|| {
            crate::sidebar::row::agent_sidebar::linked_marker("workspace", endpoint_id, id)
        })
    }

    pub(super) fn accessible_name(
        &self,
        label: &str,
        linked_worktree: bool,
        status: AgentStatus,
        muted: bool,
        host: Option<&str>,
        pane_strip: bool,
    ) -> String {
        crate::sidebar::accessibility::workspace_details_name(
            crate::sidebar::accessibility::workspace_name(
                label,
                linked_worktree,
                status,
                muted,
                host,
            ),
            self.summary
                .filter(|_| pane_strip)
                .map(|summary| (summary.tab_count, summary.pane_dots.len())),
            self.accessible_status.as_deref(),
        )
    }
}

pub(super) fn extras<'a>(
    prepared: Option<&'a Prepared>,
    snapshot: Option<&'a ClientShellSnapshot>,
    workspace_id: &str,
    layout: &SpaceLayout,
    selection: Option<&Target>,
    endpoint: &Endpoint,
) -> Extras<'a> {
    let linked = snapshot.is_some_and(|snapshot| {
        selection.is_some_and(|target| {
            target.matches(
                &endpoint.id,
                endpoint.generation,
                &snapshot.boot_id,
                workspace_id,
            )
        })
    });
    let Some((prepared, summary)) = prepared.and_then(|prepared| {
        prepared
            .workspace(workspace_id)
            .map(|summary| (prepared, summary))
    }) else {
        return Extras {
            linked,
            summary: None,
            status: None,
            accessible_status: None,
        };
    };
    let status = layout.status_text.then(|| {
        summary.winner.and_then(|winner| {
            let agent = snapshot?.agents.get(winner.agent_index)?;
            let name = agent_name(agent);
            let is_omp_idle = winner.status == AgentStatus::Idle
                && agent
                    .agent
                    .as_deref()
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("omp"));
            let use_message = matches!(
                winner.status,
                AgentStatus::Blocked | AgentStatus::Done | AgentStatus::Working
            ) || is_omp_idle;
            let preview = use_message
                .then(|| prepared.preview(&agent.pane_id))
                .flatten();
            let (text, truncated, is_message) = match preview {
                Some((message, truncated)) => {
                    (SharedString::from(message.to_owned()), truncated, true)
                }
                None => {
                    let fallback = status_text(winner.status);
                    let label = match state_label(agent, &fallback) {
                        Cow::Borrowed(_) => fallback,
                        Cow::Owned(label) => label,
                    };
                    (SharedString::from(label), false, false)
                }
            };
            Some(WorkspaceStatus {
                agent: name,
                text,
                truncated,
                is_message,
            })
        })
    });
    let status = status.flatten();
    let accessible_status = status
        .as_ref()
        .map(|status| format!("{} · {}", status.agent, status.text));
    Extras {
        linked,
        summary: Some(summary),
        status,
        accessible_status,
    }
}
