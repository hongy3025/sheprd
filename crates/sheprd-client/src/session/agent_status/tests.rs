use super::Projection;
use crate::protocol::endpoint::EndpointAgentStatusMessages;
use serde_json::json;

fn encoded(boot: &str, revision: u64, messages: serde_json::Value) -> String {
    json!({
        "boot_id": boot,
        "revision": revision,
        "messages": messages,
    })
    .to_string()
}

#[test]
fn future_projection_waits_for_its_exact_snapshot_revision() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 2, json!({"p1": "  Run\n tests  "})));

    assert!(projection.take_for("boot", 1).is_none());
    assert_eq!(
        projection
            .take_for("boot", 2)
            .unwrap()
            .messages
            .get("p1")
            .map(String::as_str),
        Some("Run tests")
    );
    assert!(projection.take_for("boot", 3).is_none());
}

#[test]
fn stale_revision_and_wrong_boot_never_match() {
    for (message_boot, snapshot_boot, snapshot_revision) in
        [("boot", "boot", 3), ("old-boot", "boot", 2)]
    {
        let mut projection = Projection::default();
        projection.receive(&encoded(message_boot, 2, json!({"p1": "stale"})));

        assert!(
            projection
                .take_for(snapshot_boot, snapshot_revision)
                .is_none()
        );
    }
}

#[test]
fn complete_projection_drops_panes_missing_from_the_new_revision() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 4, json!({"p1": "working"})));
    assert_eq!(
        projection
            .take_for("boot", 4)
            .unwrap()
            .messages
            .get("p1")
            .map(String::as_str),
        Some("working")
    );

    projection.receive(&encoded("boot", 5, json!({"p2": "a different pane"})));
    let cleared = projection.take_for("boot", 5).unwrap();
    assert!(!cleared.messages.contains_key("p1"));
    assert_eq!(cleared.messages["p2"], "a different pane");
}

#[test]
fn first_valid_projection_wins_for_duplicate_revision() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 2, json!({"p1": "first"})));
    projection.receive(&encoded("boot", 2, json!({"p1": "duplicate"})));

    assert_eq!(
        projection.take_for("boot", 2).unwrap().messages["p1"],
        "first"
    );
}

#[test]
fn duplicate_revision_after_consumption_cannot_replace_its_message() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 2, json!({"p1": "first"})));
    assert_eq!(
        projection.take_for("boot", 2).unwrap().messages["p1"],
        "first"
    );

    projection.receive(&encoded("boot", 2, json!({"p1": "duplicate"})));

    assert!(projection.take_for("boot", 2).is_none());
}

#[test]
fn malformed_optional_message_does_not_replace_a_valid_projection() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 2, json!({"p1": "valid"})));
    projection.receive("{");

    assert_eq!(
        projection.take_for("boot", 2).unwrap().messages["p1"],
        "valid"
    );
}

#[test]
fn full_messages_are_capped_at_8192_unicode_characters() {
    for (input_len, expected_len, expected_last) in [(8_192, 8_192, '界'), (8_193, 8_192, '…')] {
        let text = "界".repeat(input_len);
        let mut projection = Projection::default();
        projection.receive(&encoded("boot", 2, json!({"p1": text})));

        let messages = projection.take_for("boot", 2).unwrap();
        let text = messages.messages.get("p1").unwrap();
        assert_eq!(text.chars().count(), expected_len);
        assert_eq!(text.chars().last(), Some(expected_last));
        if input_len > 8_192 {
            assert_eq!(
                text.chars().take(8_191).collect::<String>(),
                "界".repeat(8_191)
            );
        }
    }
}

#[test]
fn empty_control_only_text_is_discarded() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 2, json!({"p1": " \t\u{0000}\n "})));

    let messages = projection.take_for("boot", 2).unwrap();
    assert!(!messages.messages.contains_key("p1"));
}

#[test]
fn projection_result_keeps_the_daemon_revision() {
    let mut projection = Projection::default();
    projection.receive(&encoded("boot", 2, json!({"p1": "run"})));

    let actual = projection.take_for("boot", 2).unwrap();

    assert_eq!(
        actual,
        EndpointAgentStatusMessages {
            boot_id: "boot".into(),
            revision: 2,
            messages: [("p1".into(), "run".into())].into(),
        }
    );
}
