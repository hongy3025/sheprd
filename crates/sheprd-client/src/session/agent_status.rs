use crate::protocol::endpoint::EndpointAgentStatusMessages;
use std::collections::BTreeMap;

const MAX_MESSAGE_CHARS: usize = 8_192;

#[derive(Default)]
pub(super) struct Projection {
    pending: Option<EndpointAgentStatusMessages>,
    applied: Option<(String, u64)>,
}

impl Projection {
    pub(super) fn receive(&mut self, data: &str) {
        let Ok(mut projection) = serde_json::from_str::<EndpointAgentStatusMessages>(data) else {
            return;
        };
        if projection.boot_id.is_empty() {
            return;
        }
        if self.applied.as_ref().is_some_and(|(boot_id, revision)| {
            boot_id == &projection.boot_id && *revision >= projection.revision
        }) {
            return;
        }
        if self.pending.as_ref().is_some_and(|pending| {
            pending.boot_id == projection.boot_id && pending.revision >= projection.revision
        }) {
            return;
        }

        let mut messages = BTreeMap::new();
        for (pane, raw) in projection.messages {
            if let Some(message) = normalize_message(&raw) {
                messages.insert(pane, message);
            }
        }
        projection.messages = messages;
        self.pending = Some(projection);
    }

    pub(super) fn take_for(
        &mut self,
        boot_id: &str,
        revision: u64,
    ) -> Option<EndpointAgentStatusMessages> {
        let pending = self.pending.as_ref()?;
        if pending.boot_id == boot_id && pending.revision == revision {
            let projection = self.pending.take()?;
            self.applied = Some((projection.boot_id.clone(), projection.revision));
            return Some(projection);
        }
        if pending.boot_id != boot_id || pending.revision < revision {
            self.pending = None;
        }
        None
    }
}

fn normalize_message(raw: &str) -> Option<String> {
    let mut text = String::with_capacity(raw.len().min(MAX_MESSAGE_CHARS + 1));
    let mut character_count = 0;
    let mut pending_space = false;

    for character in raw.chars() {
        if character.is_control() || character.is_whitespace() {
            pending_space = !text.is_empty();
            continue;
        }
        if pending_space {
            if character_count == MAX_MESSAGE_CHARS {
                text.pop();
                text.push('…');
                return Some(text);
            }
            text.push(' ');
            character_count += 1;
            pending_space = false;
        }
        if character_count == MAX_MESSAGE_CHARS {
            text.pop();
            text.push('…');
            return Some(text);
        }
        text.push(character);
        character_count += 1;
    }

    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
#[path = "agent_status/tests.rs"]
mod tests;
