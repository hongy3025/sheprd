use serde::{Deserialize, Serialize};
use sheprd_client::protocol::AgentStatus;

pub const MAX_AGENT_SIDEBAR_PANE_ID_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarSelection {
    pub endpoint: String,
    pub generation: u64,
    pub boot: String,
    pub workspace: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarPaneDot {
    pub pane: String,
    pub status: Option<AgentStatus>,
    pub winner: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarWorkspace {
    pub id: String,
    pub tab_count: usize,
    pub pane_dots: Vec<SidebarPaneDot>,
    pub winner: Option<AgentStatus>,
    pub other_tabs_attention: Option<AgentStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarAgent {
    pub pane: String,
    pub workspace: String,
    pub status: AgentStatus,
    pub preview: Option<String>,
    pub idle_recap: bool,
    pub focused: bool,
    pub linked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarHost {
    pub id: String,
    pub supported: bool,
    pub workspaces: Vec<SidebarWorkspace>,
    pub agents: Vec<SidebarAgent>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSidebarState {
    pub selected: Option<SidebarSelection>,
    pub hosts: Vec<SidebarHost>,
}
