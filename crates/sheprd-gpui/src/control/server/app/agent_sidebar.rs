use crate::{
    SheprdWindow,
    control::protocol::{Caller, ErrorCode, MAX_MESSAGE, Response, app::Reply},
    endpoint::Endpoint,
    state::LiveState,
};
use gpui::{Context, Window};
use std::path::Path;

pub(super) fn sidebar(view: &SheprdWindow) -> Response {
    let state = crate::sidebar::agent_sidebar::state(view);
    if !state.hosts.iter().any(|host| host.supported) {
        return Response::error(
            ErrorCode::Unsupported,
            "No shown Herdr host advertises agent status messages",
        );
    }
    bounded_response(Response::App {
        reply: Reply::AgentSidebar { state },
    })
}

pub(super) fn recap(
    view: &mut SheprdWindow,
    caller: &Caller,
    pane: String,
    window: &mut Window,
    cx: &mut Context<SheprdWindow>,
) -> Response {
    use crate::control::protocol::app::MAX_AGENT_SIDEBAR_PANE_ID_BYTES;

    if pane.is_empty()
        || pane.len() > MAX_AGENT_SIDEBAR_PANE_ID_BYTES
        || pane.chars().any(char::is_control)
    {
        return Response::error(ErrorCode::InvalidRequest, "Invalid pane ID");
    }
    let Some(index) = endpoint_index_for_caller(view, caller) else {
        return Response::error(
            ErrorCode::NotFound,
            "No shown host matches the caller's daemon, workspace, or pane",
        );
    };
    let Some((endpoint, live)) = endpoint_live(view, index) else {
        return Response::error(
            ErrorCode::NotFound,
            "The caller's host is no longer current",
        );
    };
    if !live.agent_sidebar.is_supported() {
        return Response::error(
            ErrorCode::Unsupported,
            "The caller's host does not advertise agent status messages",
        );
    }
    let Some((endpoint_id, message)) = live.agent_sidebar.prepared().and_then(|prepared| {
        let agent = prepared.agent(&pane)?;
        if !agent.idle_recap {
            return None;
        }
        Some((
            endpoint.id.clone(),
            prepared.full_message(&pane)?.to_owned(),
        ))
    }) else {
        if live
            .agent_sidebar
            .prepared()
            .is_some_and(|prepared| prepared.agent(&pane).is_some())
        {
            return Response::error(ErrorCode::NoResult, "No cached idle recap is available");
        }
        return Response::error(
            ErrorCode::NotFound,
            "No agent uses that pane ID on this host",
        );
    };
    if !view.open_agent_recap(&endpoint_id, &pane, window, cx) {
        return Response::error(
            ErrorCode::Busy,
            "The recap could not open while another dialog or recap is active",
        );
    }
    bounded_response(Response::App {
        reply: Reply::AgentRecap {
            endpoint: endpoint_id,
            pane,
            message,
        },
    })
}

pub(super) fn endpoint_index_for_caller(view: &SheprdWindow, caller: &Caller) -> Option<usize> {
    if let Some(daemon) = caller.daemon_socket.as_deref() {
        let requested = Path::new(daemon);
        return unique_index(
            view.endpoints
                .iter()
                .enumerate()
                .filter_map(|(index, endpoint)| {
                    (endpoint.connection.target.socket_path().ok().as_deref() == Some(requested))
                        .then_some(index)
                }),
        );
    }
    if caller.workspace_id.is_none() && caller.terminal_id.is_some() {
        return None;
    }
    if let Some(workspace) = caller.workspace_id.as_deref() {
        return unique_index(view.endpoints.iter().enumerate().filter_map(|(index, _)| {
            let (_, live) = endpoint_live(view, index)?;
            let snapshot = live.snapshot.as_ref()?;
            let has_workspace = snapshot
                .workspaces
                .iter()
                .any(|candidate| candidate.workspace_id == workspace);
            let has_pane = caller.pane_id.as_deref().is_none_or(|pane| {
                snapshot
                    .agents
                    .iter()
                    .any(|agent| agent.pane_id == pane && agent.workspace_id == workspace)
            });
            (has_workspace && has_pane).then_some(index)
        }));
    }
    if let Some(pane) = caller.pane_id.as_deref() {
        return unique_index(view.endpoints.iter().enumerate().filter_map(|(index, _)| {
            let (_, live) = endpoint_live(view, index)?;
            live.snapshot
                .as_ref()?
                .agents
                .iter()
                .any(|agent| agent.pane_id == pane)
                .then_some(index)
        }));
    }
    (caller.terminal_id.is_none())
        .then_some(view.selected_endpoint)
        .filter(|index| *index < view.endpoints.len())
}

fn endpoint_live(view: &SheprdWindow, index: usize) -> Option<(&Endpoint, &LiveState)> {
    let endpoint = view.endpoints.get(index)?;
    if index == view.selected_endpoint {
        (endpoint.generation == view.selected_generation).then_some((endpoint, &view.live))
    } else {
        Some((endpoint, &endpoint.live))
    }
}

fn unique_index(mut indices: impl Iterator<Item = usize>) -> Option<usize> {
    let index = indices.next()?;
    indices.next().is_none().then_some(index)
}

pub(super) fn bounded_response(response: Response) -> Response {
    match serde_json::to_vec(&response) {
        Ok(encoded) if encoded.len() <= MAX_MESSAGE => response,
        Ok(_) => Response::error(
            ErrorCode::Failed,
            "The Agent Sidebar response exceeds the control message limit",
        ),
        Err(error) => Response::error(
            ErrorCode::Failed,
            format!("Could not encode the Agent Sidebar response: {error}"),
        ),
    }
}
