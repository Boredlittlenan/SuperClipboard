use chrono::{DateTime, Days, Local, NaiveDateTime, Offset, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Once,
    Daily,
    Weekly,
}

impl Repeat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
        }
    }
}

pub fn parse(value: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .map_err(|_| "Invalid reminder date/time".into())
}

fn resolve(zone: &impl TimeZone, value: NaiveDateTime) -> Option<DateTime<Utc>> {
    // Select the first occurrence during a repeated hour. Move forward through a DST gap.
    for minutes in 0..=180 {
        let shifted = value.checked_add_signed(chrono::Duration::minutes(minutes))?;
        if let Some(time) = zone.from_local_datetime(&shifted).earliest() {
            return Some(time.with_timezone(&Utc));
        }
    }
    None
}

pub fn initial(value: NaiveDateTime) -> Result<i64, String> {
    resolve(&Local, value)
        .map(|time| time.timestamp())
        .ok_or_else(|| "Invalid local time".into())
}

pub fn next(start: NaiveDateTime, repeat: &str, now: DateTime<Utc>) -> Option<i64> {
    let days = match repeat {
        "daily" => 1,
        "weekly" => 7,
        _ => return None,
    };
    let local_now = now.with_timezone(&Local);
    let elapsed = (local_now.date_naive() - start.date()).num_days().max(0) as u64;
    let date = start
        .date()
        .checked_add_days(Days::new((elapsed / days) * days))?;
    let candidate = resolve(&Local, date.and_time(start.time()))?;
    if candidate > now {
        return Some(candidate.timestamp());
    }
    let date = date.checked_add_days(Days::new(days))?;
    resolve(&Local, date.and_time(start.time())).map(|time| time.timestamp())
}

pub fn zone_offset() -> i32 {
    Local::now().offset().fix().local_minus_utc()
}
