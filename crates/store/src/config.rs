//! Shared configuration: the `.tlmbench/` folder next to the recordings (design: Shared configuration and version
//! control). Everything a team defines is personal by default (the app cache, the app's settings) and shared as
//! small files here: one file per object, fixed key order, no timestamps, so a change is a one-file diff and
//! re-sharing an unchanged object rewrites nothing.
//!
//! The module is file-level. It turns what the cache holds (and what the app's settings hold, passed in as [`Own`])
//! into [`Item`]s, reads a folder's items back, compares the two ([`compare`]), and writes either way: [`share`]
//! (to a folder, which only ever happens by a person's action), [`pull`] (a folder's objects into the cache) and
//! [`export`] (everything to any directory, for CI: `tlmbench config export`). Caches (pyramids, check results) are
//! never part of it. The CLI and the app both read the nearest `.tlmbench/` ([`nearest`]) or `--config` folders
//! (later ones win).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::{Error, Result};

pub const SCHEMA: u32 = 1;

/// The kinds of object a `.tlmbench/` holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Views,
    Events,
    Checks,
    Categories,
    Recipes,
    Profiles,
    Decoders,
    Widgets,
    Icons,
    Timemaps,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Views,
        Kind::Events,
        Kind::Checks,
        Kind::Categories,
        Kind::Recipes,
        Kind::Profiles,
        Kind::Decoders,
        Kind::Widgets,
        Kind::Icons,
        Kind::Timemaps,
    ];

    /// Where its objects live in the folder (a directory; categories are the one single file).
    pub fn path(self) -> &'static str {
        match self {
            Kind::Views => "views",
            Kind::Events => "events",
            Kind::Checks => "checks",
            Kind::Categories => "categories.toml",
            Kind::Recipes => "recipes",
            Kind::Profiles => "profiles",
            Kind::Decoders => "decoders",
            Kind::Widgets => "widgets",
            Kind::Icons => "icons",
            Kind::Timemaps => "timemaps",
        }
    }
}

/// One object: a few files (a widget has its definition and its code, everything else is one file), paths relative
/// to the `.tlmbench/` folder with `/` separators.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Item {
    pub kind: Kind,
    /// What it's called to a person (a recipe's name, a recording's file name).
    pub name: String,
    pub files: Vec<(String, Vec<u8>)>,
}

impl Item {
    /// The object's key: its first file's path (a widget's folder).
    pub fn key(&self) -> String {
        match self.kind {
            Kind::Widgets => {
                self.files.first().and_then(|f| f.0.rsplit_once('/')).map_or(String::new(), |p| p.0.into())
            }
            _ => self.files.first().map(|f| f.0.clone()).unwrap_or_default(),
        }
    }
}

/// A file name for something called `name`: lower case, letters and digits joined by dashes.
pub fn slug(name: &str, empty: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { empty.into() } else { s }
}

// ---------------------------------------------------------------------------------------------------------------
// What the app's settings own (they reach the core as JSON, and come out in the same shape)

/// An event category, as the folder's `categories.toml` holds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default = "yes")]
    pub show_on_charts: bool,
    /// The name of an icon in `.tlmbench/icons/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

fn yes() -> bool {
    true
}

/// A widget of your own: a declarative SVG or code.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Widget {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub svg: Option<String>,
    pub angle: Option<[f64; 2]>,
    pub code: Option<String>,
    pub by: Option<String>,
}

/// An icon: an SVG that uses `currentColor`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Icon {
    pub name: String,
    pub svg: String,
}

/// A layout, by its file name (`flight-review.tlmview.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub file: String,
    pub view: serde_json::Value,
}

/// A recording whose events are shared: its path relative to the folder and its identity (the cache key).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    pub name: String,
    pub identity: String,
    /// An open file's id, from which the app fills `identity` in.
    pub file: Option<u32>,
}

/// What the app's settings and open tabs hold, and the cache doesn't.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Own {
    pub categories: Vec<Category>,
    pub widgets: Vec<Widget>,
    pub icons: Vec<Icon>,
    pub views: Vec<View>,
    pub recordings: Vec<Recording>,
}

/// What a folder holds of those, for the app to take into its settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FromFolder {
    pub categories: Vec<Category>,
    pub widgets: Vec<Widget>,
    pub icons: Vec<Icon>,
}

// ---------------------------------------------------------------------------------------------------------------
// File formats

#[derive(Serialize, Deserialize)]
struct CategoryFile {
    schema: u32,
    #[serde(default, rename = "category")]
    categories: Vec<Category>,
}

