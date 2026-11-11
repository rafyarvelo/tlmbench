//! `tlmbench export html`: a folder of static files (the viewer, the data for a layout, the checks summary) that opens
//! from any static host. The app's Export writes the same folder through `tlmbench_export::html`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tlmbench_export::html::{self, Mode};
use tlmbench_store::checks::{self, Sets};
use tlmbench_store::{Opened, config, events};

use crate::{Common, Fail, Span, open_parts, recipe_library, span_of, store};

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Recordings, the first filling slot 1 of the layout, the next slot 2, and so on
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
    /// Folder to write (created; an existing export in it is replaced)
    #[arg(long)]
    pub out: PathBuf,
    /// A shared view (`.tlmbench/views`) by name or file name, or a view file: its panes and time range open first.
    /// Without one the viewer starts as the app does on opening the file
    #[arg(long)]
    pub layout: Option<String>,
    /// Include every channel, not only the ones the layout uses
    #[arg(long)]
    pub all_channels: bool,
    /// Leave these channels out even if the layout uses them (repeat, or separate with commas)
    #[arg(long, value_delimiter = ',')]
    pub redact: Vec<String>,
    /// Run a check set on each file and include its summary, with the violations marked on the charts
    #[arg(long = "checks")]
    pub checks: Vec<String>,
    #[command(flatten)]
    pub span: Span,
    /// The size budget of the folder (`250MB`, `1GB`, or bytes)
    #[arg(long, default_value = "250MB")]
    pub budget: String,
    /// full: raw samples for every channel; light: raw samples only for channels shown as tables, values or XY. By
    /// default Full when it fits the budget, else Light
    #[arg(long, value_enum)]
    pub mode: Option<ModeArg>,
    /// A viewer folder to copy instead of the one built into this program (also $TLMBENCH_VIEWER)
    #[arg(long)]
    pub viewer: Option<PathBuf>,
    /// Write the data only, without the viewer
    #[arg(long)]
    pub no_viewer: bool,
    /// The result as one JSON object
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub common: Common,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ModeArg {
    Full,
    Light,
}

fn bytes_arg(s: &str) -> Result<u64, Fail> {
    let t = s.trim().to_ascii_lowercase();
    let (num, mult) = [("gb", 1u64 << 30), ("mb", 1 << 20), ("kb", 1 << 10), ("b", 1)]
        .iter()
        .find_map(|(u, m)| t.strip_suffix(u).map(|n| (n, *m)))
        .unwrap_or((&t, 1));
    num.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| *n >= 0.0)
        .map(|n| (n * mult as f64) as u64)
        .ok_or_else(|| Fail(3, format!("{s:?} is not a size (250MB, 1GB or bytes)")))
}

/// A view by name or file name in the `.tlmbench/views` of the config folders, or a path to a view file.
fn find_layout(name: &str, first: &Path, common: &Common) -> Result<Value, Fail> {
    let as_path = Path::new(name);
    let text = if as_path.is_file() {
        std::fs::read_to_string(as_path).map_err(|e| Fail(3, format!("{name}: {e}")))?
    } else {
        let dirs = config::dirs_for(first, &common.config);
        let mut found = None;
        let mut have = Vec::new();
        'dirs: for d in dirs.iter().rev() {
            let Ok(rd) = std::fs::read_dir(d.join("views")) else { continue };
            for e in rd.flatten() {
                let Ok(t) = std::fs::read_to_string(e.path()) else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&t) else { continue };
                let n = v["name"].as_str().unwrap_or_default().to_owned();
                let stem = e.file_name().to_string_lossy().split('.').next().unwrap_or_default().to_owned();
                if n.eq_ignore_ascii_case(name) || stem.eq_ignore_ascii_case(name) {
                    found = Some(t);
                    break 'dirs;
                }
                have.push(n);
            }
        }
        found.ok_or_else(|| Fail(3, format!("no view called {name:?} (found: {})", have.join(", "))))?
    };
    serde_json::from_str(&text).map_err(|e| Fail(3, format!("the view {name:?} doesn't read: {e}")))
}

/// A pane channel `name` or `name·2` (slot 2): the slot (1-based) and the channel's own name.
pub fn split_slot(name: &str) -> (usize, &str) {
    if let Some((base, n)) = name.rsplit_once('·')
        && let Ok(slot) = n.parse::<usize>()
        && slot >= 2
    {
        return (slot, base);
    }
    (1, name)
}

