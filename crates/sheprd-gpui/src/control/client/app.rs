//! `status`, `app`, `window`, `ui`, and `run`.
use super::{Context, Output, send, unexpected};
use crate::{
    cli::control::{AppCommand, UiCommand, WindowCommand},
    control::protocol::{
        Request, Response,
        app::{AlertInfo, AppOp, CommandOp, Reply, Status, UiOp, WindowInfo, WindowOp},
        settings::{Reply as SettingsReply, SettingsOp},
    },
};
use std::fmt::Write as _;

/// One line per alert, newest first, each saying who needs what.
pub(super) fn alert_lines(alerts: &[AlertInfo]) -> String {
    if alerts.is_empty() {
        return "No alerts yet".to_owned();
    }
    let mut text = String::new();
    for alert in alerts {
        let kind = if alert.needs_attention {
            "needs you"
        } else {
            "finished"
        };
        let _ = writeln!(text, "{kind}  {}  {}", alert.host, alert.title);
    }
    text
}

fn describe_window(window: &WindowInfo, text: &mut String) {
    let _ = writeln!(
        text,
        "{}{} {} ({}x{}{})",
        if window.active { "*" } else { " " },
        window.id,
        window.title,
        window.width.round(),
        window.height.round(),
        if window.fullscreen {
            ", full screen"
        } else {
            ""
        },
    );
    if let Some(workspace) = &window.workspace {
        let _ = writeln!(text, "    workspace {workspace}");
    }
    for host in &window.hosts {
        let _ = writeln!(
            text,
            "    {} {} ({}): {}",
            if host.selected { ">" } else { " " },
            host.label,
            host.id,
            host.status
        );
    }
}

pub(super) fn status(context: &Context) -> crate::Result<Output> {
    let response = send(&Request::Status(context.caller()?))?;
    let Response::Status(status) = response else {
        return Err(super::rejected(response));
    };
    let Status {
        version,
        protocol,
        pid,
        browser_tabs,
        windows,
    } = &status;
    let mut text = format!(
        "Sheprd {version} is running (pid {pid}, control protocol {protocol}{})\n",
        if *browser_tabs {
            ""
        } else {
            ", pages open in the system browser"
        }
    );
    if *protocol != crate::control::protocol::PROTOCOL_VERSION {
        let _ = writeln!(
            text,
            "This command line speaks protocol {}; update the older of the two.",
            crate::control::protocol::PROTOCOL_VERSION
        );
    }
    if windows.is_empty() {
        text.push_str("No window is open\n");
    }
    for window in windows {
        describe_window(window, &mut text);
    }
    Output::data(text.trim_end(), &status)
}

pub(super) fn app(command: AppCommand, context: &Context) -> crate::Result<Output> {
    context.act(
        Request::App,
        match command {
            AppCommand::Focus => AppOp::Focus,
            AppCommand::Hide => AppOp::Hide,
            AppCommand::Quit => AppOp::Quit,
        },
    )
}

pub(super) fn window(command: WindowCommand, context: &Context) -> crate::Result<Output> {
    let op = match command {
        WindowCommand::List => WindowOp::List,
        WindowCommand::Alerts => WindowOp::Alerts,
        WindowCommand::New { session } => WindowOp::New { session },
        WindowCommand::Focus { id } => {
            let context = Context {
                json: context.json,
                window: Some(id),
            };
            return context.act(Request::Window, WindowOp::Focus);
        }
        WindowCommand::Fullscreen(switch) => WindowOp::Fullscreen { switch },
        WindowCommand::Size(size) => WindowOp::Size { size },
        WindowCommand::Detach => WindowOp::Detach,
        WindowCommand::Reconnect => WindowOp::Reconnect,
    };
    match context.call(Request::Window, op)? {
        Response::Done { message } => Ok(Output::message(message)),
        Response::App {
            reply: Reply::Windows { windows },
        } => {
            let mut text = String::new();
            for window in &windows {
                describe_window(window, &mut text);
            }
            if windows.is_empty() {
                text.push_str("No window is open");
            }
            Output::data(text.trim_end(), &windows)
        }
        Response::App {
            reply: Reply::Alerts { alerts },
        } => Output::data(alert_lines(&alerts).trim_end(), &alerts),
        Response::App {
            reply: Reply::Size { width, height },
        } => Output::data(
            format!("{}x{}", width.round(), height.round()),
            &serde_json::json!({ "width": width, "height": height }),
        ),
        response => Err(unexpected(response)),
    }
}

