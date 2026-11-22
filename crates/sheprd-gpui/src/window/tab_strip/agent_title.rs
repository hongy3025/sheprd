//! What the strip calls a tab: the daemon's label, or, with
//! `[tab_labels] agent_title` on, the terminal title of the agent in a tab
//! that has no name of its own. Only the strip's own drawing reads this; the
//! rename and close dialogs keep the daemon's label.
use crate::SheprdWindow;
use gpui::SharedString;
use sheprd_client::protocol::{ClientShellAgent, ClientShellSnapshot, ClientShellTab};

/// Grapheme clusters kept of a title before it is cut to a tab's width. It
/// only bounds the work on untrusted text, so it sits well past that width.
const TITLE_BOUND: usize = 64;
/// Larger bound used before extracting a path's basename so long directory
/// prefixes do not truncate the final segment.
const PATH_BOUND: usize = 512;

impl SheprdWindow {
    /// The label the strip, its drag ghost, and its stand-in show for `tab`.
    pub(in crate::window) fn strip_label(
        &self,
        snapshot: &ClientShellSnapshot,
        tab: &ClientShellTab,
    ) -> SharedString {
        self.config
            .tab_labels
            .agent_title
            .then(|| title(snapshot, tab))
            .flatten()
            .unwrap_or_else(|| sheprd_client::text::tab_label(snapshot, tab).to_owned())
            .into()
    }
}

/// The title of `tab`'s first agent, unless the tab was named or that agent
/// has none. A terminal title is untrusted, so it is made one safe line
/// before it is measured; path titles collapse to their basename.
pub(super) fn title(snapshot: &ClientShellSnapshot, tab: &ClientShellTab) -> Option<String> {
    if tab.custom_label {
        return None;
    }
    let title = first_agent(snapshot, tab)?
        .terminal_title_stripped
        .as_deref()?;
    let safe = crate::truncate::one_line(title, PATH_BOUND);
    if safe.is_empty() {
        return None;
    }
    let display = basename(&safe);
    let line = crate::truncate::one_line(display, TITLE_BOUND);
    (!line.is_empty()).then(|| crate::truncate::tab_title(&line))
}

fn basename(text: &str) -> &str {
    if !looks_like_path(text) {
        return text;
    }
    let trimmed = text.trim_end_matches(['/', '\\']);
    let is_drive_root = trimmed.len() == 2
        && trimmed.as_bytes()[0].is_ascii_alphabetic()
        && trimmed.as_bytes()[1] == b':';
    if trimmed.is_empty() || is_drive_root {
        return text;
    }
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(text)
}

fn looks_like_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    let windows_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    text.starts_with(['/', '\\'])
        || text.starts_with("~/")
        || text.starts_with("~\\")
        || text.starts_with("./")
        || text.starts_with(".\\")
        || text.starts_with("../")
        || text.starts_with("..\\")
        || windows_drive
        || (!text.chars().any(char::is_whitespace) && text.contains(['/', '\\']))
}

/// The first of `tab`'s agents in `agent_order`, the daemon's layout order.
/// A plugin's agent view narrows that order and is authoritative while the
/// daemon names it, so a tab it leaves out has no agent here. Without a view
/// an agent the order does not list still counts, in the snapshot's order.
fn first_agent<'a>(
    snapshot: &'a ClientShellSnapshot,
    tab: &ClientShellTab,
) -> Option<&'a ClientShellAgent> {
    // Runs on every draw: the tab's few agents are found once, so the order
    // is not searched against every agent of every workspace.
    let held: Vec<_> = snapshot
        .agents
        .iter()
        .filter(|agent| agent.tab_id == tab.tab_id)
        .collect();
    let ordered = snapshot
        .agent_order
        .iter()
        .find_map(|pane| held.iter().find(|agent| agent.pane_id == *pane).copied());
    if snapshot.agent_view_label.is_some() {
        return ordered;
    }
    ordered.or_else(|| held.first().copied())
}
