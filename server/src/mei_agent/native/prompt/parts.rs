use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::{json, Value};

use crate::agent_runtime::bridge::BridgePromptSummary;

use super::super::{now_ms, NativeAgent};

impl NativeAgent {
    pub(super) fn append_part_text(
        &self,
        session_id: &str,
        message_id: &str,
        part_id: &str,
        delta: &str,
    ) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT json FROM parts WHERE id = ?1 AND message_id = ?2",
            params![part_id, message_id],
            |r| r.get(0),
        )?;
        let mut v: Value = serde_json::from_str(&s)?;
        let cur = v.get("text").and_then(Value::as_str).unwrap_or("");
        v["text"] = Value::String(format!("{cur}{delta}"));
        let ns = v.to_string();
        db.execute(
            "UPDATE parts SET json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![ns, part_id, session_id],
        )?;
        let t = now_ms();
        let info_s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let mut info: Value = serde_json::from_str(&info_s)?;
        if let Some(obj) = info.get_mut("time").and_then(Value::as_object_mut) {
            obj.insert("updated".to_string(), json!(t));
        }
        db.execute(
            "UPDATE messages SET info_json = ?1 WHERE id = ?2",
            params![info.to_string(), message_id],
        )?;
        Ok(())
    }

    pub(super) fn set_message_error(&self, session_id: &str, message_id: &str, err: &str) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let mut info: Value = serde_json::from_str(&s)?;
        info["error"] = json!(err);
        let t = now_ms();
        if let Some(obj) = info.get_mut("time").and_then(Value::as_object_mut) {
            obj.insert("updated".to_string(), json!(t));
        }
        db.execute(
            "UPDATE messages SET info_json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![info.to_string(), message_id, session_id],
        )?;
        Ok(())
    }

    pub(super) fn finalize_assistant(&self, session_id: &str, message_id: &str, finish: &str) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let mut info: Value = serde_json::from_str(&s)?;
        let t = now_ms();
        info["finish"] = json!(finish);
        if let Some(obj) = info.get_mut("time").and_then(Value::as_object_mut) {
            obj.insert("updated".to_string(), json!(t));
        }
        db.execute(
            "UPDATE messages SET info_json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![info.to_string(), message_id, session_id],
        )?;
        Ok(())
    }

    pub(super) fn touch_session_updated(&self, session_id: &str) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let t = now_ms();
        db.execute(
            "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
            params![t, session_id],
        )?;
        Ok(())
    }

    pub(super) fn read_prompt_summary(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<BridgePromptSummary> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let info_s: String = db.query_row(
            "SELECT info_json FROM messages WHERE id = ?1",
            params![message_id],
            |r| r.get(0),
        )?;
        let info: Value = serde_json::from_str(&info_s)?;
        let mut stmt =
            db.prepare("SELECT json FROM parts WHERE message_id = ?1 ORDER BY sort_order")?;
        let parts: Vec<Value> = stmt
            .query_map(params![message_id], |r| {
                let s: String = r.get(0)?;
                Ok(serde_json::from_str::<Value>(&s).unwrap_or(Value::Null))
            })?
            .collect::<Result<_, _>>()?;
        let mut texts = Vec::new();
        let mut part_types = Vec::new();
        for p in &parts {
            if let Some(pt) = p.get("type").and_then(Value::as_str) {
                part_types.push(pt.to_string());
                if pt == "text" {
                    if let Some(tx) = p.get("text").and_then(Value::as_str) {
                        texts.push(tx.to_string());
                    }
                }
            }
        }
        Ok(BridgePromptSummary {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            provider_id: info
                .get("providerID")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model_id: info
                .get("modelID")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            finish: info
                .get("finish")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            texts,
            part_types,
            error: info.get("error").cloned(),
            scope_digest: None,
            profile_summary: None,
        })
    }
}