pub(super) fn ui(command: UiCommand, context: &Context) -> crate::Result<Output> {
    let op = match command {
        UiCommand::AgentSidebar => UiOp::AgentSidebar,
        UiCommand::AgentRecap { pane } => UiOp::AgentRecap { pane },
        UiCommand::AgentSidebarConfig {
            pane_strip,
            status_text,
        } => {
            return sidebar_config(pane_strip, status_text, context);
        }
        UiCommand::Traces => UiOp::Traces {},
        UiCommand::TracesFilter(filter) => UiOp::TracesFilter { filter },
        UiCommand::Sidebar(switch) => UiOp::Sidebar { switch },
        UiCommand::StatusBar(switch) => UiOp::StatusBar { switch },
        UiCommand::MenuBar(switch) => UiOp::MenuBar { switch },
        UiCommand::FontSize(size) => UiOp::FontSize { size },
        UiCommand::Panel(panel) => UiOp::Panel { panel },
        UiCommand::AgentGroups => UiOp::AgentGroups,
        UiCommand::AgentGroup(name) => UiOp::AgentGroup { name },
    };
    match context.call(Request::Ui, op)? {
        Response::App {
            reply: Reply::AgentSidebar { state },
        } => Output::data(agent_sidebar_text(&state), &state),
        Response::App {
            reply:
                Reply::AgentRecap {
                    endpoint,
                    pane,
                    message,
                },
        } => Output::data(
            agent_recap_text(&endpoint, &pane, &message),
            &serde_json::json!({
                "endpoint": endpoint,
                "pane": pane,
                "message": message,
            }),
        ),
        Response::App {
            reply:
                Reply::Traces {
                    peers,
                    filter,
                    hosts,
                },
        } => Output::data(
            format!(
                "{}\n{}",
                trace_hosts_text(&filter, &hosts),
                traces_text(&peers)
            ),
            &serde_json::json!({"peers": peers, "filter": filter, "hosts": hosts}),
        ),
        Response::Done { message } => Ok(Output::message(message)),
        Response::App {
            reply: Reply::FontSize { points },
        } => Output::data(
            format!("Terminal font size {points}"),
            &serde_json::json!({ "points": points }),
        ),
        Response::App {
            reply: Reply::AgentGroups { current, groups },
        } => Output::data(
            agent_groups_text(current.as_deref(), &groups),
            &serde_json::json!({ "current": current, "groups": groups }),
        ),
        response => Err(unexpected(response)),
    }
}

fn sidebar_config(
    pane_strip: Option<bool>,
    status_text: Option<bool>,
    context: &Context,
) -> crate::Result<Output> {
    match context.call(
        Request::Settings,
        SettingsOp::AgentSidebar {
            pane_strip,
            status_text,
        },
    )? {
        Response::Settings {
            reply:
                SettingsReply::AgentSidebar {
                    pane_strip,
                    status_text,
                },
        } => Output::data(
            format!(
                "pane-strip={}\nstatus-text={}",
                on_off(pane_strip),
                on_off(status_text)
            ),
            &serde_json::json!({
                "pane_strip": pane_strip,
                "status_text": status_text,
            }),
        ),
        response => Err(unexpected(response)),
    }
}

