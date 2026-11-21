#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use sheprd_protocol::endpoint::EndpointAgentStatusMessages;
use std::collections::BTreeMap;

#[test]
fn agent_status_capability_and_kind_match_the_herdr_contract() {
    assert_eq!(
        sheprd_protocol::endpoint::AGENT_STATUS_MESSAGES_CAPABILITY,
        "agent_status_messages"
    );
    assert_eq!(
        sheprd_protocol::endpoint::AGENT_STATUS_MESSAGES_KIND,
        "endpoint.agent-status-messages.v1"
    );
}

#[test]
fn agent_status_projection_uses_the_companion_json_shape() {
    let projection = EndpointAgentStatusMessages {
        boot_id: "boot-1".into(),
        revision: 7,
        messages: BTreeMap::from([
            ("pane-a".into(), "Run tests".into()),
            ("pane-b".into(), "Waiting for approval".into()),
        ]),
    };

    let encoded = serde_json::to_value(&projection).unwrap();

    assert_eq!(
        encoded,
        json!({
            "boot_id": "boot-1",
            "revision": 7,
            "messages": {
                "pane-a": "Run tests",
                "pane-b": "Waiting for approval"
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<EndpointAgentStatusMessages>(encoded).unwrap(),
        projection
    );
}
