use anyhow::Result;
use serde_json::{json, Value};

use super::super::NativeAgent;

impl NativeAgent {
    pub(super) fn build_llm_messages(
        &self,
        session_id: &str,
        system: Option<&str>,
    ) -> Result<Vec<Value>> {
        let rows = self.session_messages_blocking(session_id)?;
        let mut messages = Vec::new();
        if let Some(s) = system {
            if !s.trim().is_empty() {
                messages.push(json!({ "role": "system", "content": s }));
            }
        }
        for row in rows {
            let role = row
                .info
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            let mut text = String::new();
            for p in &row.parts {
                if p.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(t) = p.get("text").and_then(Value::as_str) {
                        text.push_str(t);
                    }
                }
            }
            if role == "assistant" && text.is_empty() {
                continue;
            }
            messages.push(json!({ "role": role, "content": text }));
        }
        Ok(messages)
    }
}