#[derive(Serialize, Deserialize)]
struct WidgetFile {
    schema: u32,
    name: String,
    /// `svg` or `code`.
    kind: String,
    entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    angle: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    by: Option<String>,
}

/// An import profile with the header it belongs to, as `.tlmbench/profiles/<profile>.toml`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ProfileFile {
    schema: u32,
    header: Vec<String>,
    profile: tlmbench_decode::profile::ImportProfile,
}

#[derive(Serialize, Deserialize)]
struct EventFile {
    schema: u32,
    recording: String,
    identity: String,
    #[serde(default, rename = "event")]
    events: Vec<EventRow>,
}

/// One event in a file: times are whole nanoseconds (TOML integers are 64-bit), and who added it.
#[derive(Clone, Serialize, Deserialize)]
struct EventRow {
    id: String,
    name: String,
    start: i64,
    #[serde(default)]
    duration: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    category: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    channels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    by: Option<String>,
}

/// The alignment of one file against a tab's primary: the method and what it found, so it opens exactly as it was
/// left. `knots` are `[file time, shared time]` in nanoseconds (the map itself); the rest says how they were made
/// and lets the method run again.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimeMapFile {
    pub schema: u32,
    /// The tab's primary file and this one: paths relative to the folder.
    pub primary: String,
    pub file: String,
    /// Their identities (size and content hash), to find them again when renamed or moved.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub primary_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file_id: String,
    pub method: String,
    #[serde(default)]
    pub seconds: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub channel: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub knots: Vec<[i64; 2]>,
    /// Sync points as seconds from each file's first sample.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<[f64; 2]>,
}

impl TimeMapFile {
    /// The name its file has: both files' names.
    pub fn file_name(&self) -> String {
        let stem = |p: &str| {
            let n = p.rsplit(['/', '\\']).next().unwrap_or(p);
            slug(n.rsplit_once('.').map_or(n, |s| s.0), "file")
        };
        format!("{}--{}.toml", stem(&self.primary), stem(&self.file))
    }
}

pub fn timemap_to_toml(t: &TimeMapFile) -> String {
    toml::to_string(&TimeMapFile { schema: SCHEMA, ..t.clone() }).expect("time maps serialize")
}

pub fn timemap_from_toml(text: &str) -> std::result::Result<TimeMapFile, String> {
    let t: TimeMapFile = toml::from_str(text).map_err(|e| e.to_string())?;
    if t.schema != SCHEMA {
        return Err(format!("time map schema {} isn't supported (this version reads {SCHEMA})", t.schema));
    }
    Ok(t)
}

fn schema_ok(n: u32, what: &str) -> std::result::Result<(), String> {
    if n == SCHEMA { Ok(()) } else { Err(format!("{what} schema {n} isn't supported (this version reads {SCHEMA})")) }
}

fn toml_of<T: Serialize>(v: &T) -> Vec<u8> {
    toml::to_string(v).expect("configuration serializes").into_bytes()
}

fn json_of(v: &serde_json::Value) -> Vec<u8> {
    (serde_json::to_string_pretty(v).expect("a view serializes") + "\n").into_bytes()
}

fn categories_to_toml(c: &[Category]) -> Vec<u8> {
    toml_of(&CategoryFile { schema: SCHEMA, categories: c.to_vec() })
}

/// A profile file's name: the header's first names and the header's key.
fn profile_file_name(header: &[String]) -> String {
    let first: Vec<&str> = header.iter().take(3).map(String::as_str).collect();
    let key = tlmbench_decode::profile::header_key(header);
    format!("{}-{}.toml", slug(&first.join(" "), "profile"), &key[..key.len().min(6)])
}

fn event_rows(events: &[Event]) -> Vec<EventRow> {
    let mut v: Vec<EventRow> = events
        .iter()
        .filter(|e| e.source == "user")
        .map(|e| EventRow {
            id: e.id.clone(),
            name: e.name.clone(),
            start: e.start.parse().unwrap_or(0),
            duration: e.duration.parse().unwrap_or(0),
            category: e.category.clone(),
            channels: e.channels.clone(),
            by: e.by.clone(),
        })
        .collect();
    v.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    v
}

fn events_of(rows: &[EventRow]) -> Vec<Event> {
    rows.iter()
        .map(|r| Event {
            id: r.id.clone(),
            name: r.name.clone(),
            start: r.start.to_string(),
            duration: r.duration.to_string(),
            category: r.category.clone(),
            source: "user".into(),
            channels: r.channels.clone(),
            by: r.by.clone(),
        })
        .collect()
}

