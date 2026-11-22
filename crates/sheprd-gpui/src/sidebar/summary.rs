#[cfg(test)]
#[path = "summary/tests.rs"]
mod tests;
use super::status_priority;
use crate::icons::AgentIcon;
use sheprd_client::protocol::{
    AgentStatus, ClientShellAgent, ClientShellSnapshot, endpoint::EndpointAgentStatusMessages,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Prepared {
    snapshot: Arc<ClientShellSnapshot>,
    messages: Option<Arc<EndpointAgentStatusMessages>>,
    workspace_by_id: Vec<usize>,
    workspaces: Vec<WorkspaceSummary>,
    agent_by_pane: Vec<usize>,
    agents: Vec<AgentPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceSummary {
    pub(crate) workspace_index: usize,
    pub(crate) tab_count: usize,
    pub(crate) pane_dots: Vec<PaneDot>,
    pub(crate) winner: Option<Winner>,
    pub(crate) other_tabs_attention: Option<AgentStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaneDot {
    pub(crate) pane_index: usize,
    pub(crate) status: Option<AgentStatus>,
    pub(crate) agent: Option<AgentIcon>,
    pub(crate) winner: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Winner {
    pub(crate) agent_index: usize,
    pub(crate) status: AgentStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentPreview {
    pub(crate) agent_index: usize,
    preview: Option<MessagePreview>,
    pub(crate) idle_recap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MessagePreview {
    end: usize,
    truncated: bool,
}

impl Prepared {
    pub(crate) fn new(
        snapshot: Arc<ClientShellSnapshot>,
        messages: Option<Arc<EndpointAgentStatusMessages>>,
    ) -> Self {
        let messages = messages.filter(|messages| {
            messages.boot_id == snapshot.boot_id && messages.revision == snapshot.revision
        });

        let mut workspace_indices = HashMap::with_capacity(snapshot.workspaces.len());
        for (index, workspace) in snapshot.workspaces.iter().enumerate() {
            workspace_indices
                .entry(workspace.workspace_id.as_str())
                .or_insert(index);
        }
        let mut pane_order = HashMap::with_capacity(snapshot.panes.len());
        for (index, pane) in snapshot.panes.iter().enumerate() {
            pane_order.entry(pane.pane_id.as_str()).or_insert(index);
        }
        let mut agents_by_pane = HashMap::with_capacity(snapshot.agents.len());
        for (index, agent) in snapshot.agents.iter().enumerate() {
            agents_by_pane
                .entry(agent.pane_id.as_str())
                .or_insert(index);
        }

        let mut tab_counts = vec![0; snapshot.workspaces.len()];
        for tab in &snapshot.tabs {
            if let Some(index) = workspace_indices.get(tab.workspace_id.as_str()) {
                tab_counts[*index] += 1;
            }
        }
        let mut workspaces: Vec<_> = snapshot
            .workspaces
            .iter()
            .enumerate()
            .map(|(workspace_index, _)| WorkspaceSummary {
                workspace_index,
                tab_count: tab_counts[workspace_index],
                pane_dots: Vec::new(),
                winner: None,
                other_tabs_attention: None,
            })
            .collect();

        for (pane_index, pane) in snapshot.panes.iter().enumerate() {
            let Some(workspace_index) = workspace_indices.get(pane.workspace_id.as_str()) else {
                continue;
            };
            let agent = agents_by_pane
                .get(pane.pane_id.as_str())
                .map(|index| &snapshot.agents[*index]);
            let status = agent.map(|agent| agent.agent_status);
            let agent = agent.map(|agent| AgentIcon::from_identity(agent.agent.as_deref()));
            workspaces[*workspace_index].pane_dots.push(PaneDot {
                pane_index,
                status,
                agent,
                winner: false,
            });
        }

        let mut winners = vec![None; snapshot.workspaces.len()];
        for (agent_index, agent) in snapshot.agents.iter().enumerate() {
            let Some(workspace_index) = workspace_indices.get(agent.workspace_id.as_str()).copied()
            else {
                continue;
            };
            let workspace = &snapshot.workspaces[workspace_index];
            let summary = &mut workspaces[workspace_index];

            if agent.tab_id != workspace.active_tab_id
                && summary.other_tabs_attention.is_none_or(|current| {
                    status_priority(agent.agent_status) > status_priority(current)
                })
            {
                summary.other_tabs_attention = Some(agent.agent_status);
            }

            if winners[workspace_index].is_none_or(|current_index| {
                agent_outranks(
                    agent,
                    &snapshot.agents[current_index],
                    workspace,
                    &pane_order,
                )
            }) {
                winners[workspace_index] = Some(agent_index);
            }
        }

        for (workspace_index, summary) in workspaces.iter_mut().enumerate() {
            let Some(agent_index) = winners[workspace_index] else {
                continue;
            };
            let agent = &snapshot.agents[agent_index];
            summary.winner = Some(Winner {
                agent_index,
                status: agent.agent_status,
            });
            if let Some(pane_index) = pane_order.get(agent.pane_id.as_str()) {
                if let Some(dot) = summary
                    .pane_dots
                    .iter_mut()
                    .find(|dot| dot.pane_index == *pane_index)
                {
                    dot.winner = true;
                }
            }
        }

        let agents: Vec<_> = snapshot
            .agents
            .iter()
            .enumerate()
            .map(|(agent_index, agent)| {
                let preview = messages
                    .as_ref()
                    .and_then(|messages| messages.messages.get(&agent.pane_id))
                    .map(|message| preview_range(message));
                AgentPreview {
                    agent_index,
                    idle_recap: agent.agent_status == AgentStatus::Idle && preview.is_some(),
                    preview,
                }
            })
            .collect();

        let mut workspace_by_id: Vec<_> = (0..snapshot.workspaces.len()).collect();
        workspace_by_id.sort_unstable_by(|left, right| {
            snapshot.workspaces[*left]
                .workspace_id
                .cmp(&snapshot.workspaces[*right].workspace_id)
                .then_with(|| left.cmp(right))
        });
        let mut agent_by_pane: Vec<_> = (0..snapshot.agents.len()).collect();
        agent_by_pane.sort_unstable_by(|left, right| {
            snapshot.agents[*left]
                .pane_id
                .cmp(&snapshot.agents[*right].pane_id)
                .then_with(|| left.cmp(right))
        });

        Self {
            snapshot,
            messages,
            workspace_by_id,
            workspaces,
            agent_by_pane,
            agents,
        }
    }

    pub(crate) fn workspace(&self, id: &str) -> Option<&WorkspaceSummary> {
        let position = self.workspace_by_id.binary_search_by(|index| {
            self.snapshot.workspaces[*index]
                .workspace_id
                .as_str()
                .cmp(id)
        });
        position
            .ok()
            .and_then(|position| self.workspaces.get(self.workspace_by_id[position]))
    }

    pub(crate) fn agent(&self, pane: &str) -> Option<&AgentPreview> {
        let position = self
            .agent_by_pane
            .binary_search_by(|index| self.snapshot.agents[*index].pane_id.as_str().cmp(pane));
        position
            .ok()
            .and_then(|position| self.agents.get(self.agent_by_pane[position]))
    }

    pub(crate) fn agent_workspace_id(&self, pane: &str) -> Option<&str> {
        let agent = self.agent(pane)?;
        let workspace = self
            .snapshot
            .agents
            .get(agent.agent_index)?
            .workspace_id
            .as_str();
        (!workspace.is_empty()).then_some(workspace)
    }

    pub(crate) fn preview(&self, pane: &str) -> Option<(&str, bool)> {
        let preview = self.agent(pane)?.preview?;
        let message = self.messages.as_ref()?.messages.get(pane)?;
        Some((&message[..preview.end], preview.truncated))
    }

    pub(crate) fn full_message(&self, pane: &str) -> Option<&str> {
        let agent = self.agent(pane)?;
        self.messages
            .as_ref()?
            .messages
            .get(&self.snapshot.agents[agent.agent_index].pane_id)
            .map(String::as_str)
    }
}

fn preview_range(message: &str) -> MessagePreview {
    let end = message
        .char_indices()
        .nth(200)
        .map_or(message.len(), |(index, _)| index);
    MessagePreview {
        end,
        truncated: end < message.len(),
    }
}

fn agent_outranks(
    candidate: &ClientShellAgent,
    current: &ClientShellAgent,
    workspace: &sheprd_client::protocol::ClientShellWorkspace,
    pane_order: &HashMap<&str, usize>,
) -> bool {
    let candidate_priority = status_priority(candidate.agent_status);
    let current_priority = status_priority(current.agent_status);
    if candidate_priority != current_priority {
        return candidate_priority > current_priority;
    }
    if candidate.focused != current.focused {
        return candidate.focused;
    }
    let candidate_active = candidate.tab_id == workspace.active_tab_id;
    let current_active = current.tab_id == workspace.active_tab_id;
    if candidate_active != current_active {
        return candidate_active;
    }
    let candidate_pane_index = pane_order
        .get(candidate.pane_id.as_str())
        .copied()
        .unwrap_or(usize::MAX);
    let current_pane_index = pane_order
        .get(current.pane_id.as_str())
        .copied()
        .unwrap_or(usize::MAX);
    if candidate_pane_index != current_pane_index {
        return candidate_pane_index < current_pane_index;
    }
    candidate.pane_id < current.pane_id
}
