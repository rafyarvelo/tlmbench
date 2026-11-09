//! Import profiles: what the user decided about a CSV file's columns, time and rate, applied when it is decoded.
//!
//! A profile is keyed by the file's header (see [`header_key`]): the same sorted column names always find the same
//! profile. It is applied by [`crate::csv::scan_with`] (names, units, types, the time column) and by the cache build
//! (the rate); its [`ImportProfile::key`] joins the cache key, so a different profile decodes into its own entry.
//! The app keeps profiles as JSON files in its cache; `tlmbench` takes one with `--profile`.

use serde::{Deserialize, Serialize};

/// How the x axis comes from the file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimeChoice {
    /// The first column that looks like a time, else the row number.
    #[default]
    Auto,
    /// This column (by its header name), in `s`, `ms`, `us`, `ns` or `iso` (dates and times as text).
    /// A unit of `None` takes the column name's own unit, else seconds.
    Column { name: String, unit: Option<String> },
    /// No time column: the row number.
    None,
}

/// What to do about the sample rate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RateChoice {
    /// Times as recorded (a time column's own values, or the row number).
    #[default]
    AsRecorded,
    /// Linear interpolation onto an even grid at this rate, from the first time.
    Resample { hz: f64 },
    /// Ignore the time column: sample `i` is at `i / hz`.
    Assume { hz: f64 },
    /// Ignore the time column: the x axis is the sample number.
    Samples,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    #[default]
    Number,
    /// Kept out of the recording (the core has no text channels).
    Text,
    Skip,
}

/// One column's choices, found by the header name as the file spells it (after the unit is split off and repeated
/// names are numbered, as [`crate::csv::scan`] names them).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnChoice {
    pub source: String,
    /// The channel's name, when not the header's.
    pub name: Option<String>,
    pub unit: Option<String>,
    /// `None`: as detected.
    pub kind: Option<ColumnKind>,
    /// A group label for the channel list.
    pub category: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportProfile {
    pub time: TimeChoice,
    pub rate: RateChoice,
    pub columns: Vec<ColumnChoice>,
    /// Check sets that run on files with this profile (by name). They change nothing about how the file opens.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub check_sets: Vec<String>,
}

impl ImportProfile {
    /// True when the profile changes nothing (the file opens as if it had none).
    pub fn is_default(&self) -> bool {
        self == &Self { check_sets: self.check_sets.clone(), ..Self::default() }
    }

    /// A short hash of the profile's meaning, for the cache key. Choices equal to the defaults don't count, and
    /// column choices count in the order of their sources, so equal profiles always share an entry.
    pub fn key(&self) -> String {
        let mut p = self.clone();
        p.check_sets.clear();
        p.columns.retain(|c| *c != ColumnChoice { source: c.source.clone(), ..Default::default() });
        p.columns.sort_by(|a, b| a.source.cmp(&b.source));
        let json = serde_json::to_vec(&p).expect("profiles serialize");
        blake3::hash(&json).to_hex()[..12].to_owned()
    }

    /// The choice for a column, by its source name.
    pub fn column(&self, source: &str) -> Option<&ColumnChoice> {
        self.columns.iter().find(|c| c.source == source)
    }

    pub fn rate_hz(&self) -> Option<f64> {
        match self.rate {
            RateChoice::Resample { hz } | RateChoice::Assume { hz } => Some(hz),
            _ => None,
        }
    }

    /// Checks the rate is usable.
    pub fn validate(&self) -> Result<(), String> {
        match self.rate_hz() {
            Some(hz) if !(hz.is_finite() && hz > 0.0) => Err(format!("the sample rate must be positive, not {hz}")),
            _ => Ok(()),
        }
    }
}

/// The key profiles are found by: the column names of the header, sorted and joined (order doesn't matter, and
/// units in brackets are part of the names as the file spells them).
pub fn header_key(names: &[String]) -> String {
    let mut sorted: Vec<&str> = names.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let mut h = blake3::Hasher::new();
    for n in sorted {
        h.update(n.as_bytes());
        h.update(&[0]);
    }
    h.finalize().to_hex()[..16].to_owned()
}

/// How much of a profile's header `names` shares with another header, from 0 to 1 (for suggesting a profile whose
/// header is close but not equal).
pub fn header_overlap(a: &[String], b: &[String]) -> f64 {
    let shared = a.iter().filter(|n| b.contains(n)).count();
    let total = a.len().max(b.len());
    if total == 0 { 0.0 } else { shared as f64 / total as f64 }
}

/// A profile as kept in the app cache: with the header it belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedProfile {
    /// The source names of the header, as [`crate::csv::scan`] names them.
    pub header: Vec<String>,
    pub profile: ImportProfile,
}

/// What [`find`] has for a header.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Found {
    /// A profile for exactly this header: applied without asking.
    pub exact: Option<ImportProfile>,
    /// The closest profile for a header that shares at least 60% of its names, with the share: offered, never
    /// applied silently.
    pub near: Option<(f64, SavedProfile)>,
}

