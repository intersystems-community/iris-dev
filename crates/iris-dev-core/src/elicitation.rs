//! Elicitation state management for MCP source control dialogs.
//! Stores pending elicitations keyed by UUID, expires after 5 minutes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const EXPIRY: Duration = Duration::from_secs(300); // 5 minutes

#[derive(Debug, Clone)]
pub enum ElicitationAction {
    /// Resume a iris_doc(mode=put) write
    Put,
    /// Resume an iris_source_control execute action
    ScmExecute,
}

#[derive(Debug, Clone)]
pub struct PendingElicitation {
    pub id: String,
    pub document: String,
    pub action: ElicitationAction,
    /// Document content to write on resume (Put only)
    pub content: Option<String>,
    /// SCM action id to execute on resume (ScmExecute only)
    pub scm_action_id: Option<String>,
    pub namespace: String,
    pub expires_at: Instant,
}

#[derive(Clone, Default)]
pub struct ElicitationStore(Arc<Mutex<HashMap<String, PendingElicitation>>>);

impl ElicitationStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a new pending elicitation and return its UUID.
    pub fn insert(
        &self,
        document: impl Into<String>,
        action: ElicitationAction,
        content: Option<String>,
        scm_action_id: Option<String>,
        namespace: impl Into<String>,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let entry = PendingElicitation {
            id: id.clone(),
            document: document.into(),
            action,
            content,
            scm_action_id,
            namespace: namespace.into(),
            expires_at: Instant::now() + EXPIRY,
        };
        self.0.lock().unwrap().insert(id.clone(), entry);
        id
    }

    /// Look up a pending elicitation by id. Returns None if expired or missing.
    pub fn lookup(&self, id: &str) -> Option<PendingElicitation> {
        let mut store = self.0.lock().unwrap();
        let entry = store.get(id)?;
        if Instant::now() > entry.expires_at {
            store.remove(id);
            return None;
        }
        Some(entry.clone())
    }

    /// Remove a pending elicitation.
    pub fn clear(&self, id: &str) {
        self.0.lock().unwrap().remove(id);
    }

    /// Remove all expired entries. Returns the count of removed entries.
    pub fn sweep(&self) -> usize {
        let mut store = self.0.lock().unwrap();
        let now = std::time::Instant::now();
        let expired: Vec<String> = store
            .iter()
            .filter(|(_, e)| now > e.expires_at)
            .map(|(k, _)| k.clone())
            .collect();
        let count = expired.len();
        for key in expired {
            store.remove(&key);
        }
        count
    }
}
