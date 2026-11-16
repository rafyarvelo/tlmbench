//! The pure part of the shared-configuration reader: what a `.tlmbench/` folder holds that lives in the app's settings
//! (categories, widgets, icons), read through a [`Tree`] rather than paths. The app reads a folder on disk
//! (`config.rs`); the browser-only page reads the folder it was given by a directory handle (`crates/web` includes this
//! file as source), so both take a folder's objects by the same rules. Nothing here writes.

use serde::{Deserialize, Serialize};

pub const SCHEMA: u32 = 1;

/// A folder to read: files by their path inside it (`/`-separated, `widgets/dial/widget.toml`).
pub trait Tree {
    /// The text of the file at `rel`, if there is one.
    fn text(&self, rel: &str) -> Option<String>;
    /// The directories directly in `rel`, by name, in name order.
    fn dirs(&self, rel: &str) -> Vec<String>;
    /// The files directly in `rel` whose names end with `ext`, by name, in name order.
    fn files(&self, rel: &str, ext: &str) -> Vec<String>;
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") }
}

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

/// What a folder holds of those, for the app to take into its settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FromFolder {
    pub categories: Vec<Category>,
    pub widgets: Vec<Widget>,
    pub icons: Vec<Icon>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct CategoryFile {
    pub(crate) schema: u32,
    #[serde(default, rename = "category")]
    pub(crate) categories: Vec<Category>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct WidgetFile {
    pub(crate) schema: u32,
    pub(crate) name: String,
    /// `svg` or `code`.
    pub(crate) kind: String,
    pub(crate) entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) angle: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) by: Option<String>,
}

pub(crate) fn schema_ok(n: u32, what: &str) -> std::result::Result<(), String> {
    if n == SCHEMA { Ok(()) } else { Err(format!("{what} schema {n} isn't supported (this version reads {SCHEMA})")) }
}

/// The widget in folder `dir` of `t`: its `widget.toml`, entry and icon.
pub(crate) fn read_widget(t: &dyn Tree, dir: &str) -> std::result::Result<Widget, String> {
    let head = t.text(&join(dir, "widget.toml")).ok_or("widget.toml: not found")?;
    let f: WidgetFile = toml::from_str(&head).map_err(|e| e.to_string())?;
    schema_ok(f.schema, "widget")?;
    if f.entry.contains(['/', '\\']) || f.entry.starts_with('.') {
        return Err("the entry must be a file in the widget's folder".into());
    }
    let body = t.text(&join(dir, &f.entry)).ok_or_else(|| format!("{}: not found", f.entry))?;
    let icon = match &f.icon {
        Some(i) if !i.contains(['/', '\\']) => t.text(&join(dir, i)),
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
pub fn from_tree(t: &dyn Tree) -> FromFolder {
    let mut out = FromFolder::default();
    if let Some(f) = t.text("categories.toml").and_then(|text| toml::from_str::<CategoryFile>(&text).ok())
        && f.schema == SCHEMA
    {
        out.categories = f.categories;
    }
    for d in t.dirs("widgets") {
        if let Ok(mut w) = read_widget(t, &join("widgets", &d)) {
            w.id = d;
            out.widgets.push(w);
        }
    }
    for name in t.files("icons", ".svg") {
        if let Some(svg) = t.text(&join("icons", &name)) {
            out.icons.push(Icon { name: name.trim_end_matches(".svg").into(), svg });
        }
    }
    out
}
