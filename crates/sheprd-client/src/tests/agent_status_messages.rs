use super::*;

fn connected_client(capabilities: &[&str]) -> (Client, Stream, thread::JoinHandle<Result<()>>) {
    let (client, mut server, worker) = test_client();
    receive(&mut server);
    let mut welcome: Value = serde_json::from_str(WELCOME).unwrap();
    welcome["capabilities"] = json!(capabilities);
    send(
        &mut server,
        ServerMessage::EndpointControl {
            kind: ENDPOINT_WELCOME_KIND.into(),
            data: welcome.to_string(),
        },
    );
    assert!(matches!(event(&client), ClientEvent::Connected(_)));
    (client, server, worker)
}

fn status_control(boot_id: &str, revision: u64, messages: Value) -> ServerMessage {
    ServerMessage::EndpointControl {
        kind: AGENT_STATUS_MESSAGES_KIND.into(),
        data: json!({
            "boot_id": boot_id,
            "revision": revision,
            "messages": messages,
        })
        .to_string(),
    }
}

fn snapshot_at_revision(revision: u64) -> String {
    let mut snapshot: Value = serde_json::from_str(SNAPSHOT).unwrap();
    snapshot["revision"] = json!(revision);
    snapshot.to_string()
}

fn send_snapshot(server: &mut Stream, data: String) {
    send(
        server,
        ServerMessage::EndpointControl {
            kind: ENDPOINT_SNAPSHOT_KIND.into(),
            data,
        },
    );
}

#[test]
fn projection_event_is_published_only_with_its_matching_snapshot() {
    let (client, mut server, worker) = connected_client(&[AGENT_STATUS_MESSAGES_CAPABILITY]);
    let future = status_control("boot-v1", 8, json!({"w1:p1": "Running tests"}));
    send(&mut server, future);
    send_snapshot(&mut server, snapshot_at_revision(7));

    assert!(matches!(event(&client), ClientEvent::Snapshot(snapshot) if snapshot.revision == 7));

    send_snapshot(&mut server, snapshot_at_revision(8));
    let ClientEvent::AgentStatusMessages(messages) = event(&client) else {
        panic!("matching status message should precede its snapshot")
    };
    assert_eq!(messages.boot_id, "boot-v1");
    assert_eq!(messages.revision, 8);
    assert_eq!(messages.messages["w1:p1"], "Running tests");
    assert!(matches!(event(&client), ClientEvent::Snapshot(snapshot) if snapshot.revision == 8));

    let mut surface = baseline();
    surface.projection_revision = 8;
    send(&mut server, ServerMessage::PaneSurface(surface));
    assert!(
        matches!(event(&client), ClientEvent::Surface(surface) if surface.projection_revision == 8)
    );

    client.handle.disconnect();
    worker.join().unwrap().unwrap();
}

#[test]
fn complete_empty_projection_is_published_to_clear_stale_text() {
    let (client, mut server, worker) = connected_client(&[AGENT_STATUS_MESSAGES_CAPABILITY]);
    send(&mut server, status_control("boot-v1", 7, json!({})));
    send_snapshot(&mut server, SNAPSHOT.into());

    let ClientEvent::AgentStatusMessages(messages) = event(&client) else {
        panic!("empty complete projection should still be published")
    };
    assert_eq!(messages.revision, 7);
    assert!(messages.messages.is_empty());
    assert!(matches!(event(&client), ClientEvent::Snapshot(_)));

    client.handle.disconnect();
    worker.join().unwrap().unwrap();
}

#[test]
fn status_control_is_ignored_without_the_advertised_capability() {
    let (client, mut server, worker) = connected_client(&[]);
    send(
        &mut server,
        status_control("boot-v1", 7, json!({"w1:p1": "ignored"})),
    );
    send_snapshot(&mut server, SNAPSHOT.into());

    assert!(matches!(event(&client), ClientEvent::Snapshot(_)));
    client.handle.disconnect();
    worker.join().unwrap().unwrap();
}

#[test]
fn malformed_optional_status_control_does_not_disconnect() {
    let (client, mut server, worker) = connected_client(&[AGENT_STATUS_MESSAGES_CAPABILITY]);
    send(
        &mut server,
        ServerMessage::EndpointControl {
            kind: AGENT_STATUS_MESSAGES_KIND.into(),
            data: "{".into(),
        },
    );
    send_snapshot(&mut server, SNAPSHOT.into());

    assert!(matches!(event(&client), ClientEvent::Snapshot(_)));
    client.handle.disconnect();
    worker.join().unwrap().unwrap();
}

#[test]
fn wrong_boot_projection_is_never_published() {
    let (client, mut server, worker) = connected_client(&[AGENT_STATUS_MESSAGES_CAPABILITY]);
    send(
        &mut server,
        status_control("old-boot", 7, json!({"w1:p1": "stale"})),
    );
    send_snapshot(&mut server, SNAPSHOT.into());

    assert!(matches!(event(&client), ClientEvent::Snapshot(_)));
    client.handle.disconnect();
    worker.join().unwrap().unwrap();
}

#[test]
fn a_new_connection_does_not_reuse_an_unmatched_projection() {
    let (client, mut server, worker) = connected_client(&[AGENT_STATUS_MESSAGES_CAPABILITY]);
    send(
        &mut server,
        status_control("boot-v1", 8, json!({"w1:p1": "pending"})),
    );
    send_snapshot(&mut server, snapshot_at_revision(7));
    assert!(matches!(event(&client), ClientEvent::Snapshot(snapshot) if snapshot.revision == 7));
    client.handle.disconnect();
    worker.join().unwrap().unwrap();

    let (client, mut server, worker) = connected_client(&[AGENT_STATUS_MESSAGES_CAPABILITY]);
    send_snapshot(&mut server, snapshot_at_revision(8));
    assert!(matches!(event(&client), ClientEvent::Snapshot(snapshot) if snapshot.revision == 8));
    assert!(client.events.try_recv().is_err());

    client.handle.disconnect();
    worker.join().unwrap().unwrap();
}