pub fn run(a: Args) -> Result<(), Fail> {
    let budget = bytes_arg(&a.budget)?;
    let store = store(&a.common);
    // one recording per file (a packet file's types are separate recordings: pass --type)
    let mut opened: Vec<Opened> = Vec::new();
    for f in &a.files {
        let mut parts = open_parts(&store, f, &a.common)?;
        if parts.len() != 1 {
            return Err(Fail(3, format!("{} holds {} packet types: pick one with --type", f.display(), parts.len())));
        }
        opened.push(parts.remove(0));
    }
    let layout = a.layout.as_deref().map(|l| find_layout(l, &a.files[0], &a.common)).transpose()?;
    // the channels each file exports, and the ones that keep raw samples in Light
    let panes: Vec<(Vec<String>, String)> = layout
        .as_ref()
        .and_then(|l| l["panes"].as_array())
        .map(|ps| {
            ps.iter()
                .map(|p| {
                    let chans = p["channels"]
                        .as_array()
                        .map(|c| c.iter().filter_map(|x| x.as_str().map(String::from)).collect());
                    (chans.unwrap_or_default(), p["view"].as_str().unwrap_or("chart").to_owned())
                })
                .collect()
        })
        .unwrap_or_default();
    let mut files = Vec::new();
    let mut span_notes = Vec::new();
    for (k, o) in opened.iter().enumerate() {
        let rec = o.recording();
        let info = rec.info();
        let used = |slot: usize, name: &str, raw_only: bool| {
            panes.iter().any(|(chans, view)| {
                (!raw_only || matches!(view.as_str(), "table" | "both" | "values" | "xy"))
                    && chans.iter().any(|c| split_slot(c) == (slot, name))
            })
        };
        let channels: Vec<usize> = (0..info.channels.len())
            .filter(|&i| {
                let n = info.channels[i].name.as_str();
                !a.redact.iter().any(|r| r == n)
                    && (a.all_channels || layout.is_none() || panes.is_empty() || used(k + 1, n, false))
            })
            .collect();
        let raw: Vec<usize> = (0..info.channels.len()).filter(|&i| used(k + 1, &info.channels[i].name, true)).collect();
        span_notes.push(span_of(o, &a.span).ok());
        files.push(html::File {
            name: info.path.file_name().map_or_else(|| "recording".into(), |n| n.to_string_lossy().into_owned()),
            rec,
            channels,
            raw,
        });
    }
    // events: shared, personal, and what the check sets mark
    let dirs = config::dirs_for(&a.files[0], &a.common.config);
    let mut sets: Vec<tlmbench_store::checks::CheckSet> = Vec::new();
    if !a.checks.is_empty() {
        let mut all: Vec<tlmbench_store::checks::CheckSet> = Vec::new();
        for d in dirs.iter().rev() {
            if d.join("checks").is_dir() {
                all.extend(Sets::in_dir(&d.join("checks")).list());
            }
        }
        all.extend(Sets::new(&tlmbench_decode::cache::cache_root(a.common.cache.as_deref())).list());
        for n in &a.checks {
            let s = all.iter().find(|s| &s.name == n).cloned();
            sets.push(s.ok_or_else(|| {
                Fail(
                    3,
                    format!(
                        "no check set called {n:?} (found: {})",
                        all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
                    ),
                )
            })?);
        }
    }
    let mut events_out = Vec::new();
    let mut checks_out = Vec::new();
    for (k, o) in opened.iter().enumerate() {
        let info = o.recording().info();
        let name = files[k].name.clone();
        let mut evs = config::shared_events(&dirs, o.fingerprint(), &name);
        for e in events::list(store.root(), o.fingerprint()).unwrap_or_default() {
            if !evs.iter().any(|x| x.id == e.id) {
                evs.push(e);
            }
        }
        if !sets.is_empty() {
            let (recipes, _) = recipe_library(&info.path, &a.common, true, true)?;
            for set in &sets {
                let res = store.run_checks(o, set, &recipes)?;
                let marked = checks::events_of(set, &res);
                checks_out.push(json!({ "file": k, "set": set, "result": res, "events": marked, "t_first": info.t_first.unwrap_or(0).to_string() }));
            }
        }
        events_out.push(json!({ "file": k, "events": evs }));
    }
    let (from, until) = (
        a.span.from.is_some().then(|| span_notes[0].map(|s| s.0)).flatten(),
        a.span.until.is_some().then(|| span_notes[0].map(|s| s.1)).flatten(),
    );
    let opts = html::Options {
        mode: match a.mode {
            None => Mode::Auto,
            Some(ModeArg::Full) => Mode::Full,
            Some(ModeArg::Light) => Mode::Light,
        },
        budget,
        from,
        until,
    };
    let extra = json!({ "layout": layout, "events": events_out, "checks": checks_out, "tool": { "name": "tlmbench", "version": env!("CARGO_PKG_VERSION") } });
    let report = html::export(&a.out, &files, extra, &opts).map_err(|e| Fail(2, e.to_string()))?;
    if !a.no_viewer {
        match viewer_dir(a.viewer.as_deref()) {
            Some(src) => copy_dir(&src, &a.out).map_err(|e| Fail(2, format!("{}: {e}", src.display())))?,
            None => {
                let built_in = html::embedded_viewer();
                if built_in.is_empty() {
                    return Err(Fail(2, "this build has no viewer: run `just viewer` and build again, or pass --viewer <folder> (--no-viewer writes the data only)".into()));
                }
                html::write_viewer(&a.out, &built_in).map_err(|e| Fail(2, e.to_string()))?;
            }
        }
    }
    if a.json {
        println!("{}", serde_json::to_string(&json!({ "out": a.out, "report": report })).unwrap());
    } else {
        println!(
            "{} ({:?}, {} of {} budget{})",
            a.out.display(),
            report.mode,
            crate::size(report.bytes),
            crate::size(report.budget),
            if report.trimmed { ", trimmed to fit" } else { "" }
        );
        for f in &report.files {
            println!("  {}: {} samples, {}", f.name, crate::group(f.samples), crate::size(f.bytes));
            for c in &f.channels {
                println!(
                    "    {:<24} raw {:>10}  pyramid {:>10}{}",
                    c.name,
                    crate::size(c.raw_bytes),
                    crate::size(c.pyramid_bytes),
                    if c.first_level > 4 { format!("  (finest level {})", c.first_level) } else { String::new() }
                );
            }
        }
    }
    Ok(())
}

fn viewer_dir(given: Option<&Path>) -> Option<PathBuf> {
    let ok = |p: &Path| p.join("index.html").is_file();
    let mut tries: Vec<PathBuf> = given.map(Path::to_owned).into_iter().collect();
    tries.extend(std::env::var_os("TLMBENCH_VIEWER").map(PathBuf::from));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            tries.push(d.join("viewer"));
        }
        // a build in the repository: target/{debug,release}/tlmbench
        for up in exe.ancestors().skip(1).take(4) {
            tries.push(up.join("app/viewer/dist"));
        }
    }
    tries.into_iter().find(|p| ok(p))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dest = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &dest)?;
        } else {
            std::fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}
