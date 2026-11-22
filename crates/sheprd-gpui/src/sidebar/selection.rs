use crate::{SheprdWindow, endpoint::Endpoint, state::LiveState};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    target: Option<Target>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) endpoint: String,
    pub(crate) generation: u64,
    pub(crate) boot: String,
    pub(crate) workspace: String,
}

impl Target {
    pub(crate) fn matches(
        &self,
        endpoint: &str,
        generation: u64,
        boot: &str,
        workspace: &str,
    ) -> bool {
        self.endpoint == endpoint
            && self.generation == generation
            && self.boot == boot
            && self.workspace == workspace
    }
}

impl Selection {
    pub(crate) fn target(&self) -> Option<&Target> {
        self.target.as_ref()
    }

    pub(crate) fn select(&mut self, target: Target) {
        if self.target.as_ref() != Some(&target) {
            self.target = Some(target);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.target = None;
    }

    #[cfg(test)]
    pub(crate) fn matches(
        &self,
        endpoint: &str,
        generation: u64,
        boot: &str,
        workspace: &str,
    ) -> bool {
        self.target
            .as_ref()
            .is_some_and(|target| target.matches(endpoint, generation, boot, workspace))
    }

    pub(crate) fn retain(&mut self, valid: impl FnOnce(&Target) -> bool) {
        if self.target.as_ref().is_some_and(|target| !valid(target)) {
            self.target = None;
        }
    }
}

impl SheprdWindow {
    /// The workspace linked for sidebar cross-highlighting: the explicit click
    /// selection when set, otherwise the selected capable host's focused
    /// workspace so spaces and agents start linked immediately.
    pub(crate) fn effective_sidebar_target(&self) -> Option<Target> {
        if let Some(target) = self.sidebar_selection.target() {
            return Some(target.clone());
        }
        if self.kept_in_front().is_some() {
            return None;
        }
        let endpoint = self.endpoints.get(self.selected_endpoint)?;
        if endpoint.generation != self.selected_generation {
            return None;
        }
        let snapshot = self.live.snapshot.as_ref()?;
        self.live.agent_sidebar.prepared()?;
        let workspace = snapshot
            .focused_workspace_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .or_else(|| {
                snapshot
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.focused)
                    .map(|workspace| workspace.workspace_id.as_str())
            })?;
        Some(Target {
            endpoint: endpoint.id.clone(),
            generation: endpoint.generation,
            boot: snapshot.boot_id.clone(),
            workspace: workspace.to_owned(),
        })
    }

    pub(crate) fn sidebar_agent_linked(&self, index: usize, pane: &str) -> bool {
        let Some(target) = self.effective_sidebar_target() else {
            return false;
        };
        let Some(endpoint) = self.endpoints.get(index) else {
            return false;
        };
        if index == self.selected_endpoint && endpoint.generation != self.selected_generation {
            return false;
        }
        let live = if index == self.selected_endpoint {
            &self.live
        } else {
            &endpoint.live
        };
        let Some(snapshot) = live.snapshot.as_ref() else {
            return false;
        };
        let Some(workspace) = live
            .agent_sidebar
            .prepared()
            .and_then(|prepared| prepared.agent_workspace_id(pane))
        else {
            return false;
        };
        target.matches(
            &endpoint.id,
            endpoint.generation,
            &snapshot.boot_id,
            workspace,
        )
    }

    pub(crate) fn select_sidebar_workspace(&mut self, endpoint_id: &str, workspace: &str) {
        if let Some(target) = self.sidebar_target_for_workspace(endpoint_id, workspace) {
            self.sidebar_selection.select(target);
        } else {
            self.sidebar_selection.clear();
        }
    }

    pub(crate) fn select_sidebar_agent(&mut self, endpoint_id: &str, pane: &str) {
        if let Some(target) = self.sidebar_target_for_agent(endpoint_id, pane) {
            self.sidebar_selection.select(target);
        } else {
            self.sidebar_selection.clear();
        }
    }

    fn sidebar_target_for_workspace(&self, endpoint_id: &str, workspace: &str) -> Option<Target> {
        let (endpoint, live) = self.sidebar_source(endpoint_id)?;
        let snapshot = live.snapshot.as_ref()?;
        live.agent_sidebar.prepared()?;
        snapshot
            .workspaces
            .iter()
            .any(|candidate| candidate.workspace_id == workspace)
            .then(|| Target {
                endpoint: endpoint.id.clone(),
                generation: endpoint.generation,
                boot: snapshot.boot_id.clone(),
                workspace: workspace.to_owned(),
            })
    }

    fn sidebar_target_for_agent(&self, endpoint_id: &str, pane: &str) -> Option<Target> {
        let (endpoint, live) = self.sidebar_source(endpoint_id)?;
        let snapshot = live.snapshot.as_ref()?;
        let workspace = live.agent_sidebar.prepared()?.agent_workspace_id(pane)?;
        snapshot
            .workspaces
            .iter()
            .any(|candidate| candidate.workspace_id == workspace)
            .then(|| Target {
                endpoint: endpoint.id.clone(),
                generation: endpoint.generation,
                boot: snapshot.boot_id.clone(),
                workspace: workspace.to_owned(),
            })
    }

    fn sidebar_source(&self, endpoint_id: &str) -> Option<(&Endpoint, &LiveState)> {
        let (index, endpoint) = self
            .endpoints
            .iter()
            .enumerate()
            .find(|(_, endpoint)| endpoint.id == endpoint_id)?;
        if index == self.selected_endpoint && endpoint.generation != self.selected_generation {
            return None;
        }
        let live = if index == self.selected_endpoint {
            &self.live
        } else {
            &endpoint.live
        };
        Some((endpoint, live))
    }
}

#[cfg(test)]
mod tests;
