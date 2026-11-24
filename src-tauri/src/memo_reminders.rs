use crate::{
    remote_storage, run_storage,
    storage::{Memo, Storage},
    AppState,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{Emitter, Manager};
use tokio::sync::Notify;
use uuid::Uuid;

const LOCAL: &str = "local";
pub(crate) const MAX_DUE_AT: i64 = 253_402_300_799_999;
const CHANGE_EVENT: &str = "memo-reminders-changed";
const ACTIVATION_EVENT: &str = "memo-reminder-activated";
const PROTOCOL: &str = "superclipboard-reminder://open/";
const RESCAN: Duration = Duration::from_secs(15);
static WAKE: std::sync::LazyLock<Notify> = std::sync::LazyLock::new(Notify::new);

#[derive(Debug)]
struct Reminder {
    memo_id: i64,
    memo_key: String,
    token: String,
    due_at: i64,
    triggered_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderItem {
    memo_id: i64,
    pub(crate) token: String,
    pub(crate) title: String,
    due_at: i64,
    triggered_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderList {
    source: String,
    items: Vec<ReminderItem>,
}

struct Target {
    key: String,
    title: String,
    created_at: String,
}

#[derive(Default)]
pub struct ReminderActivation(Mutex<Option<String>>);

fn current_source(storage: &Storage) -> Result<String, String> {
    if storage
        .get_setting("storage_mode")
        .map_err(|e| e.to_string())?
        .as_deref()
        != Some("remote")
    {
        return Ok(LOCAL.into());
    }
    if !remote_storage::is_remote_mode(storage) {
        return Err("Remote memo storage is not ready.".into());
    }
    remote_storage::memo_reminder_source(storage).map_err(|e| e.to_string())
}

fn check_source(storage: &Storage, source: &str) -> Result<(), String> {
    if current_source(storage)? == source {
        Ok(())
    } else {
        Err("The memo data source changed.".into())
    }
}

fn reminder_rows(
    storage: &Storage,
    source: &str,
    due_by: Option<i64>,
) -> rusqlite::Result<Vec<Reminder>> {
    let conn = storage.conn.lock().unwrap();
    let (sql, args): (&str, &[&dyn rusqlite::ToSql]) = match &due_by {
        Some(now) => (
            "SELECT memo_id, memo_key, token, due_at, triggered_at FROM memo_reminders
             WHERE source = ?1 AND triggered_at IS NULL AND due_at <= ?2 ORDER BY due_at, memo_id",
            &[&source, now],
        ),
        None => (
            "SELECT memo_id, memo_key, token, due_at, triggered_at FROM memo_reminders
             WHERE source = ?1 ORDER BY due_at, memo_id",
            &[&source],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(args, |row| {
            Ok(Reminder {
                memo_id: row.get(0)?,
                memo_key: row.get(1)?,
                token: row.get(2)?,
                due_at: row.get(3)?,
                triggered_at: row.get(4)?,
            })
        })?
        .collect();
    rows
}

fn targets(storage: &Storage, source: &str, ids: &[i64]) -> Result<HashMap<i64, Target>, String> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    if source != LOCAL {
        return remote_storage::memo_reminder_targets(storage, source, ids)
            .map(|targets| {
                targets
                    .into_iter()
                    .map(|(id, key, title, created_at)| {
                        (
                            id,
                            Target {
                                key,
                                title,
                                created_at,
                            },
                        )
                    })
                    .collect()
            })
            .map_err(|e| e.to_string());
    }
    let conn = storage.conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT created_at, title FROM memos WHERE id = ?1 AND archived_at IS NULL")
        .map_err(|e| e.to_string())?;
    let mut targets = HashMap::new();
    for &id in ids {
        if let Some(target) = stmt
            .query_row(params![id], |row| {
                let created_at: String = row.get(0)?;
                Ok(Target {
                    key: created_at.clone(),
                    title: row.get(1)?,
                    created_at,
                })
            })
            .optional()
            .map_err(|e| e.to_string())?
        {
            targets.insert(id, target);
        }
    }
    Ok(targets)
}

fn match_rows(
    storage: &Storage,
    source: &str,
    rows: Vec<Reminder>,
    trigger_at: Option<i64>,
) -> Result<Vec<ReminderItem>, String> {
    let ids: Vec<_> = rows.iter().map(|row| row.memo_id).collect();
    let targets = targets(storage, source, &ids)?;
    check_source(storage, source)?;
    let mut conn = storage.conn.lock().unwrap();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for row in rows {
        let Some(target) = targets
            .get(&row.memo_id)
            .filter(|target| target.key == row.memo_key)
        else {
            tx.execute("DELETE FROM memo_reminders WHERE token = ?1", [&row.token])
                .map_err(|e| e.to_string())?;
            continue;
        };
        if let Some(now) = trigger_at {
            let claimed = tx
                .execute(
                    "UPDATE memo_reminders SET triggered_at = ?1
                 WHERE token = ?2 AND triggered_at IS NULL AND due_at <= ?1",
                    params![now, row.token],
                )
                .map_err(|e| e.to_string())?;
            if claimed == 0 {
                continue;
            }
        }
        items.push(ReminderItem {
            memo_id: row.memo_id,
            token: row.token,
            title: target.title.clone(),
            due_at: row.due_at,
            triggered_at: trigger_at.or(row.triggered_at),
        });
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(items)
}

fn list(storage: &Storage) -> Result<ReminderList, String> {
    let source = current_source(storage)?;
    let rows = reminder_rows(storage, &source, None).map_err(|e| e.to_string())?;
    let items = match_rows(storage, &source, rows, None)?;
    Ok(ReminderList { source, items })
}

fn schedule(
    storage: &Storage,
    source: &str,
    memo_id: i64,
    created_at: &str,
    due_at: i64,
) -> Result<(), String> {
    check_source(storage, source)?;
    if due_at <= Utc::now().timestamp_millis() || due_at > MAX_DUE_AT {
        return Err("The reminder time must be in the future.".into());
    }
    let mut targets = targets(storage, source, &[memo_id])?;
    let target = targets
        .remove(&memo_id)
        .filter(|target| target.created_at == created_at)
        .ok_or("This memo is no longer available.")?;
    check_source(storage, source)?;
    let conn = storage.conn.lock().unwrap();
    let updated = conn
        .execute(
            "INSERT INTO memo_reminders (source, memo_id, memo_key, token, due_at)
         SELECT ?1, ?2, ?3, ?4, ?5 WHERE ?1 <> 'local' OR EXISTS (
             SELECT 1 FROM memos WHERE id = ?2 AND created_at = ?3 AND archived_at IS NULL
         )
         ON CONFLICT(source, memo_id) DO UPDATE SET memo_key = excluded.memo_key,
             token = excluded.token, due_at = excluded.due_at, triggered_at = NULL",
            params![
                source,
                memo_id,
                target.key,
                Uuid::new_v4().to_string(),
                due_at
            ],
        )
        .map_err(|e| e.to_string())?;
    if updated == 0 {
        return Err("This memo is no longer available.".into());
    }
    Ok(())
}

fn change(storage: &Storage, token: &str, snooze: bool) -> Result<(), String> {
    let source = current_source(storage)?;
    let conn = storage.conn.lock().unwrap();
    let changed = if snooze {
        conn.execute(
            "UPDATE memo_reminders SET token = ?1, due_at = ?2, triggered_at = NULL
             WHERE source = ?3 AND token = ?4 AND triggered_at IS NOT NULL",
            params![
                Uuid::new_v4().to_string(),
                Utc::now().timestamp_millis() + 600_000,
                source,
                token
            ],
        )
    } else {
        conn.execute(
            "DELETE FROM memo_reminders WHERE source = ?1 AND token = ?2",
            params![source, token],
        )
    }
    .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("This reminder changed. Please reopen it.".into());
    }
    Ok(())
}

fn open(storage: &Storage, token: &str) -> Result<Option<Memo>, String> {
    let source = current_source(storage)?;
    let target: Option<(i64, String)> = {
        let conn = storage.conn.lock().unwrap();
        conn.query_row(
            "SELECT memo_id, memo_key FROM memo_reminders WHERE source = ?1 AND token = ?2",
            params![source, token],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
    };
    let Some((id, key)) = target else {
        return Ok(None);
    };
    let memo = if source == LOCAL {
        storage
            .get_active_memo(id)
            .map_err(|e| e.to_string())?
            .filter(|memo| memo.created_at == key)
    } else {
        remote_storage::reminder_memo(storage, &source, id, &key).map_err(|e| e.to_string())?
    };
    check_source(storage, &source)?;
    Ok(memo)
}

pub fn notify_scheduler() {
    WAKE.notify_one();
}

pub fn changed(app: &tauri::AppHandle) {
    notify_scheduler();
    let _ = app.emit(CHANGE_EVENT, ());
}

fn next_scan(app: &tauri::AppHandle, storage: &Storage) -> Result<Duration, String> {
    let source = current_source(storage)?;
    let now = Utc::now().timestamp_millis();
    let rows = reminder_rows(storage, &source, Some(now)).map_err(|e| e.to_string())?;
    if !rows.is_empty() {
        let items = match_rows(storage, &source, rows, Some(now))?;
        if !items.is_empty() {
            let include_title = storage
                .get_setting("memo_reminder_show_title")
                .ok()
                .flatten()
                .as_deref()
                == Some("true");
            let language = storage.get_setting("language").ok().flatten();
            if check_source(storage, &source).is_ok() {
                super::memo_reminder_windows::show(
                    app,
                    &items,
                    include_title,
                    language.as_deref() != Some("en"),
                );
            }
        }
        let _ = app.emit(CHANGE_EVENT, ());
    }
    let conn = storage.conn.lock().unwrap();
    let next: Option<i64> = conn
        .query_row(
            "SELECT MIN(due_at) FROM memo_reminders WHERE source = ?1 AND triggered_at IS NULL",
            [&source],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(next
        .map(|due| Duration::from_millis((due - Utc::now().timestamp_millis()).max(0) as u64))
        .unwrap_or(RESCAN)
        .min(RESCAN))
}

pub fn start(app: tauri::AppHandle, storage: std::sync::Arc<Storage>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let worker_app = app.clone();
            let worker_storage = storage.clone();
            let delay = match tauri::async_runtime::spawn_blocking(move || {
                next_scan(&worker_app, &worker_storage)
            })
            .await
            {
                Ok(Ok(delay)) => delay,
                Ok(Err(error)) => {
                    log::debug!("Memo reminder scan deferred: {error}");
                    RESCAN
                }
                Err(error) => {
                    log::warn!("Memo reminder worker failed: {error}");
                    RESCAN
                }
            };
            let _ = tokio::time::timeout(delay, WAKE.notified()).await;
        }
    });
}

fn parse_activation(arg: &str) -> Option<String> {
    let target = arg.strip_prefix(PROTOCOL)?;
    if target == "list" || Uuid::parse_str(target).is_ok_and(|id| id.to_string() == target) {
        Some(target.to_string())
    } else {
        None
    }
}

pub fn deep_link(target: &str) -> String {
    format!("{PROTOCOL}{target}")
}

pub fn activate_from_args(app: &tauri::AppHandle, args: impl Iterator<Item = String>) {
    if let Some(target) = args.skip(1).find_map(|arg| parse_activation(&arg)) {
        *app.state::<ReminderActivation>().0.lock().unwrap() = Some(target);
        let _ = app.emit(ACTIVATION_EVENT, ());
    }
}

#[tauri::command]
pub fn take_memo_reminder_activation(
    state: tauri::State<'_, ReminderActivation>,
) -> Option<String> {
    state.0.lock().unwrap().take()
}

#[tauri::command]
pub async fn list_memo_reminders(
    state: tauri::State<'_, AppState>,
) -> Result<ReminderList, String> {
    run_storage(state.storage.clone(), list).await
}

#[tauri::command]
pub async fn schedule_memo_reminder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    source: String,
    memo_id: i64,
    created_at: String,
    due_at: i64,
) -> Result<(), String> {
    run_storage(state.storage.clone(), move |storage| {
        schedule(storage, &source, memo_id, &created_at, due_at)
    })
    .await?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn change_memo_reminder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    token: String,
    snooze: bool,
) -> Result<(), String> {
    run_storage(state.storage.clone(), move |storage| {
        change(storage, &token, snooze)
    })
    .await?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn open_memo_reminder(
    state: tauri::State<'_, AppState>,
    token: String,
) -> Result<Option<Memo>, String> {
    run_storage(state.storage.clone(), move |storage| open(storage, &token)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn store_with_memo() -> (Storage, Memo) {
        let storage = Storage::new(Path::new(":memory:")).unwrap();
        let memo = storage
            .create_memo("Reminder test", "Memo body", "", &[])
            .unwrap();
        (storage, memo)
    }

    #[test]
    fn rescheduling_and_snoozing_invalidate_the_previous_occurrence() {
        let (storage, memo) = store_with_memo();
        let due_at = Utc::now().timestamp_millis() + 60_000;
        schedule(&storage, LOCAL, memo.id, &memo.created_at, due_at).unwrap();
        let original = list(&storage).unwrap().items.remove(0).token;
        let stale_rows = reminder_rows(&storage, LOCAL, Some(due_at)).unwrap();

        schedule(&storage, LOCAL, memo.id, &memo.created_at, due_at + 60_000).unwrap();
        assert!(match_rows(&storage, LOCAL, stale_rows, Some(due_at))
            .unwrap()
            .is_empty());
        assert!(open(&storage, &original).unwrap().is_none());

        let revised_due = due_at + 60_000;
        let rows = reminder_rows(&storage, LOCAL, Some(revised_due)).unwrap();
        let fired = match_rows(&storage, LOCAL, rows, Some(revised_due)).unwrap();
        assert_eq!(fired.len(), 1);
        let token = &fired[0].token;
        assert_eq!(open(&storage, token).unwrap().unwrap().id, memo.id);
        assert!(reminder_rows(&storage, LOCAL, Some(revised_due))
            .unwrap()
            .is_empty());

        change(&storage, token, true).unwrap();
        assert!(change(&storage, token, false).is_err());
        let snoozed = list(&storage).unwrap().items.remove(0);
        assert_eq!(snoozed.triggered_at, None);
        assert_ne!(snoozed.token, *token);
        change(&storage, &snoozed.token, false).unwrap();
        assert!(list(&storage).unwrap().items.is_empty());
    }

    #[test]
    fn archive_cancels_and_backup_restores_only_matching_local_reminders() {
        let (storage, memo) = store_with_memo();
        let due_at = Utc::now().timestamp_millis() + 60_000;
        schedule(&storage, LOCAL, memo.id, &memo.created_at, due_at).unwrap();
        let token = list(&storage).unwrap().items.remove(0).token;
        let backup = storage.export_backup_data("3.7.0").unwrap();
        storage.archive_memo(memo.id).unwrap();
        storage.unarchive_memo(memo.id).unwrap();
        assert!(list(&storage).unwrap().items.is_empty());
        assert!(schedule(
            &storage,
            "postgres:other",
            memo.id,
            &memo.created_at,
            due_at
        )
        .is_err());
        assert!(schedule(&storage, LOCAL, memo.id, "wrong creation date", due_at).is_err());

        storage.restore_backup_data(&backup).unwrap();
        let restored = list(&storage).unwrap().items.remove(0);
        assert_eq!(restored.due_at, due_at);
        assert_ne!(restored.token, token);
        storage.delete_memo(memo.id, false).unwrap();
        assert!(list(&storage).unwrap().items.is_empty());
    }

    #[test]
    fn only_explicit_reminder_links_are_accepted() {
        let token = Uuid::new_v4().to_string();
        assert_eq!(parse_activation(&deep_link(&token)), Some(token.clone()));
        assert_eq!(parse_activation(&deep_link("list")), Some("list".into()));
        for invalid in [
            format!("{token}?q=1"),
            "list/".into(),
            "123".into(),
            token.to_uppercase(),
        ] {
            assert!(parse_activation(&deep_link(&invalid)).is_none());
        }
        assert!(parse_activation("https://example.com/").is_none());
    }
}