fn event_file(rec: &Recording, events: &[Event]) -> Vec<u8> {
    toml_of(&EventFile {
        schema: SCHEMA,
        recording: rec.name.clone(),
        identity: rec.identity.clone(),
        events: event_rows(events),
    })
}

fn widget_files(w: &Widget) -> Vec<(String, Vec<u8>)> {
    let dir = format!("widgets/{}", slug(&w.name, "widget"));
    let (kind, entry, body) = match (&w.code, &w.svg) {
        (Some(code), _) => ("code", "widget.js", code.clone()),
        (None, svg) => ("svg", "widget.svg", svg.clone().unwrap_or_default()),
    };
    let head = WidgetFile {
        schema: SCHEMA,
        name: w.name.clone(),
        kind: kind.into(),
        entry: entry.into(),
        icon: w.icon.as_ref().map(|_| "icon.svg".to_owned()),
        angle: if kind == "svg" { w.angle } else { None },
        by: w.by.clone(),
    };
    let mut files = vec![(format!("{dir}/widget.toml"), toml_of(&head)), (format!("{dir}/{entry}"), body.into_bytes())];
    if let Some(icon) = &w.icon {
        files.push((format!("{dir}/icon.svg"), icon.clone().into_bytes()));
    }
    files
}

// ---------------------------------------------------------------------------------------------------------------
// Reading a folder, and the cache

/// Files directly in `dir` with this extension, in name order.
fn files_in(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(ext)))
        .collect();
    v.sort();
    v
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Makes a file's text canonical: its name to a person and its bytes.
type Canon = dyn Fn(&str) -> std::result::Result<(String, Vec<u8>), String>;

/// One TOML kind: every file in `dir/<sub>` made canonical by `canon` (name, bytes), or the problems.
fn read_dir_items(dir: &Path, kind: Kind, ext: &str, canon: &Canon, problems: &mut Vec<Problem>) -> Vec<Item> {
    let mut out = Vec::new();
    for p in files_in(&dir.join(kind.path()), ext) {
        let rel = format!("{}/{}", kind.path(), name_of(&p));
        match fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| canon(&t)) {
            Ok((name, body)) => out.push(Item { kind, name, files: vec![(rel, body)] }),
            Err(why) => problems.push(Problem { path: rel, why }),
        }
    }
    out
}

/// A file in a folder that can't be read as what it should be.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Problem {
    pub path: String,
    pub why: String,
}

/// Every object in a `.tlmbench/` folder `dir` (canonical bytes, so equal meaning compares equal), and the files
/// that don't read. A folder with no `.tlmbench/` has nothing.
pub fn read_folder(dir: &Path) -> (Vec<Item>, Vec<Problem>) {
    let mut items = Vec::new();
    let mut problems = Vec::new();
    items.extend(read_dir_items(
        dir,
        Kind::Views,
        ".tlmview.json",
        &|t| {
            let v: serde_json::Value = serde_json::from_str(t).map_err(|e| e.to_string())?;
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("view").to_owned();
            Ok((name, json_of(&v)))
        },
        &mut problems,
    ));
    items.extend(read_dir_items(
        dir,
        Kind::Events,
        ".toml",
        &|t| {
            let f: EventFile = toml::from_str(t).map_err(|e| e.to_string())?;
            schema_ok(f.schema, "event file")?;
            Ok((f.recording.clone(), toml_of(&EventFile { events: sorted_rows(f.events), ..f })))
        },
        &mut problems,
    ));
    items.extend(read_dir_items(
        dir,
        Kind::Checks,
        ".toml",
        &|t| crate::checks::from_toml(t).map(|s| (s.name.clone(), crate::checks::to_toml(&s).into_bytes())),
        &mut problems,
    ));
    items.extend(read_dir_items(
        dir,
        Kind::Recipes,
        ".toml",
        &|t| {
            let v = tlmbench_expr::recipe::from_toml(t)?;
            let name = v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ");
            Ok((name, tlmbench_expr::recipe::to_toml(&v).into_bytes()))
        },
        &mut problems,
    ));
    items.extend(read_dir_items(
        dir,
        Kind::Profiles,
        ".toml",
        &|t| {
            let f: ProfileFile = toml::from_str(t).map_err(|e| e.to_string())?;
            schema_ok(f.schema, "profile")?;
            f.profile.validate()?;
            Ok((f.header.iter().take(3).cloned().collect::<Vec<_>>().join(", "), toml_of(&f)))
        },
        &mut problems,
    ));
    items.extend(read_dir_items(
        dir,
        Kind::Decoders,
        ".toml",
        &|t| {
            let d = tlmbench_decode::descriptor::Descriptor::parse(t).map_err(|e| e.to_string())?;
            Ok((d.name.clone().unwrap_or_default(), d.to_toml().into_bytes()))
        },
        &mut problems,
    ));
    items.extend(read_dir_items(
        dir,
        Kind::Timemaps,
        ".toml",
        &|t| {
            let f = timemap_from_toml(t)?;
            Ok((format!("{} · {}", f.primary, f.file), timemap_to_toml(&f).into_bytes()))
        },
        &mut problems,
    ));
    // categories: one file
    let cat = dir.join(Kind::Categories.path());
    if let Ok(t) = fs::read_to_string(&cat) {
        match toml::from_str::<CategoryFile>(&t)
            .map_err(|e| e.to_string())
            .and_then(|f| schema_ok(f.schema, "categories").map(|()| f))
        {
            Ok(f) => items.push(Item {
                kind: Kind::Categories,
                name: "Event categories".into(),
                files: vec![("categories.toml".into(), categories_to_toml(&f.categories))],
            }),
            Err(why) => problems.push(Problem { path: "categories.toml".into(), why }),
        }
    }
    // widgets: a folder each
    if let Ok(rd) = fs::read_dir(dir.join("widgets")) {
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            let rel = format!("widgets/{}", name_of(&d));
            match read_widget(&d) {
                Ok(w) => {
                    let files = widget_files(&w)
                        .into_iter()
                        .map(|(p, b)| (p.replacen(&format!("widgets/{}", slug(&w.name, "widget")), &rel, 1), b))
                        .collect();
                    items.push(Item { kind: Kind::Widgets, name: w.name, files });
                }
                Err(why) => problems.push(Problem { path: format!("{rel}/widget.toml"), why }),
            }
        }
    }
    // icons: SVG files
    for p in files_in(&dir.join("icons"), ".svg") {
        if let Ok(b) = fs::read(&p) {
            let stem = name_of(&p);
            items.push(Item {
                kind: Kind::Icons,
                name: stem.trim_end_matches(".svg").into(),
                files: vec![(format!("icons/{stem}"), b)],
            });
        }
    }
    (items, problems)
}

