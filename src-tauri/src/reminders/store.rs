use super::clock::{self, Repeat};
use chrono::{Local, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use std::{path::Path, time::Duration};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub memo_id: i64,
    pub title: String,
    pub at: String,
    pub repeat: String,
    pub status: String,
    pub next_due: i64,
    pub last_sent: Option<i64>,
    pub error: Option<String>,
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reminders (
             workspace TEXT NOT NULL, memo_id INTEGER NOT NULL, title TEXT NOT NULL,
             local_time TEXT NOT NULL, repeat TEXT NOT NULL, status TEXT NOT NULL,
             next_due INTEGER NOT NULL, last_sent INTEGER,
             retry_at INTEGER NOT NULL DEFAULT 0, error TEXT,
             PRIMARY KEY (workspace, memo_id)
         );
         CREATE INDEX IF NOT EXISTS idx_reminder_due ON reminders(next_due) WHERE status = 'active';",
    )?;
    Ok(conn)
}

pub fn list(path: &Path, workspace: &str) -> Result<Vec<Reminder>, String> {
    let conn = open(path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT memo_id, title, local_time, repeat, status, next_due, last_sent, error
         FROM reminders WHERE workspace = ?1 AND status != 'cancelled'
         ORDER BY status != 'active', next_due, memo_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([workspace], |row| {
            Ok(Reminder {
                memo_id: row.get(0)?,
                title: row.get(1)?,
                at: row.get(2)?,
                repeat: row.get(3)?,
                status: row.get(4)?,
                next_due: row.get(5)?,
                last_sent: row.get(6)?,
                error: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

pub fn save(
    path: &Path,
    workspace: &str,
    id: i64,
    title: &str,
    at: &str,
    repeat: Repeat,
) -> Result<(), String> {
    let date = clock::parse(at)?;
    let due = clock::initial(date)?;
    if due <= Utc::now().timestamp() {
        return Err("Choose a reminder time in the future".into());
    }
    let conn = open(path).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO reminders (workspace, memo_id, title, local_time, repeat, status, next_due)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)
         ON CONFLICT (workspace, memo_id) DO UPDATE SET title = excluded.title,
            local_time = excluded.local_time, repeat = excluded.repeat, status = 'active',
            next_due = excluded.next_due, last_sent = NULL, retry_at = 0, error = NULL",
        params![workspace, id, title, at, repeat.as_str(), due],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn finish(path: &Path, workspace: &str, id: i64, status: &str) -> Result<(), String> {
    let conn = open(path).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE reminders SET status = ?1, error = NULL WHERE workspace = ?2 AND memo_id = ?3",
        params![status, workspace, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn needs_schedule(path: &Path) -> rusqlite::Result<bool> {
    open(path)?.query_row(
        "SELECT EXISTS (SELECT 1 FROM reminders WHERE status = 'active')",
        [],
        |r| r.get(0),
    )
}

pub fn refresh_title(path: &Path, workspace: &str, id: i64, title: &str) -> Result<(), String> {
    open(path)
        .and_then(|conn| {
            conn.execute(
                "UPDATE reminders SET title = ?1 WHERE workspace = ?2 AND memo_id = ?3",
                params![title, workspace, id],
            )
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn tick(path: &Path, notify: impl Fn(&str) -> Result<(), String>) -> Result<bool, String> {
    let mut conn = open(path).map_err(|e| e.to_string())?;
    let mut changed = false;
    for _ in 0..64 {
        if !deliver_one(&mut conn, &notify).map_err(|e| e.to_string())? {
            break;
        }
        changed = true;
    }
    Ok(changed)
}

fn deliver_one(
    conn: &mut Connection,
    notify: &impl Fn(&str) -> Result<(), String>,
) -> rusqlite::Result<bool> {
    let now = Utc::now();
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let due = tx
        .query_row(
            "SELECT workspace, memo_id, title, local_time, repeat FROM reminders
         WHERE status = 'active' AND next_due <= ?1 AND retry_at <= ?1 ORDER BY next_due LIMIT 1",
            [now.timestamp()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((workspace, id, title, local_time, repeat)) = due else {
        return Ok(false);
    };
    match notify(&title) {
        Ok(()) => {
            let next = clock::parse(&local_time)
                .ok()
                .and_then(|start| clock::next(start, &repeat, now));
            tx.execute(
                "UPDATE reminders SET status = ?1, next_due = COALESCE(?2, next_due), last_sent = ?3,
                   retry_at = 0, error = NULL WHERE workspace = ?4 AND memo_id = ?5",
                params![if next.is_some() { "active" } else { "fired" }, next, now.timestamp(), workspace, id],
            )?;
        }
        Err(error) => {
            tx.execute(
                "UPDATE reminders SET retry_at = ?1, error = ?2 WHERE workspace = ?3 AND memo_id = ?4",
                params![now.timestamp() + 60, error, workspace, id],
            )?;
        }
    }
    tx.commit()?;
    Ok(true)
}

pub fn recalculate(path: &Path) -> rusqlite::Result<()> {
    let mut conn = open(path)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let rows = {
        let mut stmt = tx.prepare("SELECT workspace, memo_id, local_time, repeat, last_sent FROM reminders WHERE status = 'active'")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (workspace, id, at, repeat, last_sent) in rows {
        let Ok(at) = clock::parse(&at) else {
            continue;
        };
        let now = Utc::now();
        let next = match last_sent.and_then(|sent| chrono::DateTime::from_timestamp(sent, 0)) {
            Some(original) => {
                // After delivery, keep the previously scheduled calendar occurrence on the new local clock.
                let last_local_day = original.with_timezone(&Local).date_naive();
                let end_of_day = last_local_day
                    .and_hms_opt(23, 59, 59)
                    .and_then(|time| time.and_local_timezone(Local).latest());
                end_of_day.and_then(|time| clock::next(at, &repeat, time.with_timezone(&Utc)))
            }
            None => clock::initial(at).ok(),
        };
        if let Some(next) = next.filter(|next| *next > now.timestamp()) {
            tx.execute(
                "UPDATE reminders SET next_due = ?1 WHERE workspace = ?2 AND memo_id = ?3",
                params![next, workspace, id],
            )?;
        }
    }
    tx.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_due(path: &Path, id: i64, repeat: Repeat) {
        let now = Utc::now();
        let start = (now - chrono::Duration::days(14))
            .with_timezone(&Local)
            .format("%Y-%m-%dT%H:%M")
            .to_string();
        open(path).unwrap().execute(
            "INSERT INTO reminders (workspace, memo_id, title, local_time, repeat, status, next_due)
             VALUES ('local', ?1, 'Reminder test', ?2, ?3, 'active', ?4)",
            params![id, start, repeat.as_str(), now.timestamp() - 300],
        ).unwrap();
    }

    #[test]
    fn overdue_occurrences_fire_once_and_repeating_tasks_advance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reminders.db");
        add_due(&path, 1, Repeat::Once);
        add_due(&path, 2, Repeat::Weekly);
        add_due(&path, 3, Repeat::Daily);
        finish(&path, "local", 3, "cancelled").unwrap();

        let count = std::cell::Cell::new(0);
        let notify = |_: &str| {
            count.set(count.get() + 1);
            Ok(())
        };
        assert!(tick(&path, notify).unwrap());
        assert!(!tick(&path, notify).unwrap());
        assert_eq!(count.get(), 2);
        let reminders = list(&path, "local").unwrap();
        assert_eq!(reminders.len(), 2);
        assert_eq!(reminders[1].status, "fired");
        assert_eq!(reminders[0].status, "active");
        assert!(reminders[0].next_due > Utc::now().timestamp());
    }

    #[test]
    fn failed_notification_is_retained_for_retry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reminders.db");
        add_due(&path, 1, Repeat::Once);
        assert!(tick(&path, |_| Err("Toast service unavailable".into())).unwrap());
        let reminder = list(&path, "local").unwrap().remove(0);
        assert_eq!(reminder.status, "active");
        assert_eq!(reminder.error.as_deref(), Some("Toast service unavailable"));
        assert!(!tick(&path, |_| panic!("Must back off after failures")).unwrap());
    }
}