fn profile_dir(root: &std::path::Path) -> std::path::PathBuf {
    root.join("profiles")
}

/// Saves the profile for a header under the cache `root`.
pub fn save(root: &std::path::Path, header: &[String], profile: &ImportProfile) -> crate::Result<()> {
    let dir = profile_dir(root);
    std::fs::create_dir_all(&dir).map_err(crate::error::io_err(&dir))?;
    let path = dir.join(format!("{}.json", header_key(header)));
    let saved = SavedProfile { header: header.to_vec(), profile: profile.clone() };
    let json = serde_json::to_vec_pretty(&saved).expect("profiles serialize");
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json).map_err(crate::error::io_err(&tmp))?;
    std::fs::rename(&tmp, &path).map_err(crate::error::io_err(&path))
}

/// Every saved profile, most similar to `header` first when one is given.
pub fn list(root: &std::path::Path) -> Vec<SavedProfile> {
    let Ok(rd) = std::fs::read_dir(profile_dir(root)) else { return Vec::new() };
    let mut out: Vec<SavedProfile> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| a.header.cmp(&b.header));
    out
}

/// Finds the profile for a header: the exact one, else the closest near one.
pub fn find(root: &std::path::Path, header: &[String]) -> Found {
    let key = header_key(header);
    let mut found = Found::default();
    for saved in list(root) {
        if header_key(&saved.header) == key {
            found.exact = Some(saved.profile);
            found.near = None;
            return found;
        }
        let share = header_overlap(header, &saved.header);
        if share >= 0.6 && found.near.as_ref().is_none_or(|(s, _)| share > *s) {
            found.near = Some((share, saved));
        }
    }
    found
}

/// Removes the profile for a header.
pub fn remove(root: &std::path::Path, header: &[String]) {
    let _ = std::fs::remove_file(profile_dir(root).join(format!("{}.json", header_key(header))));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_choices_do_not_change_the_key() {
        let a = ImportProfile::default();
        let b = ImportProfile { columns: vec![ColumnChoice { source: "x".into(), ..Default::default() }], ..a.clone() };
        assert_eq!(a.key(), b.key());
        let c = ImportProfile { rate: RateChoice::Assume { hz: 10.0 }, ..a.clone() };
        assert_ne!(a.key(), c.key());
    }

    #[test]
    fn column_order_does_not_change_the_key() {
        let col = |s: &str| ColumnChoice { source: s.into(), kind: Some(ColumnKind::Skip), ..Default::default() };
        let a = ImportProfile { columns: vec![col("a"), col("b")], ..Default::default() };
        let b = ImportProfile { columns: vec![col("b"), col("a")], ..Default::default() };
        assert_eq!(a.key(), b.key());
    }

    #[test]
    fn header_keys_ignore_order() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(header_key(&n(&["t", "a"])), header_key(&n(&["a", "t"])));
        assert_ne!(header_key(&n(&["t", "a"])), header_key(&n(&["t", "b"])));
        assert!((header_overlap(&n(&["t", "a", "b"]), &n(&["t", "a", "c"])) - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn saved_profiles_are_found_exactly_or_nearly() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let tmp = tempfile::tempdir().unwrap();
        let p = ImportProfile { rate: RateChoice::Samples, ..Default::default() };
        save(tmp.path(), &n(&["t", "a", "b", "c"]), &p).unwrap();
        assert_eq!(find(tmp.path(), &n(&["c", "b", "a", "t"])).exact, Some(p.clone()));
        let f = find(tmp.path(), &n(&["t", "a", "b", "d"]));
        assert_eq!(f.exact, None);
        assert_eq!(f.near.as_ref().map(|(s, sp)| (*s, sp.profile.clone())), Some((0.75, p.clone())));
        assert_eq!(find(tmp.path(), &n(&["x", "y"])), Found::default());
        remove(tmp.path(), &n(&["t", "a", "b", "c"]));
        assert!(list(tmp.path()).is_empty());
    }

    #[test]
    fn json_round_trip() {
        let p = ImportProfile {
            time: TimeChoice::Column { name: "t".into(), unit: Some("ms".into()) },
            rate: RateChoice::Resample { hz: 100.0 },
            columns: vec![ColumnChoice { source: "a".into(), name: Some("A".into()), ..Default::default() }],
            check_sets: vec!["Flight acceptance".into()],
        };
        // the sets a profile names don't change how a file opens: same key, still the default when nothing else is set
        assert_eq!(p.key(), ImportProfile { check_sets: Vec::new(), ..p.clone() }.key());
        assert!(ImportProfile { check_sets: vec!["x".into()], ..Default::default() }.is_default());
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<ImportProfile>(&json).unwrap(), p);
        assert_eq!(serde_json::from_str::<ImportProfile>("{}").unwrap(), ImportProfile::default());
    }
}