fn sorted_rows(mut v: Vec<EventRow>) -> Vec<EventRow> {
    v.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.id.cmp(&b.id)));
    v
}

/// A widget folder (`widget.toml` and its entry) read as a [`Widget`].
fn read_widget(dir: &Path) -> std::result::Result<Widget, String> {
    let head = fs::read_to_string(dir.join("widget.toml")).map_err(|e| format!("widget.toml: {e}"))?;
    let f: WidgetFile = toml::from_str(&head).map_err(|e| e.to_string())?;
    schema_ok(f.schema, "widget")?;
    if f.entry.contains(['/', '\\']) || f.entry.starts_with('.') {
        return Err("the entry must be a file in the widget's folder".into());
    }
    let body = fs::read_to_string(dir.join(&f.entry)).map_err(|e| format!("{}: {e}", f.entry))?;
    let icon = match &f.icon {
        Some(i) if !i.contains(['/', '\\']) => fs::read_to_string(dir.join(i)).ok(),
        _ => None,
    };
    let mut w = Widget { id: String::new(), name: f.name, icon, by: f.by, ..Default::default() };
    match f.kind.as_str() {
        "code" => w.code = Some(body),
        "svg" => {
            w.svg = Some(body);
            w.angle = f.angle;
        }
        other => return Err(format!("widget kind {other:?} isn't svg or code")),
    }
    Ok(w)
}

/// What a folder holds that lives in the app's settings: categories, widgets and icons.
pub fn from_folder(dir: &Path) -> FromFolder {
    let mut out = FromFolder::default();
    if let Ok(f) = fs::read_to_string(dir.join("categories.toml"))
        .map_err(|e| e.to_string())
        .and_then(|t| toml::from_str::<CategoryFile>(&t).map_err(|e| e.to_string()))
        && f.schema == SCHEMA
    {
        out.categories = f.categories;
    }
    if let Ok(rd) = fs::read_dir(dir.join("widgets")) {
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            if let Ok(mut w) = read_widget(&d) {
                w.id = name_of(&d);
                out.widgets.push(w);
            }
        }
    }
    for p in files_in(&dir.join("icons"), ".svg") {
        if let Ok(svg) = fs::read_to_string(&p) {
            out.icons.push(Icon { name: name_of(&p).trim_end_matches(".svg").into(), svg });
        }
    }
    out
}

