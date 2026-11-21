use super::*;
use sheprd_client::protocol::endpoint::EndpointAgentStatusMessages;
use sheprd_client::protocol::{
    AgentStatus, ClientShellAgent, ClientShellPane, ClientShellSnapshot,
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn snapshot() -> Arc<ClientShellSnapshot> {
    Arc::new(
        serde_json::from_str(include_str!(
            "../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
        ))
        .unwrap(),
    )
}

fn agent(
    base: &ClientShellAgent,
    pane_id: &str,
    tab_id: &str,
    status: AgentStatus,
    focused: bool,
) -> ClientShellAgent {
    let mut agent = base.clone();
    agent.pane_id = pane_id.into();
    agent.tab_id = tab_id.into();
    agent.agent_status = status;
    agent.focused = focused;
    agent
}

fn pane(base: &ClientShellPane, pane_id: &str, tab_id: &str) -> ClientShellPane {
    let mut pane = base.clone();
    pane.pane_id = pane_id.into();
    pane.tab_id = tab_id.into();
    pane
}

fn projection(
    boot_id: &str,
    revision: u64,
    messages: &[(&str, &str)],
) -> Arc<EndpointAgentStatusMessages> {
    Arc::new(EndpointAgentStatusMessages {
        boot_id: boot_id.into(),
        revision,
        messages: messages
            .iter()
            .map(|(pane, text)| ((*pane).into(), (*text).into()))
            .collect::<BTreeMap<_, _>>(),
    })
}

fn make_prepared(snapshot: Arc<ClientShellSnapshot>) -> Prepared {
    Prepared::new(snapshot, None)
}

#[test]
fn winner_priority_precedes_focus_active_tab_and_stable_pane_order() {
    let mut snapshot = snapshot();
    let next = Arc::make_mut(&mut snapshot);
    let base_agent = next.agents[0].clone();
    let base_pane = next.panes[0].clone();
    let mut second_tab = next.tabs[0].clone();
    second_tab.tab_id = "w1:t2".into();
    second_tab.number = 2;
    second_tab.focused = false;
    next.tabs.push(second_tab);
    next.agents = vec![
        agent(&base_agent, "w1:p2", "w1:t2", AgentStatus::Done, true),
        agent(&base_agent, "w1:p1", "w1:t1", AgentStatus::Blocked, false),
    ];
    next.panes = vec![
        pane(&base_pane, "w1:p2", "w1:t2"),
        pane(&base_pane, "w1:p1", "w1:t1"),
    ];

    let prepared = make_prepared(snapshot.clone());
    let summary = prepared.workspace("w1").unwrap();
    assert_eq!(
        snapshot.agents[summary.winner.unwrap().agent_index].pane_id,
        "w1:p1"
    );
    drop(prepared);

    let next = Arc::make_mut(&mut snapshot);
    next.agents[1].agent_status = AgentStatus::Done;
    assert_eq!(
        snapshot.agents[make_prepared(snapshot.clone())
            .workspace("w1")
            .unwrap()
            .winner
            .unwrap()
            .agent_index]
            .pane_id,
        "w1:p2"
    );

    let next = Arc::make_mut(&mut snapshot);
    next.agents[1].focused = true;
    assert_eq!(
        snapshot.agents[make_prepared(snapshot.clone())
            .workspace("w1")
            .unwrap()
            .winner
            .unwrap()
            .agent_index]
            .pane_id,
        "w1:p1"
    );

    let next = Arc::make_mut(&mut snapshot);
    next.agents[0].tab_id = "w1:t1".into();
    assert_eq!(
        snapshot.agents[make_prepared(snapshot.clone())
            .workspace("w1")
            .unwrap()
            .winner
            .unwrap()
            .agent_index]
            .pane_id,
        "w1:p2"
    );

    Arc::make_mut(&mut snapshot).agents.reverse();
    assert_eq!(
        snapshot.agents[make_prepared(snapshot.clone())
            .workspace("w1")
            .unwrap()
            .winner
            .unwrap()
            .agent_index]
            .pane_id,
        "w1:p2"
    );
}

#[test]
fn pane_id_breaks_ties_when_no_status_pane_has_an_order() {
    let mut snapshot = snapshot();
    let next = Arc::make_mut(&mut snapshot);
    let base = next.agents[0].clone();
    next.agents = vec![
        agent(&base, "z-pane", "w1:t1", AgentStatus::Working, true),
        agent(&base, "a-pane", "w1:t1", AgentStatus::Working, true),
    ];

    let prepared = make_prepared(snapshot.clone());
    let summary = prepared.workspace("w1").unwrap();
    assert_eq!(
        snapshot.agents[summary.winner.unwrap().agent_index].pane_id,
        "a-pane"
    );
}

#[test]
fn workspace_counts_only_its_tabs_and_panes_and_marks_other_tab_attention() {
    let mut snapshot = snapshot();
    let next = Arc::make_mut(&mut snapshot);
    let first_workspace = next.workspaces[0].clone();
    let first_tab = next.tabs[0].clone();
    let base_pane = next.panes[0].clone();
    let base_agent = next.agents[0].clone();

    let mut second_tab = first_tab.clone();
    second_tab.tab_id = "w1:t2".into();
    second_tab.number = 2;
    second_tab.focused = false;
    next.tabs.push(second_tab);

    let mut second_workspace = first_workspace;
    second_workspace.workspace_id = "w2".into();
    second_workspace.active_tab_id = "w2:t1".into();
    second_workspace.number = 2;
    next.workspaces.push(second_workspace);

    let mut third_tab = first_tab;
    third_tab.tab_id = "w2:t1".into();
    third_tab.workspace_id = "w2".into();
    third_tab.number = 1;
    next.tabs.push(third_tab);

    next.panes.push(pane(&base_pane, "w1:p2", "w1:t2"));
    next.panes.push(pane(&base_pane, "w1:shell", "w1:t2"));
    let mut third_pane = pane(&base_pane, "w2:p1", "w2:t1");
    third_pane.workspace_id = "w2".into();
    next.panes.push(third_pane);
    let mut other_tab_agent = agent(&base_agent, "w1:p2", "w1:t2", AgentStatus::Done, false);
    other_tab_agent.workspace_id = "w1".into();
    next.agents.push(other_tab_agent);
    let mut other_workspace_agent =
        agent(&base_agent, "w2:p1", "w2:t1", AgentStatus::Blocked, false);
    other_workspace_agent.workspace_id = "w2".into();
    next.agents.push(other_workspace_agent);

    let prepared = make_prepared(snapshot);
    let first = prepared.workspace("w1").unwrap();
    assert_eq!(first.tab_count, 2);
    assert_eq!(first.pane_dots.len(), 3);
    assert_eq!(first.other_tabs_attention, Some(AgentStatus::Done));
    assert_eq!(first.pane_dots[1].status, Some(AgentStatus::Done));
    assert_eq!(first.pane_dots[2].status, None);

    let second = prepared.workspace("w2").unwrap();
    assert_eq!(second.tab_count, 1);
    assert_eq!(second.pane_dots.len(), 1);
    assert_eq!(second.pane_dots[0].status, Some(AgentStatus::Blocked));
}

#[test]
fn only_the_winning_agents_message_is_used_and_idle_recap_is_identified() {
    let mut snapshot = snapshot();
    let next = Arc::make_mut(&mut snapshot);
    let base = next.agents[0].clone();
    next.agents[0] = agent(&base, "w1:p1", "w1:t1", AgentStatus::Idle, true);
    let second = agent(&base, "w1:p2", "w1:t1", AgentStatus::Done, false);
    next.agents.push(second);
    let messages = projection(
        "boot-v1",
        7,
        &[("w1:p1", "OMP recap"), ("w1:stale", "ignored")],
    );
    let prepared = Prepared::new(snapshot, Some(messages));

    let summary = prepared.workspace("w1").unwrap();
    assert_eq!(
        prepared.snapshot.agents[summary.winner.unwrap().agent_index].pane_id,
        "w1:p2"
    );
    assert_eq!(prepared.preview("w1:p2"), None);
    assert_eq!(prepared.preview("w1:p1"), Some(("OMP recap", false)));
    assert!(prepared.agent("w1:p1").unwrap().idle_recap);
    assert!(!prepared.agent("w1:p2").unwrap().idle_recap);
}

#[test]
fn message_preview_caps_unicode_without_copying_the_full_message() {
    let mut snapshot = snapshot();
    let next = Arc::make_mut(&mut snapshot);
    next.agents[0].agent_status = AgentStatus::Idle;
    let long_message = "界".repeat(201);
    let messages = projection("boot-v1", 7, &[("w1:p1", &long_message)]);
    let prepared = Prepared::new(snapshot, Some(messages));

    let (preview, truncated) = prepared.preview("w1:p1").unwrap();
    assert_eq!(preview.chars().count(), 200);
    assert!(truncated);
    assert_eq!(preview, "界".repeat(200));
    assert_eq!(prepared.full_message("w1:p1").unwrap().chars().count(), 201);
    assert!(prepared.agent("w1:p1").unwrap().idle_recap);
}
