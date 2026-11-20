use crate::protocol::{
    Call, Caller, Request,
    app::{
        AgentSidebarState, Reply, SidebarAgent, SidebarHost, SidebarPaneDot, SidebarSelection,
        SidebarWorkspace, UiOp,
    },
};
use sheprd_client::protocol::AgentStatus;

#[test]
fn sidebar_operations_and_reply_round_trip_with_snapshot_identity() {
    let state = AgentSidebarState {
        selected: Some(SidebarSelection {
            endpoint: "host-a".into(),
            generation: 7,
            boot: "boot-1".into(),
            workspace: "workspace-1".into(),
        }),
        hosts: vec![SidebarHost {
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
                other_tabs_attention: Some(AgentStatus::Blocked),
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
        }],
    };
    let requests = [
        Request::Ui(Call {
            caller: Caller::default(),
            op: UiOp::AgentSidebar,
        }),
        Request::Ui(Call {
            caller: Caller::default(),
            op: UiOp::AgentRecap {
                pane: "w1:p1".into(),
            },
        }),
    ];
    assert_eq!(
        serde_json::to_value(&requests[0]).unwrap()["op"],
        serde_json::json!({"op":"agent_sidebar"})
    );
    assert_eq!(
        serde_json::to_value(&requests[1]).unwrap()["op"],
        serde_json::json!({"op":"agent_recap","pane":"w1:p1"})
    );

    let reply = crate::protocol::Response::App {
        reply: Reply::AgentSidebar { state },
    };
    let encoded = serde_json::to_vec(&reply).unwrap();
    assert_eq!(
        serde_json::from_slice::<crate::protocol::Response>(&encoded).unwrap(),
        reply
    );
    assert!(
        serde_json::from_value::<UiOp>(serde_json::json!({
            "op": "agent_recap",
            "pane": "x",
            "extra": true
        }))
        .is_err()
    );
}