/// Every object the cache (under `root`) and `own` hold, in the form a folder holds them.
pub fn cache_items(root: &Path, own: &Own) -> Vec<Item> {
    // the cache keeps these in the folder's own formats
    let (mut items, _) = read_folder(root);
    items.retain(|i| matches!(i.kind, Kind::Recipes | Kind::Checks | Kind::Decoders | Kind::Timemaps));
    // profiles are JSON in the cache; shared they are TOML
    for sp in tlmbench_decode::profile::list(root) {
        let f = ProfileFile { schema: SCHEMA, header: sp.header.clone(), profile: sp.profile.clone() };
        items.push(Item {
            kind: Kind::Profiles,
            name: sp.header.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
            files: vec![(format!("profiles/{}", profile_file_name(&sp.header)), toml_of(&f))],
        });
    }
    for rec in &own.recordings {
        let events: Vec<Event> = crate::events::list(root, &rec.identity)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.source == "user")
            .collect();
        if !events.is_empty() {
            items.push(Item {
                kind: Kind::Events,
                name: rec.name.clone(),
                files: vec![(
                    format!("events/{}.toml", slug(&rec.name.replace('/', " "), "recording")),
                    event_file(rec, &events),
                )],
            });
        }
    }
    if !own.categories.is_empty() {
        items.push(Item {
            kind: Kind::Categories,
            name: "Event categories".into(),
            files: vec![("categories.toml".into(), categories_to_toml(&own.categories))],
        });
    }
    for w in own.widgets.iter().filter(|w| w.svg.is_some() || w.code.is_some()) {
        items.push(Item { kind: Kind::Widgets, name: w.name.clone(), files: widget_files(w) });
    }
    for i in &own.icons {
        items.push(Item {
            kind: Kind::Icons,
            name: i.name.clone(),
            files: vec![(format!("icons/{}.svg", slug(&i.name, "icon")), i.svg.clone().into_bytes())],
        });
    }
    for v in &own.views {
        let name = v.view.get("name").and_then(|n| n.as_str()).unwrap_or("view").to_owned();
        items.push(Item { kind: Kind::Views, name, files: vec![(format!("views/{}", v.file), json_of(&v.view))] });
    }
    items.sort_by_key(|i| (i.kind, i.key()));
    items
}

// ---------------------------------------------------------------------------------------------------------------
// Comparing and writing

/// How an object in the cache stands against the folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Only you have it.
    New,
    /// Both have it, and they differ.
    Changed,
    /// Both have it, equal.
    Same,
    /// Only the folder has it.
    Folder,
}

/// One row of the Share dialog.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    pub kind: Kind,
    pub key: String,
    pub name: String,
    pub state: State,
    /// For events: how many of yours the folder lacks and how many it has that you lack.
    pub detail: Option<String>,
}

/// What a shared view has beyond its layout: who shared it, and the files and time range it opens with. A layout of
/// yours is the same as the folder's when the rest matches.
const VIEW_EXTRAS: [&str; 3] = ["by", "files", "view"];

fn comparable(i: &Item) -> Vec<(String, Vec<u8>)> {
    if i.kind != Kind::Views {
        return i.files.clone();
    }
    i.files
        .iter()
        .map(|(p, b)| {
            let mut v: serde_json::Value = serde_json::from_slice(b).unwrap_or_default();
            if let Some(o) = v.as_object_mut() {
                VIEW_EXTRAS.iter().for_each(|k| drop(o.remove(*k)));
            }
            (p.clone(), json_of(&v))
        })
        .collect()
}

/// The cache's objects against the folder's.
pub fn compare(cache: &[Item], folder: &[Item]) -> Vec<Row> {
    let theirs: BTreeMap<(Kind, String), &Item> = folder.iter().map(|i| ((i.kind, i.key()), i)).collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut rows = Vec::new();
    for i in cache {
        let key = (i.kind, i.key());
        seen.insert(key.clone());
        let (state, detail) = match theirs.get(&key) {
            None => (State::New, None),
            Some(t) if comparable(t) == comparable(i) => (State::Same, None),
            Some(t) => (State::Changed, event_detail(i, t)),
        };
        // events merge by id: the file is "same" when the folder already has all of yours
        let state = if i.kind == Kind::Events && state == State::Changed && detail.as_deref() == Some("") {
            State::Same
        } else {
            state
        };
        rows.push(Row {
            kind: i.kind,
            key: key.1,
            name: i.name.clone(),
            state,
            detail: detail.filter(|d| !d.is_empty()),
        });
    }
    for f in folder {
        if !seen.contains(&(f.kind, f.key())) {
            rows.push(Row { kind: f.kind, key: f.key(), name: f.name.clone(), state: State::Folder, detail: None });
        }
    }
    rows.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    rows
}

