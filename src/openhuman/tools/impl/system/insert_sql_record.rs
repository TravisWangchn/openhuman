//! Tool: insert_sql_record — insert an episodic record into the FTS5 memory database.

use crate::openhuman::memory::store::fts5::{self, EpisodicEntry};
use crate::openhuman::tools::traits::{PermissionLevel, Tool, ToolResult};
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::Connection;
use serde_json::json;
use std::sync::Arc;

/// Valid values for the `role` parameter.
const VALID_ROLES: &[&str] = &["user", "assistant", "tool"];

/// Inserts an episodic memory record into the FTS5 episodic-memory SQLite table.
///
/// Wired to the live `episodic_log` + `episodic_fts` tables via
/// `fts5::episodic_insert`. Accepts an `Arc<Mutex<Connection>>` from the
/// `UnifiedMemory` connection pool so inserts land in the same database
/// the Archivist and search tools use.
pub struct InsertSqlRecordTool {
    conn: Option<Arc<Mutex<Connection>>>,
}

impl InsertSqlRecordTool {
    /// Create an insert_sql_record tool with a live DB connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn: Some(conn) }
    }

    /// Create an insert_sql_record tool with no DB connection (no-op stub).
    /// Used when the memory system hasn't been initialised yet — every
    /// insert returns a clear error so the agent doesn't silently lose data.
    pub fn disabled() -> Self {
        Self { conn: None }
    }
}

impl Default for InsertSqlRecordTool {
    fn default() -> Self {
        Self::disabled()
    }
}

#[async_trait]
impl Tool for InsertSqlRecordTool {
    fn name(&self) -> &str {
        "insert_sql_record"
    }

    fn description(&self) -> &str {
        "Insert an episodic memory record into the FTS5 memory database. \
         Records are tagged with a session ID, role (user/assistant/tool), \
         content, and an optional extracted lesson. The database enables \
         full-text search over conversation history for future retrieval."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["session_id", "role", "content"],
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Unique identifier for the conversation session."
                },
                "role": {
                    "type": "string",
                    "enum": ["user", "assistant", "tool"],
                    "description": "Who produced this record."
                },
                "content": {
                    "type": "string",
                    "description": "The text content of the message or tool output."
                },
                "lesson": {
                    "type": "string",
                    "description": "Optional distilled lesson extracted from this exchange."
                }
            }
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let conn = match &self.conn {
            Some(c) => c,
            None => {
                return Ok(ToolResult::error(
                    "insert_sql_record is not connected to the memory database. \
                     The UnifiedMemory system must be initialised before episodic inserts are available."
                ));
            }
        };

        // ── Parameter extraction ────────────────────────────────────────────
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter 'session_id'"))?;

        let role = args
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter 'role'"))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter 'content'"))?;

        let lesson = args.get("lesson").and_then(|v| v.as_str());

        // ── Validation ──────────────────────────────────────────────────────
        if !VALID_ROLES.contains(&role) {
            return Ok(ToolResult::error(format!(
                "Invalid role '{role}'. Must be one of: user, assistant, tool."
            )));
        }

        if session_id.trim().is_empty() {
            return Ok(ToolResult::error("'session_id' must not be empty."));
        }

        if content.trim().is_empty() {
            return Ok(ToolResult::error("'content' must not be empty."));
        }

        // ── Insert via the live FTS5 pipeline ───────────────────────────────
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let entry = EpisodicEntry {
            id: None,
            session_id: session_id.to_string(),
            timestamp: ts,
            role: role.to_string(),
            content: content.to_string(),
            lesson: lesson.map(|l| l.to_string()),
            tool_calls_json: None,
            cost_microdollars: 0,
        };

        fts5::episodic_insert(conn, &entry)?;

        tracing::info!(
            session_id = session_id,
            role = role,
            content_len = content.len(),
            has_lesson = lesson.is_some(),
            "[insert_sql_record] episodic record inserted into FTS5"
        );

        let summary = format!(
            "Record inserted: session={session_id} role={role} content_len={} lesson={}",
            content.len(),
            lesson.map_or("none".to_string(), |l| format!("{} chars", l.len())),
        );

        Ok(ToolResult::success(summary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_with_db() -> InsertSqlRecordTool {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(fts5::EPISODIC_INIT_SQL).unwrap();
        InsertSqlRecordTool::new(Arc::new(Mutex::new(conn)))
    }

    fn tool_disabled() -> InsertSqlRecordTool {
        InsertSqlRecordTool::disabled()
    }

    #[tokio::test]
    async fn inserts_and_reads_back() {
        let tool = tool_with_db();
        let result = tool
            .execute(json!({
                "session_id": "sess-001",
                "role": "user",
                "content": "Hello, world!"
            }))
            .await
            .unwrap();
        assert!(!result.is_error, "should succeed: {}", result.output());
        assert!(result.output().contains("Record inserted"));
    }

    #[tokio::test]
    async fn inserts_with_lesson() {
        let tool = tool_with_db();
        let result = tool
            .execute(json!({
                "session_id": "sess-002",
                "role": "assistant",
                "content": "Use cargo fmt before committing.",
                "lesson": "Always format Rust code before review."
            }))
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.output().contains("lesson=24 chars"));
    }

    #[tokio::test]
    async fn disabled_tool_returns_clear_error() {
        let tool = tool_disabled();
        let result = tool
            .execute(json!({
                "session_id": "sess",
                "role": "user",
                "content": "test"
            }))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.output().contains("not connected"));
    }

    #[tokio::test]
    async fn rejects_invalid_role() {
        let tool = tool_with_db();
        let result = tool
            .execute(json!({
                "session_id": "sess-003",
                "role": "system",
                "content": "Invalid role test."
            }))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.output().contains("Invalid role"));
    }

    #[tokio::test]
    async fn rejects_empty_session_id() {
        let tool = tool_with_db();
        let result = tool
            .execute(json!({
                "session_id": "  ",
                "role": "user",
                "content": "Some content."
            }))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.output().contains("session_id"));
    }

    #[tokio::test]
    async fn rejects_empty_content() {
        let tool = tool_with_db();
        let result = tool
            .execute(json!({
                "session_id": "sess-004",
                "role": "tool",
                "content": ""
            }))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.output().contains("content"));
    }

    #[tokio::test]
    async fn missing_required_param_returns_error() {
        let tool = tool_with_db();
        let result = tool
            .execute(json!({ "session_id": "s", "role": "user" }))
            .await;
        assert!(result.is_err(), "should return Err for missing 'content'");
    }

    #[test]
    fn schema_has_required_fields() {
        let schema = tool_disabled().parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("session_id")));
        assert!(required.contains(&json!("role")));
        assert!(required.contains(&json!("content")));
    }

    #[test]
    fn permission_is_write() {
        assert_eq!(tool_disabled().permission_level(), PermissionLevel::Write);
    }
}
