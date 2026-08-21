use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemory {
    pub id: String,
    pub agent_id: String,
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub ttl_seconds: Option<i64>,
}

/// Conflict resolution policy for agent memory updates.
/// Research ref: Graph-Native Belief Revision arXiv:2603.17244 — AGM postulates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Last write wins — overwrites without history. Default for backward compat.
    #[default]
    Overwrite,
    /// Reject the update if a value already exists for this key.
    Reject,
    /// Archive old value to memory_history, then store new value.
    VersionAndKeep,
    /// Mark old value as superseded (timestamp), then store new value.
    Invalidate,
}

impl ConflictPolicy {
    pub fn parse(s: &str) -> Self {
        match s {
            "overwrite" => ConflictPolicy::Overwrite,
            "reject" => ConflictPolicy::Reject,
            "version_and_keep" => ConflictPolicy::VersionAndKeep,
            "invalidate" => ConflictPolicy::Invalidate,
            _ => ConflictPolicy::Overwrite,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryRequest {
    pub namespace: Option<String>,
    pub key: String,
    pub value: serde_json::Value,
    pub ttl_seconds: Option<i64>,
    /// Conflict resolution policy. Default: Overwrite (backward compatible).
    #[serde(default)]
    pub conflict_policy: ConflictPolicy,
}

/// A historical memory entry archived by VersionAndKeep or Invalidate policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHistoryEntry {
    pub id: String,
    pub agent_id: String,
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    pub superseded_at: DateTime<Utc>,
    pub superseded_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLog {
    pub id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub status: ToolStatus,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Success,
    Error,
    Timeout,
}

impl ToolStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ToolStatus::Success => "success",
            ToolStatus::Error => "error",
            ToolStatus::Timeout => "timeout",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "success" => ToolStatus::Success,
            "error" => ToolStatus::Error,
            "timeout" => ToolStatus::Timeout,
            _ => ToolStatus::Error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateToolLogRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub status: ToolStatus,
    pub duration_ms: Option<i64>,
}

impl AgentMemory {
    pub fn new(
        agent_id: String,
        namespace: String,
        key: String,
        value: serde_json::Value,
        ttl_seconds: Option<i64>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id,
            namespace,
            key,
            value,
            created_at: now,
            updated_at: now,
            ttl_seconds,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_seconds {
            let expires_at = self.updated_at + chrono::Duration::seconds(ttl);
            Utc::now() > expires_at
        } else {
            false
        }
    }
}

impl ToolLog {
    pub fn new(
        agent_id: String,
        tool_name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        status: ToolStatus,
        duration_ms: Option<i64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id,
            tool_name,
            input,
            output,
            status,
            duration_ms,
            created_at: Utc::now(),
        }
    }
}