/// "2 of yours not shared, 1 of theirs not yours" for two event files ("" when yours are all there).
fn event_detail(mine: &Item, theirs: &Item) -> Option<String> {
    if mine.kind != Kind::Events {
        return None;
    }
    let ids = |i: &Item| -> Vec<(String, Vec<u8>)> {
        let f: EventFile = toml::from_str(&String::from_utf8_lossy(&i.files[0].1)).unwrap_or(EventFile {
            schema: 1,
            recording: String::new(),
            identity: String::new(),
            events: vec![],
        });
        f.events.iter().map(|e| (e.id.clone(), toml_of(&e_pack(e)))).collect()
    };
    let (m, t) = (ids(mine), ids(theirs));
    let mine_new = m.iter().filter(|e| !t.contains(e)).count();
    let theirs_new = t.iter().filter(|e| !m.iter().any(|x| x.0 == e.0)).count();
    let mut parts = Vec::new();
    if mine_new > 0 {
        parts.push(format!("{mine_new} of yours not shared"));
    }
    if theirs_new > 0 {
        parts.push(format!("{theirs_new} of theirs not yours"));
    }
    Some(parts.join(", "))
}

fn e_pack(e: &EventRow) -> EventFile {
    EventFile { schema: 0, recording: String::new(), identity: String::new(), events: vec![e.clone()] }
}

/// Writes `bytes` to `path` when it differs (nothing is rewritten when it doesn't), through a temp file.
fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool> {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(Error::io(dir))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(Error::io(&tmp))?;
    fs::rename(&tmp, path).map_err(Error::io(path))?;
    Ok(true)
}

/// The union of two event files' events by id: the cache's version of an event wins over the folder's.
fn merge_events(mine: &[u8], theirs: &[u8]) -> Vec<u8> {
    let parse = |b: &[u8]| toml::from_str::<EventFile>(&String::from_utf8_lossy(b)).ok();
    let (Some(mut m), Some(t)) = (parse(mine), parse(theirs)) else { return mine.to_vec() };
    for e in t.events {
        if !m.events.iter().any(|x| x.id == e.id) {
            m.events.push(e);
        }
    }
    m.events = sorted_rows(m.events);
    toml_of(&m)
}

/// A layout written over a shared view keeps the files, time range and author the view had, unless the new one has its own.
fn keep_view_extras(new: &[u8], old: &[u8]) -> Vec<u8> {
    let (Ok(mut n), Ok(o)) =
        (serde_json::from_slice::<serde_json::Value>(new), serde_json::from_slice::<serde_json::Value>(old))
    else {
        return new.to_vec();
    };
    if let (Some(no), Some(oo)) = (n.as_object_mut(), o.as_object()) {
        for k in VIEW_EXTRAS {
            if let (false, Some(v)) = (no.contains_key(k), oo.get(k)) {
                no.insert(k.into(), v.clone());
            }
        }
    }
    json_of(&n)
}

/// Writes `items` into the folder `dir` (a `.tlmbench/`), creating it when needed; events merge by id with the ones
/// already there. Returns the paths that changed. Files that already hold the same bytes are left alone.
pub fn share(dir: &Path, items: &[Item]) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for i in items {
        let existing = i.kind == Kind::Widgets;
        if existing {
            // a widget's folder is replaced as a whole: files that are no longer part of it go
            let folder = i.key();
            if let Ok(rd) = fs::read_dir(dir.join(&folder)) {
                for e in rd.flatten() {
                    let rel = format!("{folder}/{}", name_of(&e.path()));
                    if e.path().is_file() && !i.files.iter().any(|f| f.0 == rel) {
                        fs::remove_file(e.path()).map_err(Error::io(&e.path()))?;
                    }
                }
            }
        }
        for (rel, bytes) in &i.files {
            let path = dir.join(rel);
            let body = match (i.kind, fs::read(&path)) {
                (Kind::Events, Ok(old)) => merge_events(bytes, &old),
                (Kind::Views, Ok(old)) => keep_view_extras(bytes, &old),
                _ => bytes.clone(),
            };
            if write_if_changed(&path, &body)? {
                written.push(rel.clone());
            }
        }
    }
    Ok(written)
}

