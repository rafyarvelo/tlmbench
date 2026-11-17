//! Events you add (markers and ranges), kept per recording in the app cache by the recording's identity: nothing is
//! written beside the data. An event is a name, a start, a duration (0 is an instant), a category and a source; it may
//! list the channels it concerns. Times are decimal strings on the wire: JSON numbers lose i64 nanoseconds.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Opened, Result};

/// One event.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Stable, so a shared event can be merged and edited by id.
    pub id: String,
    pub name: String,
    /// Nanoseconds, as the recording's own times are.
    pub start: String,
    /// Nanoseconds; "0" is an instant.
    pub duration: String,
    /// A category's id (empty: uncategorized).
    #[serde(default)]
    pub category: String,
    /// `user` for what you added; `file`, `rule` and `check` are read-only and come from elsewhere.
    #[serde(default = "user")]
    pub source: String,
    /// Channels it concerns, by name.
    #[serde(default)]
    pub channels: Vec<String>,
    /// Who added it (the account name), shown with the event and kept when it is shared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

fn user() -> String {
    "user".into()
}

fn path(root: &Path, fingerprint: &str) -> PathBuf {
    root.join("events").join(format!("{fingerprint}.json"))
}

/// The events kept for the recording with this fingerprint, in start order.
/// The most events one recording's marked columns make: a noisy column can't flood the list.
pub const MAX_FILE_EVENTS: usize = 20_000;

/// The events a recording's marked channels make: one at each change of the shown value, named for the new value (its
/// text for a text channel) and listing the channel. Read-only, source `file`, in the `mode` category; the first sample
/// is the starting state, not a change. Missing values are skipped.
pub fn from_channels(rec: &dyn tlmbench_core::Recording, channels: &[usize]) -> Vec<Event> {
    let info = rec.info();
    let held = tlmbench_core::chunks::Held::default();
    let mut out = Vec::new();
    for &ch in channels {
        let Some(c) = info.channels.get(ch) else { continue };
        let mut prev: Option<f64> = None;
        for i in 0..info.samples {
            let v = rec.value_at(ch, i, &held);
            if v.is_nan() {
                continue;
            }
            if prev.is_some_and(|p| p != v) {
                let text = if v >= 0.0 && v.fract() == 0.0 && (v as usize) < c.labels.len() {
                    c.labels[v as usize].clone()
                } else {
                    format!("{v}")
                };
                out.push(Event {
                    id: format!("file-{ch}-{i}"),
                    name: text,
                    start: rec.time_at(i).to_string(),
                    duration: "0".into(),
                    category: "mode".into(),
                    source: "file".into(),
                    channels: vec![c.name.clone()],
                    by: None,
                });
                if out.len() >= MAX_FILE_EVENTS {
                    return out;
                }
            }
            prev = Some(v);
        }
    }
    out.sort_by_key(|e| e.start.parse::<i64>().unwrap_or(0));
    out
}

pub fn list(root: &Path, fingerprint: &str) -> Result<Vec<Event>> {
    let p = path(root, fingerprint);
    let text = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(&p)(e)),
    };
    let mut v: Vec<Event> =
        serde_json::from_str(&text).map_err(|e| Error::Damaged { path: p.clone(), why: e.to_string() })?;
    v.sort_by_key(|e| e.start.parse::<i64>().unwrap_or(0));
    Ok(v)
}

fn write(root: &Path, fingerprint: &str, events: &[Event]) -> Result<()> {
    let p = path(root, fingerprint);
    let dir = p.parent().expect("a file in events/");
    fs::create_dir_all(dir).map_err(Error::io(dir))?;
    let tmp = p.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(events).expect("events serialize");
    fs::write(&tmp, json).map_err(Error::io(&tmp))?;
    fs::rename(&tmp, &p).map_err(Error::io(&p))
}

/// Adds `event` or replaces the one with its id; the events now.
pub fn save(root: &Path, fingerprint: &str, event: Event) -> Result<Vec<Event>> {
    for (what, t) in [("start", &event.start), ("duration", &event.duration)] {
        t.parse::<i64>().map_err(|_| Error::Invalid(format!("An event's {what} is a whole number of nanoseconds")))?;
    }
    if event.duration.starts_with('-') {
        return Err(Error::Invalid("An event can't last a negative time".into()));
    }
    let mut all = list(root, fingerprint)?;
    match all.iter_mut().find(|e| e.id == event.id) {
        Some(e) => *e = event,
        None => all.push(event),
    }
    all.sort_by_key(|e| e.start.parse::<i64>().unwrap_or(0));
    write(root, fingerprint, &all)?;
    Ok(all)
}

/// Removes the event with this id; the events now.
pub fn delete(root: &Path, fingerprint: &str, id: &str) -> Result<Vec<Event>> {
    let mut all = list(root, fingerprint)?;
    all.retain(|e| e.id != id);
    write(root, fingerprint, &all)?;
    Ok(all)
}

impl Opened {
    /// The identity events are kept under: the source file's contents (a derived or adjusted view of it shares it).
    pub fn identity(&self) -> &str {
        &self.fingerprint
    }
}
