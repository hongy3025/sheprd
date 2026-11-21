use super::super::{Reply, SettingsOp};
use crate::protocol::{Call, Caller, Request};

#[test]
fn sidebar_settings_distinguish_a_read_from_a_durable_edit() {
    let read = Request::Settings(Call {
        caller: Caller::default(),
        op: SettingsOp::AgentSidebar {
            pane_strip: None,
            status_text: None,
        },
    });
    assert_eq!(
        serde_json::to_value(read).unwrap()["op"],
        serde_json::json!({"op":"agent_sidebar"})
    );

    let edit = SettingsOp::AgentSidebar {
        pane_strip: Some(false),
        status_text: Some(true),
    };
    assert!(edit.wait_seconds() > 0);
    let read = SettingsOp::AgentSidebar {
        pane_strip: None,
        status_text: None,
    };
    assert!(
        read.wait_seconds() > 0,
        "reading shared config does disk I/O"
    );
    let reply = crate::protocol::Response::Settings {
        reply: Reply::AgentSidebar {
            pane_strip: false,
            status_text: true,
        },
    };
    let encoded = serde_json::to_vec(&reply).unwrap();
    assert_eq!(
        serde_json::from_slice::<crate::protocol::Response>(&encoded).unwrap(),
        reply
    );
}
