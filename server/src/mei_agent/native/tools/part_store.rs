use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent_runtime::events::{host_part_summary_from_stored, HostOpencodeEvent};

use super::super::NativeAgent;

impl NativeAgent {
    pub(crate) const MAX_TOOL_PART_OUTPUT: usize = 80_000;

    pub(crate) fn truncate_tool_output_for_store(s: &str, max: usize) -> String {
        if s.len() <= max {
            return s.to_string();
        }
        format!(
            "{}\n… (truncated for storage/UI, {} bytes total)",
            &s[..max],
            s.len()
        )
    }

    pub(crate) fn tool_display_fields(
        name: &str,
        args: &Value,
    ) -> (Option<String>, Option<String>) {
        match name {
            "read_file" => {
                let p = args.get("path").and_then(Value::as_str).unwrap_or("");
                (Some("read_file".into()), Some(p.to_string()))
            }
            "dataset_query" => {
                let id = args
                    .get("dataset_id")
                    .or_else(|| args.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                (Some(format!("dataset_query `{id}`")), Some(id.to_string()))
            }
            "dataset_metric" => {
                let id = args
                    .get("dataset_id")
                    .or_else(|| args.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                (Some(format!("dataset_metric `{id}`")), Some(id.to_string()))
            }
            "resource_get" => {
                let id = args
                    .get("resource_id")
                    .or_else(|| args.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                (Some(format!("resource_get `{id}`")), Some(id.to_string()))
            }
            "rewrite_current_mei" => {
                let path = args
                    .get("target_file")
                    .and_then(Value::as_str)
                    .unwrap_or("<current-target>");
                (Some("rewrite_current_mei".into()), Some(path.to_string()))
            }
            "skill_list" => (Some("skill_list".into()), None),
            "skill_read" => {
                let p = args.get("path").and_then(Value::as_str).unwrap_or("");
                (Some("skill_read".into()), Some(p.to_string()))
            }
            _ => (Some(name.to_string()), None),
        }
    }

    pub(super) fn emit_part_upsert_from_value(&self, part: &Value) -> Result<()> {
        let Some(summary) = host_part_summary_from_stored(part) else {
            anyhow::bail!("part normalize failed");
        };
        let sid = part
            .get("sessionID")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.emit(HostOpencodeEvent::MessagePartUpsert {
            session_id: sid,
            message_id: summary.message_id.clone(),
            part: summary,
        });
        Ok(())
    }

    /// 在一批 `tool` parts 之后追加空 `text` part，并把后续 `append_part_text` 的目标切换到该 part，
    /// 使 `ORDER BY sort_order` 与对话时间线一致（避免「整段正文永远在工具块之前」）。
    pub(crate) fn ensure_assistant_text_continuation_part(
        &self,
        session_id: &str,
        message_id: &str,
        active_text_part_id: &Arc<Mutex<String>>,
    ) -> Result<()> {
        let last_json: Option<String> = {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            match db.query_row(
                "SELECT json FROM parts WHERE message_id = ?1 ORDER BY sort_order DESC LIMIT 1",
                params![message_id],
                |r| r.get(0),
            ) {
                Ok(s) => Some(s),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(e.into()),
            }
        };
        let Some(last_json) = last_json else {
            return Ok(());
        };
        let v: Value = serde_json::from_str(&last_json)?;
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        if ty != "tool" {
            return Ok(());
        }

        let new_part_id = format!("prt_{}", Uuid::new_v4());
        let part = json!({
            "id": new_part_id,
            "messageID": message_id,
            "sessionID": session_id,
            "type": "text",
            "text": "",
        });
        {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            let sort_order: i64 = db.query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM parts WHERE message_id = ?1",
                params![message_id],
                |r| r.get(0),
            )?;
            db.execute(
                "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    new_part_id,
                    message_id,
                    session_id,
                    sort_order,
                    part.to_string(),
                ],
            )?;
        }
        self.emit_part_upsert_from_value(&part)?;

        let mut g = active_text_part_id
            .lock()
            .map_err(|_| anyhow!("assistant text part lock poisoned"))?;
        *g = new_part_id;
        Ok(())
    }

    pub(crate) fn insert_tool_part_running(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        call_id: &str,
        tool_name: &str,
        title: Option<&str>,
        input_path: Option<&str>,
    ) -> Result<String> {
        let part_id = format!("prt_{}", Uuid::new_v4());
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let sort_order: i64 = db.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM parts WHERE message_id = ?1",
            params![assistant_message_id],
            |r| r.get(0),
        )?;
        let state = json!({
            "status": "running",
            "title": title.unwrap_or(tool_name),
            "input": { "filePath": input_path.unwrap_or("") },
        });
        let part = json!({
            "id": part_id,
            "messageID": assistant_message_id,
            "sessionID": session_id,
            "type": "tool",
            "callID": call_id,
            "tool": tool_name,
            "state": state,
        });
        let ps = part.to_string();
        db.execute(
            "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&part_id, assistant_message_id, session_id, sort_order, ps],
        )?;
        drop(db);
        self.emit_part_upsert_from_value(&part)?;
        Ok(part_id)
    }

    pub(crate) fn update_tool_part_finished(
        &self,
        session_id: &str,
        assistant_message_id: &str,
        part_id: &str,
        status: &str,
        output: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let s: String = db.query_row(
            "SELECT json FROM parts WHERE id = ?1 AND message_id = ?2",
            params![part_id, assistant_message_id],
            |r| r.get(0),
        )?;
        let mut v: Value = serde_json::from_str(&s)?;
        let state = v
            .get_mut("state")
            .and_then(|x| x.as_object_mut())
            .ok_or_else(|| anyhow!("tool part missing state"))?;
        state.insert("status".into(), json!(status));
        match output {
            Some(o) => {
                state.insert("output".into(), json!(o));
            }
            None => {
                state.remove("output");
            }
        }
        match error {
            Some(e) => {
                state.insert("error".into(), json!(e));
            }
            None => {
                state.remove("error");
            }
        }
        let ns = v.to_string();
        db.execute(
            "UPDATE parts SET json = ?1 WHERE id = ?2 AND session_id = ?3",
            params![ns, part_id, session_id],
        )?;
        drop(db);
        self.emit_part_upsert_from_value(&v)?;
        Ok(())
    }
}
