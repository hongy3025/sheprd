use crate::SheprdWindow;
use sheprd_control_protocol::protocol::app::{
    AgentSidebarState, SidebarAgent, SidebarHost, SidebarPaneDot, SidebarSelection,
    SidebarWorkspace,
};

/// The CLI and hosted view share the same connection-fenced projection.
pub(crate) fn state(view: &SheprdWindow) -> AgentSidebarState {
    let selected = view
        .sidebar_selection
        .target()
        .map(|target| SidebarSelection {
            endpoint: target.endpoint.clone(),
            generation: target.generation,
            boot: target.boot.clone(),
            workspace: target.workspace.clone(),
        });
    let mut hosts = Vec::with_capacity(view.endpoints.len());
    for (index, endpoint) in view.endpoints.iter().enumerate() {
        let current =
            index != view.selected_endpoint || endpoint.generation == view.selected_generation;
        let live = if index == view.selected_endpoint {
            &view.live
        } else {
            &endpoint.live
        };
        let supported = current && live.agent_sidebar.is_supported();
        let (workspaces, agents) = if supported {
            match (live.snapshot.as_ref(), live.agent_sidebar.prepared()) {
                (Some(snapshot), Some(prepared)) => {
                    let workspaces = snapshot
                        .workspaces
                        .iter()
                        .map(|workspace| {
                            let summary = prepared.workspace(&workspace.workspace_id);
                            SidebarWorkspace {
                                id: workspace.workspace_id.clone(),
                                tab_count: summary.map_or(0, |summary| summary.tab_count),
                                pane_dots: summary.map_or_else(Vec::new, |summary| {
                                    summary
                                        .pane_dots
                                        .iter()
                                        .filter_map(|dot| {
                                            snapshot.panes.get(dot.pane_index).map(|pane| {
                                                SidebarPaneDot {
                                                    pane: pane.pane_id.clone(),
                                                    status: dot.status,
                                                    winner: dot.winner,
                                                }
                                            })
                                        })
                                        .collect()
                                }),
                                winner: summary
                                    .and_then(|summary| summary.winner.map(|winner| winner.status)),
                                other_tabs_attention: summary
                                    .and_then(|summary| summary.other_tabs_attention),
                            }
                        })
                        .collect();
                    let agents = snapshot
                        .agents
                        .iter()
                        .map(|agent| {
                            let preview = prepared
                                .preview(&agent.pane_id)
                                .map(|(message, _)| message.to_owned());
                            let idle_recap = prepared
                                .agent(&agent.pane_id)
                                .is_some_and(|summary| summary.idle_recap);
                            SidebarAgent {
                                pane: agent.pane_id.clone(),
                                workspace: agent.workspace_id.clone(),
                                status: agent.agent_status,
                                preview,
                                idle_recap,
                                focused: agent.focused,
                                linked: view.sidebar_selection.matches(
                                    &endpoint.id,
                                    endpoint.generation,
                                    &snapshot.boot_id,
                                    &agent.workspace_id,
                                ),
                            }
                        })
                        .collect();
                    (workspaces, agents)
                }
                _ => (Vec::new(), Vec::new()),
            }
        } else {
            (Vec::new(), Vec::new())
        };
        hosts.push(SidebarHost {
            id: endpoint.id.clone(),
            supported,
            workspaces,
            agents,
        });
    }
    AgentSidebarState { selected, hosts }
}