/// Takes the folder's `items` of the kinds the core keeps (recipes, check sets, profiles, decoders, time maps,
/// events) into the cache under `root`. Events merge by id with yours and are stored under the identity in `own`
/// when the recording is open (matched by identity, else by name). Returns the keys taken. Categories, widgets and
/// icons belong to the app's settings: see [`from_folder`].
pub fn pull(root: &Path, items: &[Item], own: &Own) -> Result<Vec<String>> {
    let mut taken = Vec::new();
    for i in items {
        match i.kind {
            Kind::Recipes | Kind::Checks | Kind::Decoders | Kind::Timemaps => {
                let sub = i.kind.path();
                let name = i.files[0].0.rsplit('/').next().unwrap_or("");
                write_if_changed(&root.join(sub).join(name), &i.files[0].1)?;
            }
            Kind::Profiles => {
                if let Ok(f) = toml::from_str::<ProfileFile>(&String::from_utf8_lossy(&i.files[0].1)) {
                    tlmbench_decode::profile::save(root, &f.header, &f.profile)?;
                }
            }
            Kind::Events => {
                let Ok(f) = toml::from_str::<EventFile>(&String::from_utf8_lossy(&i.files[0].1)) else { continue };
                let id = own
                    .recordings
                    .iter()
                    .find(|r| r.identity == f.identity)
                    .or_else(|| own.recordings.iter().find(|r| r.name == f.recording))
                    .map(|r| r.identity.clone())
                    .unwrap_or(f.identity);
                for e in events_of(&f.events) {
                    crate::events::save(root, &id, e)?;
                }
            }
            _ => continue,
        }
        taken.push(i.key());
    }
    Ok(taken)
}

/// Writes every object the cache and `own` hold to the directory `out` (which becomes a `.tlmbench/`): the CLI's
/// `config export`, so CI uses exactly what you tested with. Returns the paths written.
pub fn export(root: &Path, own: &Own, out: &Path) -> Result<Vec<String>> {
    share(out, &cache_items(root, own))
}

// ---------------------------------------------------------------------------------------------------------------
// The app's own settings, mirrored into the cache so the CLI can export them too

const MIRROR: &str = "own.json";

/// Keeps `own` (the app's categories, widgets, icons, layouts and open recordings) in the cache root as `own.json`,
/// where `tlmbench config export` finds it. Nothing is written when it is unchanged.
pub fn save_mirror(root: &Path, own: &Own) -> Result<()> {
    let text = serde_json::to_string_pretty(own).expect("settings serialize");
    write_if_changed(&root.join(MIRROR), text.as_bytes()).map(|_| ())
}

/// What [`save_mirror`] kept (nothing when the app hasn't run).
pub fn load_mirror(root: &Path) -> Own {
    fs::read_to_string(root.join(MIRROR)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------------
// The folder the app reads alongside its cache

static OVERLAY: std::sync::RwLock<Vec<PathBuf>> = std::sync::RwLock::new(Vec::new());

/// The `.tlmbench/` folders the app reads shared recipes, check sets, profiles and events from, besides its cache
/// (the nearest above the open folder, set when the folder changes). Yours win a name; the folder's fill in the rest.
pub fn set_overlay(dirs: Vec<PathBuf>) {
    *OVERLAY.write().unwrap() = dirs;
}

pub fn overlay() -> Vec<PathBuf> {
    OVERLAY.read().unwrap().clone()
}

/// A `.tlmbench/` folder's recipes (later folders win a name).
pub fn shared_recipes(dirs: &[PathBuf]) -> Vec<tlmbench_expr::Recipe> {
    let mut out: Vec<tlmbench_expr::Recipe> = Vec::new();
    for d in dirs {
        for p in files_in(&d.join("recipes"), ".toml") {
            let Ok(v) =
                fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| tlmbench_expr::recipe::from_toml(&t))
            else {
                continue;
            };
            for r in v {
                match out.iter_mut().find(|o| o.name == r.name) {
                    Some(o) => *o = r,
                    None => out.push(r),
                }
            }
        }
    }
    out
}

/// A `.tlmbench/` folder's check sets (later folders win a name).
pub fn shared_sets(dirs: &[PathBuf]) -> Vec<crate::checks::CheckSet> {
    let mut out: Vec<crate::checks::CheckSet> = Vec::new();
    for d in dirs {
        for p in files_in(&d.join("checks"), ".toml") {
            let Ok(s) = fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| crate::checks::from_toml(&t))
            else {
                continue;
            };
            match out.iter_mut().find(|o| o.name == s.name) {
                Some(o) => *o = s,
                None => out.push(s),
            }
        }
    }
    out
}

