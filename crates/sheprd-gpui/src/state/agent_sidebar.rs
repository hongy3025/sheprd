use crate::sidebar::summary::Prepared;
use sheprd_client::protocol::{
    ClientShellSnapshot,
    endpoint::{
        AGENT_STATUS_MESSAGES_CAPABILITY, EndpointAgentStatusMessages, EndpointServerWelcome,
    },
};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct AgentSidebarState {
    active: Option<Arc<EndpointAgentStatusMessages>>,
    pending: Option<Arc<EndpointAgentStatusMessages>>,
    prepared: Option<Arc<Prepared>>,
    supported: bool,
}

impl AgentSidebarState {
    pub(crate) fn connected(&mut self, welcome: &EndpointServerWelcome) {
        *self = Self {
            supported: welcome
                .capabilities
                .iter()
                .any(|capability| capability == AGENT_STATUS_MESSAGES_CAPABILITY),
            ..Self::default()
        };
    }

    pub(crate) fn receive(&mut self, messages: Arc<EndpointAgentStatusMessages>) {
        if !self.supported {
            return;
        }
        if self
            .pending
            .as_ref()
            .or(self.active.as_ref())
            .is_some_and(|current| {
                current.boot_id == messages.boot_id && current.revision >= messages.revision
            })
        {
            return;
        }
        self.pending = Some(messages);
    }

    pub(crate) fn commit_snapshot(&mut self, snapshot: Arc<ClientShellSnapshot>) {
        if !self.supported {
            *self = Self::default();
            return;
        }

        if let Some(pending) = self.pending.as_ref() {
            if pending.boot_id == snapshot.boot_id && pending.revision == snapshot.revision {
                self.active = self.pending.take();
            } else if pending.boot_id != snapshot.boot_id || pending.revision < snapshot.revision {
                self.pending = None;
            }
        }
        if self.active.as_ref().is_some_and(|active| {
            active.boot_id != snapshot.boot_id || active.revision != snapshot.revision
        }) {
            self.active = None;
        }

        self.prepared = Some(Arc::new(Prepared::new(snapshot, self.active.clone())));
    }

    pub(crate) fn disconnect(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn prepared(&self) -> Option<&Prepared> {
        self.prepared.as_deref()
    }

    pub(crate) fn is_supported(&self) -> bool {
        self.supported
    }

    pub(crate) fn projection_stamp(&self) -> usize {
        self.prepared
            .as_ref()
            .map_or(0, |prepared| Arc::as_ptr(prepared) as usize)
    }
}