fn agent_sidebar_text(state: &crate::control::protocol::app::AgentSidebarState) -> String {
    use std::fmt::Write as _;

    let mut text = String::new();
    if let Some(selected) = &state.selected {
        text.push_str("linked: ");
        append_safe(&mut text, &selected.endpoint);
        text.push('/');
        append_safe(&mut text, &selected.workspace);
        text.push('\n');
    }
    for host in &state.hosts {
        let support = if host.supported {
            "supported"
        } else {
            "unsupported"
        };
        append_safe(&mut text, &host.id);
        let _ = writeln!(text, " ({support})");
        for workspace in &host.workspaces {
            text.push_str("  workspace ");
            append_safe(&mut text, &workspace.id);
            let _ = write!(text, ": {} tabs", workspace.tab_count);
            if let Some(status) = workspace.winner {
                let _ = write!(text, ", winner {}", agent_status_name(status));
            }
            if let Some(status) = workspace.other_tabs_attention {
                let _ = write!(text, ", other tabs {}", agent_status_name(status));
            }
            if !workspace.pane_dots.is_empty() {
                let _ = write!(text, ", panes ");
                for (index, dot) in workspace.pane_dots.iter().enumerate() {
                    if index != 0 {
                        let _ = write!(text, ", ");
                    }
                    let status = dot.status.map_or("pane", agent_status_name);
                    append_safe(&mut text, &dot.pane);
                    let _ = write!(text, ":{status}{}", if dot.winner { "*" } else { "" });
                }
            }
            let _ = writeln!(text);
        }
        for agent in &host.agents {
            text.push_str("  pane ");
            append_safe(&mut text, &agent.pane);
            let _ = write!(text, " [{}]", agent_status_name(agent.status));
            if agent.focused {
                let _ = write!(text, " focused");
            }
            if agent.linked {
                let _ = write!(text, " linked");
            }
            if agent.idle_recap {
                let _ = write!(text, " recap");
            }
            if let Some(preview) = &agent.preview {
                text.push_str(": ");
                append_safe(&mut text, preview);
            }
            let _ = writeln!(text);
        }
    }
    if text.is_empty() {
        text.push_str("No hosts are shown in this window");
    }
    text.trim_end().to_owned()
}

fn agent_recap_text(endpoint: &str, pane: &str, message: &str) -> String {
    let mut text = String::new();
    append_safe(&mut text, endpoint);
    text.push('/');
    append_safe(&mut text, pane);
    text.push('\n');
    append_safe(&mut text, message);
    text
}

fn append_safe(output: &mut String, value: &str) {
    output.extend(value.chars().map(|character| {
        if character.is_control() {
            ' '
        } else {
            character
        }
    }));
}

fn agent_status_name(status: sheprd_client::protocol::AgentStatus) -> &'static str {
    use sheprd_client::protocol::AgentStatus;
    match status {
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Idle => "idle",
        AgentStatus::Unknown => "unknown",
        AgentStatus::Working => "working",
    }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

fn traces_text(peers: &[sheprd_control_protocol::protocol::app::TracePeer]) -> String {
    use sheprd_control_protocol::protocol::app::TraceSyncState;
    if peers.is_empty() {
        return "No connected hosts to synchronize traces".into();
    }
    peers
        .iter()
        .map(|peer| {
            let status = match &peer.state {
                TraceSyncState::Waiting => "waiting".into(),
                TraceSyncState::Syncing => "synchronizing".into(),
                TraceSyncState::Ready => "up to date at the last exchange".into(),
                TraceSyncState::Failed { error } => format!("failed: {error}"),
            };
            format!("{}: {status}", peer.host)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn trace_hosts_text(
    filter: &sheprd_control_protocol::protocol::app::TraceFilter,
    hosts: &[sheprd_control_protocol::protocol::app::TaggedHost],
) -> String {
    use sheprd_control_protocol::protocol::app::TraceFilter;
    let mut text = match filter {
        TraceFilter::All => "Traces: all hosts".to_owned(),
        TraceFilter::Host { id } => format!("Traces: host {id}"),
        TraceFilter::Tag { tag } => format!("Traces: label {tag}"),
    };
    for host in hosts {
        let _ = write!(
            text,
            "\n{}  {}  [{}]",
            host.id,
            host.label,
            host.tags.tags.join(", ")
        );
    }
    text
}

/// One group a line, the one shown marked, as `theme list` marks its own.
fn agent_groups_text(current: Option<&str>, groups: &[String]) -> String {
    if groups.is_empty() {
        return "No coordinator sessions are listed".to_owned();
    }
    groups
        .iter()
        .map(|group| {
            let mark = if current == Some(group.as_str()) {
                "*"
            } else {
                " "
            };
            format!("{mark} {group}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn run_command(name: &str, context: &Context) -> crate::Result<Output> {
    if !crate::controls::COMMANDS
        .iter()
        .any(|info| info.name == name)
    {
        return Err(crate::control::Error::Rejected {
            code: crate::control::ErrorCode::NotFound,
            message: format!("No command is named {name}; `sheprd commands` lists them"),
        }
        .into());
    }
    context.act(
        Request::Command,
        CommandOp::Run {
            name: name.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests;