/// The import profile a `.tlmbench/` folder keeps for a CSV header (later folders win).
pub fn shared_profile(dirs: &[PathBuf], header: &[String]) -> Option<tlmbench_decode::profile::ImportProfile> {
    let key = tlmbench_decode::profile::header_key(header);
    dirs.iter().rev().find_map(|d| {
        files_in(&d.join("profiles"), ".toml").into_iter().find_map(|p| {
            let f: ProfileFile = toml::from_str(&fs::read_to_string(p).ok()?).ok()?;
            (f.schema == SCHEMA && tlmbench_decode::profile::header_key(&f.header) == key).then_some(f.profile)
        })
    })
}

/// An import profile file's text (`--profile x.toml`).
pub fn profile_from_toml(text: &str) -> std::result::Result<tlmbench_decode::profile::ImportProfile, String> {
    let f: ProfileFile = toml::from_str(text).map_err(|e| e.to_string())?;
    schema_ok(f.schema, "profile")?;
    f.profile.validate()?;
    Ok(f.profile)
}

/// The events a `.tlmbench/` folder shares for a recording (matched by identity, else by name), as read-only events of
/// source `shared`.
pub fn shared_events(dirs: &[PathBuf], identity: &str, name: &str) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::new();
    for d in dirs {
        for p in files_in(&d.join("events"), ".toml") {
            let Ok(f) = fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|t| toml::from_str::<EventFile>(&t).map_err(|e| e.to_string()))
            else {
                continue;
            };
            if f.schema != SCHEMA || !(f.identity == identity || (!name.is_empty() && f.recording == name)) {
                continue;
            }
            for mut e in events_of(&f.events) {
                e.source = "shared".into();
                if !out.iter().any(|o| o.id == e.id) {
                    out.push(e);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Where the configuration is

/// The nearest `.tlmbench/` at or above `path` (a file or a folder).
pub fn nearest(path: &Path) -> Option<PathBuf> {
    let abs = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    abs.ancestors().map(|d| d.join(".tlmbench")).find(|d| d.is_dir())
}

/// The folders configuration is read from for something at `path`: the `--config` ones (given order, later win) or,
/// with none, the nearest `.tlmbench/`.
pub fn dirs_for(path: &Path, config: &[PathBuf]) -> Vec<PathBuf> {
    if config.is_empty() { nearest(path).into_iter().collect() } else { config.to_vec() }
}

/// Whether `dir` (a `.tlmbench/` or the folder that would hold one) looks writable. The permission bits only say so
/// much, so a write that still fails is reported where it happens (Share then offers Export).
pub fn writable(dir: &Path) -> bool {
    let probe = if dir.exists() { dir.to_owned() } else { dir.parent().unwrap_or(dir).to_owned() };
    let Ok(m) = fs::metadata(&probe) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode() & 0o200 != 0
    }
    #[cfg(not(unix))]
    {
        !m.permissions().readonly()
    }
}

/// Lints a folder: every file that doesn't read as what it should be, with why.
pub fn validate(dir: &Path) -> Vec<Problem> {
    let (items, mut problems) = read_folder(dir);
    // a recipe that doesn't parse as an expression, a check without a usable expression
    for i in &items {
        match i.kind {
            Kind::Recipes => {
                if let Ok(v) = tlmbench_expr::recipe::from_toml(&String::from_utf8_lossy(&i.files[0].1)) {
                    for r in v {
                        if let Err(e) = tlmbench_expr::parse(&r.expr) {
                            problems.push(Problem {
                                path: i.files[0].0.clone(),
                                why: format!("recipe {:?}: {} (at {})", r.name, e.msg, e.at),
                            });
                        }
                    }
                }
            }
            Kind::Checks => {
                if let Ok(s) = crate::checks::from_toml(&String::from_utf8_lossy(&i.files[0].1)) {
                    for c in &s.checks {
                        if let Err(e) = tlmbench_expr::parse(&c.expect) {
                            problems.push(Problem {
                                path: i.files[0].0.clone(),
                                why: format!("check {:?}: {} (at {})", c.name, e.msg, e.at),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Flight review!", "x"), "flight-review");
        assert_eq!(slug("", "x"), "x");
    }

    #[test]
    fn time_map_file_round_trips_and_names_itself() {
        let t = TimeMapFile {
            schema: 1,
            primary: "runs/a.csv".into(),
            file: "runs/B 2.csv".into(),
            method: "sync".into(),
            knots: vec![[0, 5], [10, 20]],
            pairs: vec![[1.0, 2.5]],
            ..Default::default()
        };
        assert_eq!(t.file_name(), "a--b-2.toml");
        assert_eq!(timemap_from_toml(&timemap_to_toml(&t)).unwrap(), t);
    }
}
