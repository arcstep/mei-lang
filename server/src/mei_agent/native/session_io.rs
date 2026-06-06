use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::agent_runtime::bridge::{
    BridgeDiffSummary, BridgeFileDiffSummary, BridgeRevertRequest, BridgeRevertSummary,
    BridgeUnrevertSummary,
};

use super::super::workspace_snapshot_git::{WorkspaceSnapshotGit, SESSION_BASELINE_ANCHOR};

use super::{diff, now_ms, NativeAgent};
impl NativeAgent {
    pub(super) fn capture_session_diff_snapshot(
        &self,
        session_id: &str,
        anchor_message_id: &str,
        diff_rel: Option<&str>,
    ) -> Result<()> {
        let (vcs, _) = self.vcs_summary_blocking();
        let (diff_text, additions, deletions) = if vcs {
            diff::git_worktree_diff(&self.inner.source_root, diff_rel)
        } else {
            (
                "(native diff snapshot: no git worktree at capture time)\n".to_string(),
                0,
                0,
            )
        };
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let t = now_ms() as i64;
        db.execute(
            "INSERT INTO session_diff_snapshots (session_id, anchor_message_id, diff_text, additions, deletions, captured_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                anchor_message_id,
                diff_text,
                additions as i64,
                deletions as i64,
                t,
            ],
        )?;
        Ok(())
    }
    #[allow(dead_code)]
    pub fn session_diff_blocking(
        &self,
        session_id: &str,
        message_id: Option<&str>,
        diff_rel: Option<&str>,
    ) -> Result<BridgeDiffSummary> {
        let root = &self.inner.source_root;
        if let Some(mid) = message_id {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            let row = db.query_row(
                "SELECT diff_text, additions, deletions, captured_ms FROM session_diff_snapshots \
                 WHERE session_id = ?1 AND anchor_message_id = ?2 ORDER BY id DESC LIMIT 1",
                params![session_id, mid],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            );
            drop(db);
            match row {
                Ok((mut diff_text, _add_i, _del_i, cap_ms)) => {
                    if let Some(rel) = diff_rel {
                        diff_text = diff::filter_unified_diff_for_rel_path(&diff_text, rel);
                    }
                    let (additions, deletions) = diff::count_diff_lines(&diff_text);
                    let anchor = match diff_rel {
                        Some(rel) => format!(
                            "(diff snapshot for assistant message {mid}; captured_ms={cap_ms}; path={rel})"
                        ),
                        None => format!(
                            "(diff snapshot for assistant message {mid}; captured_ms={cap_ms})"
                        ),
                    };
                    let file_label = diff_rel
                        .map(str::to_string)
                        .unwrap_or_else(|| anchor.clone());
                    return Ok(BridgeDiffSummary {
                        session_id: session_id.to_string(),
                        message_id: message_id.map(ToString::to_string),
                        additions,
                        deletions,
                        files: vec![BridgeFileDiffSummary {
                            file: file_label,
                            additions,
                            deletions,
                            before: String::new(),
                            after: diff_text,
                        }],
                    });
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {}
                Err(e) => return Err(e.into()),
            }
        }

        let (after, additions, deletions) = diff::git_worktree_diff(root, diff_rel);
        let before = String::new();
        let anchor = message_id
            .map(|m| format!("(live git diff; session anchor {session_id} @ {m}; no snapshot row)"))
            .unwrap_or_else(|| format!("(live git diff; session {session_id})"));
        let file_label = diff_rel
            .map(str::to_string)
            .unwrap_or_else(|| anchor.clone());
        Ok(BridgeDiffSummary {
            session_id: session_id.to_string(),
            message_id: message_id.map(ToString::to_string),
            additions,
            deletions,
            files: vec![BridgeFileDiffSummary {
                file: file_label,
                additions,
                deletions,
                before: before.clone(),
                after: after.clone(),
            }],
        })
    }

    pub(super) fn persist_workspace_tree_hash(
        conn: &Connection,
        session_id: &str,
        anchor_message_id: &str,
        tree_hash: &str,
    ) -> Result<()> {
        let t = now_ms();
        conn.execute(
            "INSERT INTO workspace_tree_snapshots (session_id, anchor_message_id, tree_hash, captured_ms) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(session_id, anchor_message_id) DO UPDATE SET \
               tree_hash = excluded.tree_hash, captured_ms = excluded.captured_ms",
            params![session_id, anchor_message_id, tree_hash, t],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(super) fn query_last_assistant_tree_before_sort(
        conn: &Connection,
        session_id: &str,
        before_sort: i64,
    ) -> Result<Option<String>> {
        let mut stmt = conn
            .prepare(
                "SELECT w.tree_hash FROM workspace_tree_snapshots w \
                 INNER JOIN messages m ON m.id = w.anchor_message_id AND m.session_id = w.session_id \
                 WHERE w.session_id = ?1 AND m.role = 'assistant' AND m.sort_order < ?2 \
                 ORDER BY m.sort_order DESC LIMIT 1",
            )
            .context("prepare last assistant tree")?;
        let mut rows =
            stmt.query_map(params![session_id, before_sort], |r| r.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(s)) => Ok(Some(s)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    #[allow(dead_code)]
    pub(super) fn query_session_baseline_tree(
        conn: &Connection,
        session_id: &str,
    ) -> Result<Option<String>> {
        match conn.query_row(
            "SELECT tree_hash FROM workspace_tree_snapshots \
             WHERE session_id = ?1 AND anchor_message_id = ?2",
            params![session_id, SESSION_BASELINE_ANCHOR],
            |r| r.get::<_, String>(0),
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    #[allow(dead_code)]
    pub fn revert_blocking(
        &self,
        session_id: &str,
        request: &BridgeRevertRequest,
    ) -> Result<BridgeRevertSummary> {
        let snap_git = WorkspaceSnapshotGit::new(self.inner.source_root.clone());

        let (mids, snap_val, target_opt) = {
            let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
            let sort_order: i64 = db.query_row(
                "SELECT sort_order FROM messages WHERE id = ?1 AND session_id = ?2",
                params![request.message_id, session_id],
                |r| r.get(0),
            )?;
            let mids: Vec<String> = db
                .prepare(
                    "SELECT id FROM messages WHERE session_id = ?1 AND sort_order >= ?2 ORDER BY sort_order",
                )?
                .query_map(params![session_id, sort_order], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            if mids.is_empty() {
                let t = now_ms();
                db.execute(
                    "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
                    params![t, session_id],
                )?;
                return Ok(BridgeRevertSummary {
                    session_id: session_id.to_string(),
                    message_id: request.message_id.clone(),
                    part_id: request.part_id.clone(),
                    reverted: true,
                });
            }

            let first_so: i64 = db.query_row(
                "SELECT sort_order FROM messages WHERE id = ?1 AND session_id = ?2",
                params![&mids[0], session_id],
                |r| r.get(0),
            )?;

            let mut target_opt =
                Self::query_last_assistant_tree_before_sort(&db, session_id, first_so)?;
            if target_opt.is_none() {
                target_opt = Self::query_session_baseline_tree(&db, session_id)?;
            }

            let snap_val = Self::collect_revert_snapshot(&db, session_id, &mids)?;
            drop(db);
            (mids, snap_val, target_opt)
        };

        let mut snap_val = snap_val;
        if let Some(ref tgt) = target_opt {
            match snap_git.track() {
                Ok(baseline) => match snap_git.restore_worktree(tgt) {
                    Ok(()) => {
                        if let Value::Object(ref mut o) = snap_val {
                            o.insert("workspace_baseline_tree".to_string(), json!(baseline));
                            o.insert("workspace_target_tree".to_string(), json!(tgt));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            %e,
                            "revert: workspace restore failed; continuing message-only revert"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(%e, "revert: baseline track failed; skipping file restore");
                }
            }
        }

        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        if !mids.is_empty() {
            let snap_s = snap_val.to_string();
            db.execute(
                "INSERT INTO revert_journal (session_id, snapshot_json, created_ms) VALUES (?1, ?2, ?3)",
                params![session_id, snap_s, now_ms()],
            )
            .context("insert revert journal")?;
        }
        for mid in &mids {
            db.execute(
                "DELETE FROM workspace_tree_snapshots WHERE session_id = ?1 AND anchor_message_id = ?2",
                params![session_id, mid],
            )
            .ok();
        }
        for mid in &mids {
            db.execute("DELETE FROM parts WHERE message_id = ?1", params![mid])?;
        }
        for mid in &mids {
            db.execute(
                "DELETE FROM messages WHERE id = ?1 AND session_id = ?2",
                params![mid, session_id],
            )?;
        }
        let t = now_ms();
        db.execute(
            "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
            params![t, session_id],
        )?;
        Ok(BridgeRevertSummary {
            session_id: session_id.to_string(),
            message_id: request.message_id.clone(),
            part_id: request.part_id.clone(),
            reverted: true,
        })
    }

    #[allow(dead_code)]
    pub(super) fn collect_revert_snapshot(
        conn: &Connection,
        session_id: &str,
        mids: &[String],
    ) -> Result<Value> {
        let mut messages = Vec::new();
        for mid in mids {
            let row: (String, i64, String) = conn.query_row(
                "SELECT role, sort_order, info_json FROM messages WHERE id = ?1 AND session_id = ?2",
                params![mid, session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let mut pstm = conn.prepare(
                "SELECT id, sort_order, json FROM parts WHERE message_id = ?1 ORDER BY sort_order",
            )?;
            let parts: Vec<Value> = pstm
                .query_map(params![mid], |r| {
                    let pid: String = r.get(0)?;
                    let ps: i64 = r.get(1)?;
                    let pj: String = r.get(2)?;
                    Ok(json!({"id": pid, "sort_order": ps, "json": pj}))
                })?
                .collect::<Result<_, _>>()?;
            messages.push(json!({
                "id": mid,
                "role": row.0,
                "sort_order": row.1,
                "info_json": row.2,
                "parts": parts,
            }));
        }
        Ok(json!({ "messages": messages }))
    }

    #[allow(dead_code)]
    pub fn unrevert_blocking(&self, session_id: &str) -> Result<BridgeUnrevertSummary> {
        let db = self.inner.db.lock().map_err(|_| anyhow!("db poison"))?;
        let journal: Result<(i64, String), rusqlite::Error> = db.query_row(
            "SELECT id, snapshot_json FROM revert_journal WHERE session_id = ?1 ORDER BY id DESC LIMIT 1",
            params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
        let (jid, snap_s) = match journal {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(BridgeUnrevertSummary {
                    session_id: session_id.to_string(),
                    restored: false,
                });
            }
            Err(e) => return Err(e.into()),
        };
        let snap: Value = serde_json::from_str(&snap_s).context("parse revert snapshot")?;
        let workspace_baseline = snap
            .get("workspace_baseline_tree")
            .and_then(Value::as_str)
            .map(str::to_string);
        let Some(arr) = snap.get("messages").and_then(Value::as_array) else {
            anyhow::bail!("invalid revert snapshot");
        };
        let next_base: i64 = db.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM messages WHERE session_id = ?1",
            params![session_id],
            |r| r.get(0),
        )?;
        let mut order = next_base;
        for m in arr {
            let id = m
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("snapshot message id"))?;
            let role = m.get("role").and_then(Value::as_str).unwrap_or("user");
            let info_json = m.get("info_json").and_then(Value::as_str).unwrap_or("{}");
            db.execute(
                "INSERT INTO messages (id, session_id, role, sort_order, info_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, session_id, role, order, info_json],
            )
            .with_context(|| format!("restore message {id}"))?;
            if let Some(parts) = m.get("parts").and_then(Value::as_array) {
                for (pi, p) in parts.iter().enumerate() {
                    let pid = p
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("part id"))?;
                    let ps: i64 = p
                        .get("sort_order")
                        .and_then(Value::as_i64)
                        .unwrap_or(pi as i64);
                    let pj = p.get("json").and_then(Value::as_str).unwrap_or("{}");
                    db.execute(
                        "INSERT INTO parts (id, message_id, session_id, sort_order, json) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![pid, id, session_id, ps, pj],
                    )
                    .with_context(|| format!("restore part {pid}"))?;
                }
            }
            order += 1;
        }
        if let Some(ref h) = workspace_baseline {
            if let Err(e) =
                WorkspaceSnapshotGit::new(self.inner.source_root.clone()).restore_worktree(h)
            {
                tracing::warn!(%e, "unrevert: workspace baseline restore failed");
            }
        }
        db.execute("DELETE FROM revert_journal WHERE id = ?1", params![jid])
            .context("delete revert journal")?;
        let t = now_ms();
        db.execute(
            "UPDATE sessions SET updated_ms = ?1 WHERE id = ?2",
            params![t, session_id],
        )?;
        Ok(BridgeUnrevertSummary {
            session_id: session_id.to_string(),
            restored: true,
        })
    }
}
