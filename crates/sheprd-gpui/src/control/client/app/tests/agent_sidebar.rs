use crate::{
    control::client::{
        Output,
        app::{agent_recap_text, agent_sidebar_text},
    },
    control::protocol::app::{
        AgentSidebarState, SidebarAgent, SidebarHost, SidebarPaneDot, SidebarSelection,
        SidebarWorkspace,
    },
};
use sheprd_client::protocol::AgentStatus;

#[test]
fn sidebar_text_is_compact_and_json_keeps_the_structured_projection() {
    let state = AgentSidebarState {
        selected: Some(SidebarSelection {
            endpoint: "host-a".into(),
            generation: 4,
            boot: "boot".into(),
            workspace: "workspace-1".into(),
        }),
        hosts: vec![
            SidebarHost {
                id: "host-a".into(),
                supported: true,
                workspaces: vec![SidebarWorkspace {
                    id: "workspace-1".into(),
                    tab_count: 2,
                    pane_dots: vec![SidebarPaneDot {
                        pane: "w1:p1".into(),
                        status: Some(AgentStatus::Working),
                        winner: true,
                    }],
                    winner: Some(AgentStatus::Working),
                    other_tabs_attention: None,
                }],
                agents: vec![SidebarAgent {
                    pane: "w1:p1".into(),
                    workspace: "workspace-1".into(),
                    status: AgentStatus::Idle,
                    preview: Some("Finished the migration.".into()),
                    idle_recap: true,
                    focused: true,
                    linked: true,
                }],
            },
            SidebarHost {
                id: "legacy".into(),
                supported: false,
                workspaces: Vec::new(),
                agents: Vec::new(),
            },
        ],
    };
    let text = agent_sidebar_text(&state);
    assert!(text.contains("host-a (supported)"), "{text}");
    assert!(text.contains("w1:p1"), "{text}");
    assert!(text.contains("Finished the migration."), "{text}");
    assert!(text.contains("legacy (unsupported)"), "{text}");

    let output = Output::data(text, &state).unwrap();
    assert_eq!(
        output.json,
        serde_json::to_value(state).expect("sidebar state serializes")
    );
}

#[test]
fn sidebar_and_recap_text_never_write_remote_control_characters() {
    let state = AgentSidebarState {
        selected: None,
        hosts: vec![SidebarHost {
            id: "host\u{1b}[31m".into(),
            supported: true,
            workspaces: vec![SidebarWorkspace {
                id: "work\nspace".into(),
                tab_count: 1,
                pane_dots: vec![SidebarPaneDot {
                    pane: "pane\t1".into(),
                    status: None,
                    winner: false,
                }],
                winner: None,
                other_tabs_attention: None,
            }],
            agents: vec![SidebarAgent {
                pane: "agent\u{1b}[31m".into(),
                workspace: "work\nspace".into(),
                status: AgentStatus::Idle,
                preview: Some("safe preview".into()),
                idle_recap: true,
                focused: false,
                linked: false,
            }],
        }],
    };
    let sidebar = agent_sidebar_text(&state);
    assert!(
        !sidebar
            .chars()
            .any(|character| character.is_control() && character != '\n'),
        "{sidebar:?}"
    );
    assert!(sidebar.contains("host [31m"), "{sidebar:?}");
    assert!(sidebar.contains("work space"), "{sidebar:?}");
    assert!(sidebar.contains("pane 1"), "{sidebar:?}");

    let recap = agent_recap_text("host\u{1b}[31m", "pane\n1", "safe recap");
    assert!(
        !recap
            .chars()
            .any(|character| character.is_control() && character != '\n'),
        "{recap:?}"
    );
    assert!(recap.contains("host [31m/pane 1\nsafe recap"), "{recap:?}");
}
