//! TelemetryBench desktop app: the core runs in this process, and the web UI asks it for viewport reductions over
//! Tauri IPC, which return the binary wire format (`tlmbench_core::wire`) as raw bytes: one batch per request, for
//! every pane and the overview.
//!
//! Recordings open through the store (`tlmbench_store`): read-only, with their pyramid and gap index built in the
//! background into the shared cache root, never beside the data. While a build runs, views are exact from the
//! summarized part and the UI hatches the rest; its progress streams over a channel.
//!
//! `tlmbench-app <file>` opens a file at launch. `TLMBENCH_GATE=<file>` runs the Phase 1 gate and
//! `TLMBENCH_GATE4=<manifest>` the Phase 4 format matrix in the app (`just gate4 app`).
//! `TLMBENCH_GATE2=<file>` the Phase 2 gate (`just gate`, `just gate2`): a scripted run, then the report on stdout
//! and exit code 0 (pass) or 1.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, Emitter, Manager, State};
use tlmbench_core::gaps::DEFAULT_FACTOR;
use tlmbench_core::table::{self, Col, Filter, SortKey};
use tlmbench_core::timemap::TimeMap;
use tlmbench_core::{RecordingInfo, Viewport, wire};
use tlmbench_decode::profile::ImportProfile;
use tlmbench_store::group::Group;
use tlmbench_store::{Opened as StoreOpened, Precision, Request, Status, Store};

/// Open files, by id, and live sources (each a file that grows). Each window keeps its own view; files are shared.
struct Files {
    store: Store,
    next: AtomicU32,
    /// Which windows use each open file, group or live source (by id): closing in one window releases only its hold.
    holds: Holds,
    /// What a window made by moving a tab opens with, by the window's label (taken once, by that window).
    handoffs: Mutex<HashMap<String, serde_json::Value>>,
    /// The number the next window's label takes (`win-2`, `win-3`, …).
    next_window: AtomicU32,
    open: Mutex<HashMap<u32, Arc<StoreOpened>>>,
    /// A file with derived channels shows them joined to its recording: the recording alone, to join again when the
    /// recipes change.
    bases: Mutex<HashMap<u32, Arc<StoreOpened>>>,
    live: Mutex<HashMap<u32, Arc<LiveEntry>>>,
    /// A tab's several files on one timeline, by the id the tab shows (the members stay open under their own ids).
    groups: Mutex<HashMap<u32, Arc<Group>>>,
    /// Sorted and filtered row lists, by query token (the latest per table; see `table_query`).
    queries: Mutex<HashMap<u32, Arc<RowQuery>>>,
    /// Running queries' cancel flags, by token.
    cancels: Mutex<HashMap<u32, Arc<AtomicBool>>>,
}

/// Windows holding each id. An id nobody holds is closed at once.
#[derive(Default)]
struct Holds(Mutex<HashMap<u32, std::collections::BTreeSet<String>>>);

impl Holds {
    fn add(&self, id: u32, window: &str) {
        self.0.lock().unwrap().entry(id).or_default().insert(window.to_string());
    }

    /// Drops the window's hold; true when nobody holds the id any more (it is to be closed).
    fn release(&self, id: u32, window: &str) -> bool {
        let mut m = self.0.lock().unwrap();
        let Some(set) = m.get_mut(&id) else { return true };
        set.remove(window);
        if set.is_empty() {
            m.remove(&id);
            true
        } else {
            false
        }
    }

    /// Every id the window holds.
    fn of(&self, window: &str) -> Vec<u32> {
        self.0.lock().unwrap().iter().filter(|(_, w)| w.contains(window)).map(|(i, _)| *i).collect()
    }

    /// The other windows holding the id.
    fn others(&self, id: u32, window: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .get(&id)
            .map(|s| s.iter().filter(|w| *w != window).cloned().collect())
            .unwrap_or_default()
    }
}

impl Files {
    /// The group a tab shows, if it has several files.
    fn group(&self, id: u32) -> Option<Arc<Group>> {
        let g = self.groups.lock().unwrap().get(&id).cloned()?;
        // a live source's recording is a new snapshot after every refresh: the group is made again from the newest ones
        // (same files, same maps), so a tab of live sources grows with them
        let live = self.live.lock().unwrap();
        if !g.file_ids().iter().any(|i| live.contains_key(i)) {
            return Some(g);
        }
        drop(live);
        let parts: Option<Vec<_>> =
            g.files().into_iter().zip(g.file_ids()).map(|((_, m), &i)| self.get(i).ok().map(|o| (o, m))).collect();
        let fresh = Arc::new(Group::new(parts?, g.file_ids().to_vec()));
        self.groups.lock().unwrap().insert(id, fresh.clone());
        Some(fresh)
    }

    /// The file as opened (before derived channels join it), or a live source's as of its last refresh: what events
    /// and checks are kept against.
    fn base(&self, id: u32) -> Result<Arc<StoreOpened>, String> {
        if let Some(o) = self.bases.lock().unwrap().get(&id) {
            return Ok(o.clone());
        }
        // a tab of several files: events and checks are kept against its primary
        if let Some(g) = self.groups.lock().unwrap().get(&id).cloned() {
            return self.base(g.file_ids()[0]);
        }
        // a live source's events are kept against the recording itself, not its derived channels' join
        let live = self.live.lock().unwrap().get(&id).cloned();
        live.map(|l| l.follow.read().unwrap().opened()).ok_or_else(|| "that file is not open".into())
    }

    /// The file's recording, or a live source's as of its last refresh.
    fn get(&self, id: u32) -> Result<Arc<StoreOpened>, String> {
        if let Some(o) = self.open.lock().unwrap().get(&id) {
            return Ok(o.clone());
        }
        // commands that don't know several files act on the primary's
        if let Some(g) = self.group(id) {
            return Ok(g.primary().clone());
        }
        let live = self.live.lock().unwrap().get(&id).cloned();
        live.map(|l| l.opened()).ok_or_else(|| "that file is not open".into())
    }
}

/// A live source: recorded into a session (tlmbench_live) and followed as its file grows (Store::open_live).
struct LiveEntry {
    source: Mutex<Option<tlmbench_live::Source>>,
    /// The source's newest part, followed (a tailed file's rotation starts a new one).
    follow: RwLock<Arc<tlmbench_store::Live>>,
    /// Recipes computed over it as it grows, joined to it (set by `derive_apply`).
    derived: RwLock<Option<Arc<tlmbench_store::LiveDerived>>>,
    part: AtomicUsize,
    session: std::path::PathBuf,
    /// What the Sources panel calls it.
    name: String,
    /// The session it records into (shared by the sources added to it) and its place there.
    recording: Arc<Mutex<tlmbench_live::Session>>,
    /// Whether the session is kept: one that isn't recorded lives in the cache and goes with its last source.
    keep: bool,
    /// The last health while the source ran (kept once it stops).
    last: Mutex<tlmbench_live::Health>,
    closed: AtomicBool,
}

impl LiveEntry {
    /// The recording with its derived channels (if it has some), as of the last refresh.
    fn opened(&self) -> Arc<StoreOpened> {
        match self.derived.read().unwrap().as_ref() {
            Some(d) => d.opened(),
            None => self.follow.read().unwrap().opened(),
        }
    }

    fn health(&self) -> tlmbench_live::Health {
        let g = self.source.lock().unwrap();
        match g.as_ref() {
            Some(s) => {
                let h = s.health();
                *self.last.lock().unwrap() = h.clone();
                h
            }
            None => self.last.lock().unwrap().clone(),
        }
    }
}

#[derive(Serialize)]
struct Opened {
    id: u32,
    info: RecordingInfo,
    /// First and last sample times as decimal strings: JSON numbers lose i64 nanoseconds past 2^53.
    range: Option<(String, String)>,
    status: Status,
    /// The derived channels among `info.channels` (they come after the recording's own).
    derived: Vec<DerivedInfo>,
    /// Recipes asked for that don't apply to this file, and why.
    skipped: Vec<Skipped>,
    /// Channels with samples called missing (see `Adjust`): how many, and for how long.
    missing: Vec<Missing>,
}

/// How much of a channel is missing.
#[derive(Serialize)]
struct Missing {
    channel: usize,
    count: usize,
    /// Nanoseconds the missing samples stand for (each lasts until the next sample).
    ns: i64,
}

/// Missing samples of `rec`'s channels in samples `a..b`, for those with any.
fn missing_in(
    rec: &dyn tlmbench_core::Recording,
    channels: impl Iterator<Item = usize>,
    a: usize,
    b: usize,
) -> Vec<Missing> {
    let n = rec.info().samples;
    channels
        .filter_map(|channel| {
            let mask = rec.missing_mask(channel);
            let (mut count, mut ns) = (0usize, 0i64);
            for r in mask.runs() {
                let (s, e) = (r.start.max(a), r.end.min(b));
                if s >= e {
                    continue;
                }
                count += e - s;
                let end = if e < n { rec.time_at(e) } else { rec.time_at(n - 1) };
                ns += end - rec.time_at(s);
            }
            (count > 0).then_some(Missing { channel, count, ns })
        })
        .collect()
}

/// Every channel's missing samples over the whole recording.
fn missing_all(rec: &dyn tlmbench_core::Recording) -> Vec<Missing> {
    missing_in(rec, 0..rec.info().channels.len(), 0, rec.info().samples)
}

/// One channel's adjustments from the app's settings (see `tlmbench_store::Adjust`).
#[derive(Deserialize)]
struct AdjustIn {
    channel: String,
    scale: Option<f64>,
    offset: Option<f64>,
    values: Option<Vec<f64>>,
    lo: Option<f64>,
    hi: Option<f64>,
    when: Option<String>,
}

fn adjusts(a: Option<Vec<AdjustIn>>) -> Vec<(String, tlmbench_store::Adjust)> {
    a.unwrap_or_default()
        .into_iter()
        .map(|a| {
            (
                a.channel,
                tlmbench_store::Adjust {
                    scale: a.scale.unwrap_or(1.0),
                    offset: a.offset.unwrap_or(0.0),
                    values: a.values.unwrap_or_default(),
                    lo: a.lo,
                    hi: a.hi,
                    when: a.when.filter(|w| !w.trim().is_empty()),
                },
            )
        })
        .collect()
}

/// A derived channel: where it is in the channel list, and the recipe it comes from.
#[derive(Serialize)]
struct DerivedInfo {
    channel: usize,
    name: String,
    expr: String,
    automatic: bool,
}

#[derive(Serialize)]
struct Skipped {
    name: String,
    why: String,
}

/// What a file shows once its recipes are applied: the recording (joined with its derived channels), the channels
/// derived, and the attached recipes that didn't apply.
type Shown = (Arc<StoreOpened>, Vec<DerivedInfo>, Vec<Skipped>);

/// `base` with the library's recipes that apply to it (those named in `attached`, in that order, then the automatic
/// ones) as channels after its own: what the file shows, the channels derived, and the attached recipes that didn't apply.
fn with_derived(
    store: &Store,
    base: &Arc<StoreOpened>,
    attached: &[String],
    exclude: &[String],
    foreign: &[tlmbench_store::Foreign],
) -> Result<Shown, String> {
    let (recipes, mut skipped) = pick_recipes(store, attached, exclude);
    if recipes.is_empty() {
        return Ok((base.clone(), Vec::new(), skipped));
    }
    let d = store.open_derived_cross(base, &recipes, foreign).map_err(|e| e.to_string())?;
    // an automatic recipe that needs channels the file lacks just doesn't apply; one the user attached is reported
    skipped.extend(
        d.skipped
            .iter()
            .filter(|(n, _)| attached.contains(n))
            .map(|(n, e)| Skipped { name: n.clone(), why: e.msg.clone() }),
    );
    let Some(opened) = d.opened else { return Ok((base.clone(), Vec::new(), skipped)) };
    let first = base.recording().info().channels.len();
    let derived = derived_infos(&recipes, &d.names, first);
    let joined = store.join(base.clone(), Arc::new(opened)).map_err(|e| e.to_string())?;
    Ok((Arc::new(joined), derived, skipped))
}

fn derived_infos(recipes: &[tlmbench_expr::Recipe], names: &[String], first: usize) -> Vec<DerivedInfo> {
    names
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let r = recipes.iter().find(|r| &r.name == name).expect("computed from a recipe in the list");
            DerivedInfo { channel: first + k, name: name.clone(), expr: r.expr.clone(), automatic: r.automatic }
        })
        .collect()
}

/// The library's recipes a file gets: those named in `attached` in that order, then the automatic ones not excluded;
/// and the attached names that aren't in the library.
fn pick_recipes(store: &Store, attached: &[String], exclude: &[String]) -> (Vec<tlmbench_expr::Recipe>, Vec<Skipped>) {
    let lib = tlmbench_store::Library::new(store.root());
    // the attached ones in the order given (so a channel keeps its number when others come and go), then the
    // automatic ones that weren't named, by name
    let all = lib.list();
    let mut recipes: Vec<tlmbench_expr::Recipe> = Vec::new();
    for n in attached {
        if let Some(r) = all.iter().find(|r| &r.name == n).filter(|r| !recipes.iter().any(|x| x.name == r.name)) {
            recipes.push(r.clone());
        }
    }
    let rest: Vec<_> = all
        .into_iter()
        .filter(|r| r.automatic && !exclude.contains(&r.name) && !recipes.iter().any(|x| x.name == r.name))
        .collect();
    recipes.extend(rest);
    let skipped: Vec<Skipped> = attached
        .iter()
        .filter(|n| !recipes.iter().any(|r| &r.name == *n))
        .map(|n| Skipped { name: n.clone(), why: "There is no recipe with that name".into() })
        .collect();
    (recipes, skipped)
}

/// Builds `o`'s pyramid in the background, reporting progress (the part of `open` shared with applying recipes).
fn build_in_background(o: Arc<StoreOpened>, progress: Channel<Progress>) {
    std::thread::spawn(move || {
        let rec = o.recording();
        let send = |done: u64, total: u64, error: Option<String>| {
            let built_until = done.checked_sub(1).map(|i| rec.time_at(i as usize).to_string());
            let _ = progress.send(Progress { done, total, built_until, error });
        };
        if let Err(e) = o.build(&mut |done, total| send(done, total, None)) {
            send(0, rec.info().samples as u64, Some(e.to_string()));
        }
    });
}

/// Build progress, streamed while the pyramid is built.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: u64,
    total: u64,
    /// Samples up to this time (a decimal string) are summarized.
    built_until: Option<String>,
    error: Option<String>,
}

fn parse_time(s: &str) -> Result<i64, String> {
    s.parse::<i64>().map_err(|e| format!("time {s:?}: {e}"))
}

/// The saved import profile for a CSV file's header, if there is exactly one (the app applies it on every open).
/// The saved profile for a header, yours first, else the one the folder's `.tlmbench/profiles/` shares.
fn find_profile(root: &Path, header: &[String]) -> tlmbench_decode::profile::Found {
    let mut found = tlmbench_decode::profile::find(root, header);
    if found.exact.is_none() {
        found.exact = tlmbench_store::config::shared_profile(&tlmbench_store::config::overlay(), header);
    }
    found
}

fn saved_profile(root: &Path, path: &Path) -> Option<ImportProfile> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "csv" | "tsv" | "txt") {
        return None;
    }
    let i = tlmbench_decode::csv::inspect_file(path).ok()?;
    let header: Vec<String> = i.columns.into_iter().map(|c| c.source).collect();
    find_profile(root, &header).exact
}

/// What the import dialog needs about one file: its columns and first rows as the core reads them, and the saved
/// profile for its header (exact: applied without asking; near: offered).
#[derive(Serialize)]
struct Inspected {
    path: String,
    inspection: Option<tlmbench_decode::csv::Inspection>,
    error: Option<String>,
    found: tlmbench_decode::profile::Found,
}

#[tauri::command]
async fn import_inspect(paths: Vec<String>, files: State<'_, Files>) -> Result<Vec<Inspected>, String> {
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        paths
            .into_iter()
            .map(|path| {
                let p = Path::new(&path);
                let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
                if !matches!(ext.as_deref(), Some("csv" | "tsv" | "txt")) {
                    let error = Some("not a CSV, TSV or TXT file".to_owned());
                    return Inspected { path, inspection: None, error, found: Default::default() };
                }
                match tlmbench_decode::csv::inspect_file(p) {
                    Ok(i) => {
                        let header: Vec<String> = i.columns.iter().map(|c| c.source.clone()).collect();
                        let found = find_profile(&root, &header);
                        Inspected { path, inspection: Some(i), error: None, found }
                    }
                    Err(e) => {
                        Inspected { path, inspection: None, error: Some(e.to_string()), found: Default::default() }
                    }
                }
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Keeps an import profile for a header in the app cache (`header` as the core names the columns).
#[tauri::command]
async fn profile_save(header: Vec<String>, profile: ImportProfile, files: State<'_, Files>) -> Result<(), String> {
    profile.validate()?;
    let root = files.store.root().to_owned();
    spawn_blocking(move || tlmbench_decode::profile::save(&root, &header, &profile).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// The packet types of the descriptor that names `path` (each opens as its own recording); none when the file has no
/// packet types.
#[tauri::command]
async fn packet_parts(path: String, files: State<'_, Files>) -> Result<Vec<String>, String> {
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let Some(found) = tlmbench_decode::descriptor::discover(Path::new(&path), &root) else { return Vec::new() };
        let d = tlmbench_decode::descriptor::Descriptor::load(&found).ok();
        d.map(|d| d.packet_types.into_iter().map(|t| t.name).collect()).unwrap_or_default()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Opens a file with the recipes and adjustments it should have (one IPC argument each, as the UI sends them).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn open(
    path: String,
    profile: Option<ImportProfile>,
    part: Option<String>,
    recipes: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    adjust: Option<Vec<AdjustIn>>,
    progress: Channel<Progress>,
    window: tauri::Window,
    files: State<'_, Files>,
) -> Result<Opened, String> {
    let store = files.store.clone();
    let adjust = adjusts(adjust);
    let attached = recipes.unwrap_or_default();
    let exclude = exclude.unwrap_or_default();
    let (base, shown, derived, skipped) = spawn_blocking(move || -> Result<_, String> {
        let profile = profile.or_else(|| saved_profile(store.root(), Path::new(&path)));
        let base = store
            .clone()
            .with_profile(profile)
            .with_adjust(adjust)
            .open_part(Path::new(&path), None, part.as_deref())
            .map_err(|e| e.to_string())?;
        let base = Arc::new(base);
        let (shown, derived, skipped) = with_derived(&store, &base, &attached, &exclude, &[])?;
        Ok((base, shown, derived, skipped))
    })
    .await
    .map_err(|e| e.to_string())??;
    let id = files.next.fetch_add(1, Ordering::Relaxed) + 1;
    let info = shown.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    let status = shown.status();
    files.holds.add(id, window.label());
    files.open.lock().unwrap().insert(id, shown.clone());
    files.bases.lock().unwrap().insert(id, base);
    if status != Status::Built {
        build_in_background(shown.clone(), progress);
    }
    let missing = missing_all(shown.recording());
    Ok(Opened { id, info, range, status, derived, skipped, missing })
}

/// One file of a tab with several, as asked for.
#[derive(Deserialize)]
struct MemberIn {
    file: u32,
    /// `(t_file, t_shared)` in nanoseconds as decimal strings; none is the identity.
    knots: Vec<(String, String)>,
}

/// One file of a group as it is now.
#[derive(Serialize)]
struct SlotOut {
    file: u32,
    /// The file's first channel in the group's numbering, and how many it has.
    first: usize,
    count: usize,
    knots: Vec<(String, String)>,
    /// The file's own first and last times.
    own: Option<(String, String)>,
}

/// A tab's files on one timeline.
#[derive(Serialize)]
struct GroupOpened {
    id: u32,
    info: RecordingInfo,
    range: Option<(String, String)>,
    status: Status,
    slots: Vec<SlotOut>,
}

fn knots_in(k: &[(String, String)]) -> Result<TimeMap, String> {
    let v: Result<Vec<(i64, i64)>, String> = k.iter().map(|(a, b)| Ok((parse_time(a)?, parse_time(b)?))).collect();
    TimeMap::new(v?).map_err(|e| e.to_string())
}

fn knots_out(m: &TimeMap) -> Vec<(String, String)> {
    m.knots.iter().map(|&(a, b)| (a.to_string(), b.to_string())).collect()
}

fn group_out(id: u32, g: &Group) -> GroupOpened {
    let mut info = g.info().clone();
    let (a, b) = g.bounds_now();
    (info.t_first, info.t_last) = (a, b);
    GroupOpened {
        id,
        range: a.zip(b).map(|(a, b)| (a.to_string(), b.to_string())),
        status: g.status(),
        slots: g
            .slots()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let own = s.opened.recording().info();
                let _ = i;
                SlotOut {
                    file: g.file_ids()[i],
                    first: s.first,
                    count: s.count,
                    knots: knots_out(&s.map()),
                    own: own.t_first.zip(own.t_last).map(|(a, b)| (a.to_string(), b.to_string())),
                }
            })
            .collect(),
        info,
    }
}

/// Puts the files in `members` (the first is the primary) on one timeline, as the tab's file: the group with `group`'s
/// id when it is given (the same id, so the tab's panes stay), else a new one. The members stay open under their own
/// ids. Channels are numbered slot after slot.
#[tauri::command]
async fn group_set(
    group: Option<u32>,
    members: Vec<MemberIn>,
    progress: Channel<Progress>,
    window: tauri::Window,
    files: State<'_, Files>,
) -> Result<GroupOpened, String> {
    if members.is_empty() {
        return Err("A tab needs a file".into());
    }
    let mut parts = Vec::new();
    let mut ids = Vec::new();
    for m in &members {
        let o = files.get(m.file).map_err(|_| "that file is not open".to_string())?;
        parts.push((o, knots_in(&m.knots)?));
        ids.push(m.file);
    }
    let g = Arc::new(Group::new(parts, ids));
    let id = group.unwrap_or_else(|| files.next.fetch_add(1, Ordering::Relaxed) + 1);
    // the files it held stay open (and building): only closing one stops its build
    files.holds.add(id, window.label());
    files.groups.lock().unwrap().insert(id, g.clone());
    let out = group_out(id, &g);
    if out.status != Status::Built {
        let g = g.clone();
        std::thread::spawn(move || {
            let send = |done: u64, total: u64, error: Option<String>| {
                let _ = progress.send(Progress { done, total, built_until: None, error });
            };
            if let Err(e) = g.build(&mut |d, t| send(d, t, None)) {
                send(0, 0, Some(e.to_string()));
            }
        });
    }
    Ok(out)
}

/// The tab's files as they are now: a tab of live sources grows with them.
#[tauri::command]
fn group_state(group: u32, files: State<'_, Files>) -> Result<GroupOpened, String> {
    let g = files.group(group).ok_or("that tab has one file")?;
    Ok(group_out(group, &g))
}

/// Replaces one file's time map in a group (a nudge, a method's result): nothing is rebuilt.
#[tauri::command]
fn group_map(
    group: u32,
    slot: usize,
    knots: Vec<(String, String)>,
    files: State<'_, Files>,
) -> Result<GroupOpened, String> {
    let g = files.group(group).ok_or("that tab has one file")?;
    if slot >= g.slots().len() {
        return Err("no such file in this tab".into());
    }
    g.set_map(slot, knots_in(&knots)?);
    Ok(group_out(group, &g))
}

/// What an alignment method finds for one file of a group against the primary, without applying it: the map, how far
/// each matched point is from it, a quality in 0..1 and a note.
#[tauri::command]
async fn align_estimate(
    group: u32,
    slot: usize,
    method: tlmbench_store::align::Method,
    files: State<'_, Files>,
) -> Result<AlignOut, String> {
    let g = files.group(group).ok_or("that tab has one file")?;
    let root = files.store.root().to_owned();
    if slot == 0 || slot >= g.slots().len() {
        return Err("the primary file is what the others line up with".into());
    }
    spawn_blocking(move || {
        let (r, t) = (&g.slots()[0], &g.slots()[slot]);
        let a = tlmbench_store::align::estimate(&root, &r.opened, &r.map(), &t.opened, &method)
            .map_err(|e| e.to_string())?;
        Ok(AlignOut {
            knots: knots_out(&a.map),
            residuals: a.residuals.iter().map(|&(t, r)| (t.to_string(), r)).collect(),
            quality: a.quality,
            note: a.note,
            channel: a.channel,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct AlignOut {
    knots: Vec<(String, String)>,
    residuals: Vec<(String, f64)>,
    quality: f64,
    note: Option<String>,
    channel: Option<String>,
}

/// Changes which recipes an open file shows (the automatic ones, and these): the same file id, with its channels
/// recomputed. The recording's own channels keep their numbers; derived ones come after.
#[tauri::command]
async fn derive_apply(
    file: u32,
    recipes: Vec<String>,
    exclude: Vec<String>,
    // the tab's group, when the file is the primary of several: recipes may read the other files' channels
    group: Option<u32>,
    progress: Channel<Progress>,
    files: State<'_, Files>,
) -> Result<Opened, String> {
    // a live source: the recipes follow it as it grows
    let live = files.live.lock().unwrap().get(&file).cloned();
    if let Some(l) = live {
        let store = files.store.clone();
        return spawn_blocking(move || -> Result<Opened, String> {
            let (recipes, mut skipped) = pick_recipes(&store, &recipes, &exclude);
            let base = l.follow.read().unwrap().clone();
            let first = base.opened().recording().info().channels.len();
            let mut derived = Vec::new();
            *l.derived.write().unwrap() = None;
            if !recipes.is_empty() {
                let d = store.open_live_derived(base, &recipes).map_err(|e| e.to_string())?;
                skipped.extend(d.skipped.iter().map(|(n, why)| Skipped { name: n.clone(), why: why.clone() }));
                derived = derived_infos(&recipes, &d.names, first);
                if !d.names.is_empty() {
                    *l.derived.write().unwrap() = Some(Arc::new(d.live));
                }
            }
            let o = l.opened();
            let info = o.recording().info().clone();
            let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
            Ok(Opened { id: file, info, range, status: Status::Built, derived, skipped, missing: Vec::new() })
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let base = files.bases.lock().unwrap().get(&file).cloned().ok_or("that file is not open")?;
    let store = files.store.clone();
    let b = base.clone();
    let foreign = group.and_then(|g| files.group(g)).map_or_else(Vec::new, |g| g.foreign());
    let (shown, derived, skipped) = spawn_blocking(move || with_derived(&store, &b, &recipes, &exclude, &foreign))
        .await
        .map_err(|e| e.to_string())??;
    let info = shown.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    let status = shown.status();
    if let Some(old) = files.open.lock().unwrap().insert(file, shown.clone()) {
        old.cancel();
    }
    if status != Status::Built {
        build_in_background(shown.clone(), progress);
    }
    let missing = missing_all(shown.recording());
    Ok(Opened { id: file, info, range, status, derived, skipped, missing })
}

/// One value label: channel `channel`'s stored number `value` is called `text`.
#[derive(Deserialize)]
struct LabelIn {
    channel: String,
    text: String,
    value: f64,
}

/// One channel's warn or alarm limits.
#[derive(Deserialize)]
struct LimitIn {
    channel: String,
    alarm: bool,
    lo: Option<f64>,
    hi: Option<f64>,
}

/// The value labels and limits that derived channels' expressions read (`state == 'flight'`, `past_warn(x)`), from the
/// app's settings. Set before files open and again when they change.
#[tauri::command]
fn set_expr_env(labels: Vec<LabelIn>, limits: Vec<LimitIn>, files: State<'_, Files>) {
    files.store.set_expr_env(tlmbench_store::ExprEnv {
        labels: labels.into_iter().map(|l| ((l.channel, l.text), l.value)).collect(),
        limits: limits.into_iter().map(|l| ((l.channel, l.alarm), (l.lo, l.hi))).collect(),
    });
}

/// Opens an open file again with other adjustments (scale and offset, missing values) and the recipes it has now:
/// the same file id, a new pyramid for the new values.
#[tauri::command]
async fn adjust_apply(
    file: u32,
    adjust: Vec<AdjustIn>,
    recipes: Vec<String>,
    exclude: Vec<String>,
    progress: Channel<Progress>,
    files: State<'_, Files>,
) -> Result<Opened, String> {
    let old = files.bases.lock().unwrap().get(&file).cloned().ok_or("that file is not open")?;
    let store = files.store.clone();
    let adjust = adjusts(Some(adjust));
    let (base, shown, derived, skipped) = spawn_blocking(move || -> Result<_, String> {
        let info = old.recording().info();
        let base = old
            .store()
            .clone()
            .with_adjust(adjust)
            .open_part(&info.path, None, info.part.as_deref())
            .map_err(|e| e.to_string())?;
        let base = Arc::new(base);
        let (shown, derived, skipped) = with_derived(&store, &base, &recipes, &exclude, &[])?;
        Ok((base, shown, derived, skipped))
    })
    .await
    .map_err(|e| e.to_string())??;
    let info = shown.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    let status = shown.status();
    if let Some(prev) = files.open.lock().unwrap().insert(file, shown.clone()) {
        prev.cancel();
    }
    files.bases.lock().unwrap().insert(file, base);
    if status != Status::Built {
        build_in_background(shown.clone(), progress);
    }
    let missing = missing_all(shown.recording());
    Ok(Opened { id: file, info, range, status, derived, skipped, missing })
}

/// Yours, then the events the folder's `.tlmbench/events/` shares for this recording that you don't have (read-only,
/// source `shared`).
fn with_shared(
    mut mine: Vec<tlmbench_store::events::Event>,
    path: &str,
    identity: &str,
) -> Vec<tlmbench_store::events::Event> {
    let dirs = tlmbench_store::config::overlay();
    if let Some(parent) = dirs.first().and_then(|d| d.parent()) {
        let name =
            Path::new(path).strip_prefix(parent).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        for e in tlmbench_store::config::shared_events(&dirs, identity, &name) {
            if !mine.iter().any(|m| m.id == e.id) {
                mine.push(e);
            }
        }
        mine.sort_by_key(|e| e.start.parse::<i64>().unwrap_or(0));
    }
    mine
}

/// The events you added to an open file (kept in the app cache under the file's identity), in start order, and the
/// ones the folder shares.
#[tauri::command]
async fn events_list(file: u32, files: State<'_, Files>) -> Result<Vec<tlmbench_store::events::Event>, String> {
    let base = files.base(file)?;
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let mine = tlmbench_store::events::list(&root, base.identity()).map_err(|e| e.to_string())?;
        Ok(with_shared(mine, &base.recording().info().path.to_string_lossy(), base.identity()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Adds an event or replaces the one with its id; the events now.
#[tauri::command]
async fn event_save(
    file: u32,
    mut event: tlmbench_store::events::Event,
    files: State<'_, Files>,
) -> Result<Vec<tlmbench_store::events::Event>, String> {
    let base = files.base(file)?;
    let root = files.store.root().to_owned();
    if event.by.is_none() && event.source == "user" {
        event.by = user_name();
    }
    spawn_blocking(move || {
        let mine = tlmbench_store::events::save(&root, base.identity(), event).map_err(|e| e.to_string())?;
        Ok(with_shared(mine, &base.recording().info().path.to_string_lossy(), base.identity()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes an event; the events now.
#[tauri::command]
async fn event_delete(
    file: u32,
    id: String,
    files: State<'_, Files>,
) -> Result<Vec<tlmbench_store::events::Event>, String> {
    let base = files.base(file)?;
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let mine = tlmbench_store::events::delete(&root, base.identity(), &id).map_err(|e| e.to_string())?;
        Ok(with_shared(mine, &base.recording().info().path.to_string_lossy(), base.identity()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Missing samples of the given channels between two times (the cursors, or the view).
#[tauri::command]
async fn missing_between(
    file: u32,
    channels: Vec<u32>,
    t0: String,
    t1: String,
    files: State<'_, Files>,
) -> Result<Vec<Missing>, String> {
    let o = files.get(file)?;
    let (t0, t1): (i64, i64) = (t0.parse().map_err(|_| "bad time")?, t1.parse().map_err(|_| "bad time")?);
    spawn_blocking(move || {
        let rec = o.recording();
        let (a, b) = (rec.lower_bound(t0), rec.lower_bound(t1.saturating_add(1)));
        missing_in(rec, channels.into_iter().map(|c| c as usize), a, b)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn recipes_list(files: State<'_, Files>) -> Vec<tlmbench_expr::Recipe> {
    tlmbench_store::Library::new(files.store.root()).list()
}

/// Saves a recipe; `replacing` is its name before an edit (a rename).
#[tauri::command]
fn recipe_save(
    recipe: tlmbench_expr::Recipe,
    replacing: Option<String>,
    files: State<'_, Files>,
) -> Result<(), String> {
    // the expression must at least parse: a recipe that can't is never saved
    tlmbench_expr::parse(&recipe.expr).map_err(|e| format!("{} (at {})", e.msg, e.at))?;
    tlmbench_store::Library::new(files.store.root()).save(&recipe, replacing.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn recipe_delete(name: String, files: State<'_, Files>) -> Result<(), String> {
    tlmbench_store::Library::new(files.store.root()).delete(&name).map_err(|e| e.to_string())
}

/// Adds the recipes in a recipe file's text; their names.
#[tauri::command]
fn recipes_import(text: String, files: State<'_, Files>) -> Result<Vec<String>, String> {
    tlmbench_store::Library::new(files.store.root()).import(&text)
}

/// The named recipes (all when none are named) as a recipe file's text.
#[tauri::command]
fn recipes_export(names: Vec<String>, files: State<'_, Files>) -> String {
    tlmbench_store::Library::new(files.store.root()).export(&names)
}

/// The check sets the app keeps (the cache's `checks/`).
#[tauri::command]
fn check_sets_list(files: State<'_, Files>) -> Vec<tlmbench_store::checks::CheckSet> {
    tlmbench_store::checks::Sets::new(files.store.root()).list()
}

/// Saves a set; `replacing` is its name before an edit (a rename).
#[tauri::command]
fn check_set_save(
    set: tlmbench_store::checks::CheckSet,
    replacing: Option<String>,
    files: State<'_, Files>,
) -> Result<(), String> {
    tlmbench_store::checks::Sets::new(files.store.root()).save(&set, replacing.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn check_set_delete(name: String, files: State<'_, Files>) -> Result<(), String> {
    tlmbench_store::checks::Sets::new(files.store.root()).delete(&name).map_err(|e| e.to_string())
}

/// A set's results on a file, and the read-only events they make.
#[derive(Serialize)]
struct CheckRun {
    set: String,
    result: tlmbench_store::checks::SetResult,
    events: Vec<tlmbench_store::events::Event>,
    /// The file's first sample (ns, decimal), for ordering a folder run by date.
    t_first: String,
}

/// Runs sets (by name) on an open file, or reads the results kept for its contents and the sets' versions.
#[tauri::command]
async fn checks_run(file: u32, sets: Vec<String>, files: State<'_, Files>) -> Result<Vec<CheckRun>, String> {
    let live = files.live.lock().unwrap().contains_key(&file);
    let base = if live { files.get(file)? } else { files.base(file)? };
    let store = files.store.clone();
    spawn_blocking(move || -> Result<Vec<CheckRun>, String> {
        let library = tlmbench_store::checks::Sets::new(store.root());
        let recipes = tlmbench_store::Library::new(store.root()).list();
        sets.iter()
            .filter_map(|n| library.get(n))
            .map(|set| {
                // a recording that grows is checked afresh each time, and not kept
                let result = if live {
                    store.run_checks_live(&base, &set, &recipes)
                } else {
                    store.run_checks(&base, &set, &recipes).map_err(|e| e.to_string())?
                };
                let events = tlmbench_store::checks::events_of(&set, &result);
                Ok(CheckRun {
                    set: set.name,
                    result,
                    events,
                    t_first: base.recording().info().t_first.unwrap_or(0).to_string(),
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sets run on a file that isn't open (the folder run): opened as `open` would, not kept; nothing is shown of it.
#[tauri::command]
async fn checks_run_path(
    path: String,
    sets: Vec<String>,
    adjust: Option<Vec<AdjustIn>>,
    files: State<'_, Files>,
) -> Result<Vec<CheckRun>, String> {
    let store = files.store.clone();
    let adjust = adjusts(adjust);
    spawn_blocking(move || -> Result<Vec<CheckRun>, String> {
        let profile = saved_profile(store.root(), Path::new(&path));
        let base = store
            .clone()
            .with_profile(profile)
            .with_adjust(adjust)
            .open_part(Path::new(&path), None, None)
            .map_err(|e| e.to_string())?;
        let library = tlmbench_store::checks::Sets::new(store.root());
        let recipes = tlmbench_store::Library::new(store.root()).list();
        sets.iter()
            .filter_map(|n| library.get(n))
            .map(|set| {
                let result = store.run_checks(&base, &set, &recipes).map_err(|e| e.to_string())?;
                let events = tlmbench_store::checks::events_of(&set, &result);
                Ok(CheckRun {
                    set: set.name,
                    result,
                    events,
                    t_first: base.recording().info().t_first.unwrap_or(0).to_string(),
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Writes text to a file the person chose in a save dialog (a report, a CSV).
#[tauri::command]
fn write_text_file(path: String, text: String) -> Result<(), String> {
    std::fs::write(&path, text).map_err(|e| format!("{path}: {e}"))
}

/// One row run on the file in front, for the editor's preview (nothing is kept).
#[tauri::command]
async fn check_preview(
    file: u32,
    check: tlmbench_store::checks::Check,
    files: State<'_, Files>,
) -> Result<tlmbench_store::checks::Outcome, String> {
    let base = files.base(file)?;
    let store = files.store.clone();
    spawn_blocking(move || {
        let recipes = tlmbench_store::Library::new(store.root()).list();
        store.check_one(&base, &check, &recipes)
    })
    .await
    .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct CheckError {
    msg: String,
    at: usize,
}

/// What the Derive dialog's check found: a preview of the result, or the mistake.
#[derive(Serialize)]
struct CheckReply {
    preview: Option<tlmbench_store::Preview>,
    error: Option<CheckError>,
}

/// The Derive dialog's check: an expression against a file's channels, with a summary of what it gives over the start
/// of the file, or the mistake and the character it is at.
#[tauri::command]
async fn derive_check(file: u32, expr: String, files: State<'_, Files>) -> Result<CheckReply, String> {
    let o = files.get(file)?;
    let foreign = files.group(file).map_or_else(Vec::new, |g| g.foreign());
    spawn_blocking(move || match tlmbench_store::check_cross(o.recording(), &expr, &foreign) {
        Ok(p) => CheckReply { preview: Some(p), error: None },
        Err(e) => CheckReply { preview: None, error: Some(CheckError { msg: e.msg, at: e.at }) },
    })
    .await
    .map_err(|e| e.to_string())
}

/// A channel's spectrum over a time view.
#[derive(Serialize)]
struct SpectrumReply {
    channel: u32,
    /// Hz between bins, and the Nyquist frequency.
    df: f64,
    nyquist: f64,
    peak_hz: f64,
    peak_amp: f64,
    /// Samples transformed.
    n: usize,
    /// Amplitude per bin, DC first.
    mag: Vec<f32>,
}

/// The Spectrum view: each channel's frequency content over `t0..t1` (none for a view under 16 samples).
#[tauri::command]
async fn spectrum(
    file: u32,
    channels: Vec<u32>,
    t0: String,
    t1: String,
    files: State<'_, Files>,
) -> Result<Vec<SpectrumReply>, String> {
    let o = files.get(file)?;
    let (t0, t1) = (parse_time(&t0)?, parse_time(&t1)?);
    spawn_blocking(move || {
        channels
            .iter()
            .filter_map(|&c| {
                let s = o.spectrum(c as usize, t0, t1, tlmbench_expr::fft::MAX_POINTS)?;
                Some(SpectrumReply {
                    channel: c,
                    df: s.df,
                    nyquist: s.nyquist,
                    peak_hz: s.peak_hz,
                    peak_amp: s.peak_amp,
                    n: s.n,
                    mag: s.mag.iter().map(|&m| m as f32).collect(),
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// A window stops using a file, group or source: it closes when no window holds it.
#[tauri::command]
fn close(file: u32, window: tauri::Window, files: State<'_, Files>) {
    if files.holds.release(file, window.label()) {
        close_now(file, &files);
    }
}

/// Another window starts using an open file, group or live source (a tab moved or shown there too).
#[tauri::command]
fn hold(file: u32, window: tauri::Window, files: State<'_, Files>) -> Result<(), String> {
    let known = files.open.lock().unwrap().contains_key(&file)
        || files.groups.lock().unwrap().contains_key(&file)
        || files.live.lock().unwrap().contains_key(&file);
    if !known {
        return Err("that file is not open".into());
    }
    files.holds.add(file, window.label());
    Ok(())
}

/// The other windows using a file (a live source stopped in one window asks before stopping everywhere).
#[tauri::command]
fn holders(file: u32, window: tauri::Window, files: State<'_, Files>) -> Vec<String> {
    files.holds.others(file, window.label())
}

/// Where the settings shared by every window are kept.
fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json"))
}

/// The settings every window shares, as the last window saved them (none before the first save).
#[tauri::command]
fn settings_get(app: AppHandle) -> Option<String> {
    std::fs::read_to_string(settings_path(&app).ok()?).ok()
}

/// Saves the shared settings and tells the other windows, which apply them at once.
#[tauri::command]
fn settings_save(text: String, window: tauri::Window, app: AppHandle) -> Result<(), String> {
    let path = settings_path(&app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    let _ = app.emit("settings-changed", serde_json::json!({ "from": window.label(), "text": text }));
    Ok(())
}

/// A window's place on the screen, in logical pixels.
#[derive(Serialize, Deserialize, Clone, Copy)]
struct Frame {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Opens a window (`win-2`, …) showing what `handoff` carries (a tab moved from another window, or a saved window),
/// and returns its name. The new window asks for the handoff once it has started (`window_take`).
#[tauri::command]
async fn window_new(
    handoff: Option<serde_json::Value>,
    frame: Option<Frame>,
    hold: Option<Vec<u32>>,
    app: AppHandle,
    files: State<'_, Files>,
) -> Result<String, String> {
    let label = format!("win-{}", files.next_window.fetch_add(1, Ordering::Relaxed));
    // held from the start, so the window that hands a live source over can let go of it at once
    for id in hold.unwrap_or_default() {
        files.holds.add(id, &label);
    }
    if let Some(h) = handoff {
        files.handoffs.lock().unwrap().insert(label.clone(), h);
    }
    let mut b = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App("index.html".into()))
        .title("TelemetryBench")
        .inner_size(1200.0, 760.0)
        .min_inner_size(480.0, 320.0);
    if let Some(f) = frame {
        b = b.inner_size(f.w, f.h).position(f.x, f.y);
    }
    b.build().map_err(|e| e.to_string())?;
    Ok(label)
}

/// What this window was opened with, once.
#[tauri::command]
fn window_take(window: tauri::Window, files: State<'_, Files>) -> Option<serde_json::Value> {
    files.handoffs.lock().unwrap().remove(window.label())
}

/// Where this window is.
#[tauri::command]
fn window_frame(window: tauri::Window) -> Option<Frame> {
    let scale = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(Frame { x: pos.x, y: pos.y, w: size.width, h: size.height })
}

/// Puts this window back where it was.
#[tauri::command]
fn window_place(frame: Frame, window: tauri::Window) {
    let _ = window.set_size(tauri::LogicalSize::new(frame.w, frame.h));
    let _ = window.set_position(tauri::LogicalPosition::new(frame.x, frame.y));
}

/// A window closed: it lets go of everything it held.
fn release_window(label: &str, files: &Files) {
    for id in files.holds.of(label) {
        if files.holds.release(id, label) {
            close_now(id, files);
        }
    }
}

fn close_now(file: u32, files: &Files) {
    if let Some(g) = files.groups.lock().unwrap().remove(&file) {
        g.cancel();
    }
    files.bases.lock().unwrap().remove(&file);
    if let Some(o) = files.open.lock().unwrap().remove(&file) {
        o.cancel();
    }
    if let Some(l) = files.live.lock().unwrap().remove(&file) {
        l.closed.store(true, Ordering::Release);
        if let Some(s) = l.source.lock().unwrap().take() {
            *l.last.lock().unwrap() = s.stop();
        }
        // a session that isn't recorded goes with its last source
        if !l.keep && Arc::strong_count(&l.recording) == 1 {
            let dir = l.session.clone();
            let live = l.follow.read().unwrap().clone();
            std::thread::spawn(move || {
                drop(live);
                let _ = std::fs::remove_dir_all(dir);
            });
        }
    }
}

/// How often a live source's file is read again (new samples folded into its pyramid).
const LIVE_REFRESH: Duration = Duration::from_millis(50);

/// How a live source is connected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveArgs {
    endpoint: String,
    descriptor: Option<String>,
    /// What the Sources panel and the tab call it (the endpoint when empty).
    name: Option<String>,
    /// Keep the recording (the default). Off: it lives in the app cache and is removed when the source closes.
    record: Option<bool>,
    /// Where recordings are kept (the setting; empty is the default folder).
    dir: Option<String>,
    /// Another live file whose session this source joins, as its next source.
    add_to: Option<u32>,
}

/// Connects a live source (`tcp://`, `udp://`, `serial://`, `file://` or `replay://`; sockets need the frames'
/// descriptor, a TOML file) into a new session in the Recordings folder (or the one of `addTo`), and returns it opened
/// like a file once its first samples are recorded.
#[tauri::command]
async fn live_connect(args: LiveArgs, window: tauri::Window, files: State<'_, Files>) -> Result<Opened, String> {
    let store = files.store.clone();
    let shared = match args.add_to {
        Some(id) => {
            let l = files.live.lock().unwrap().get(&id).cloned().ok_or("that session is not open")?;
            Some((l.recording.clone(), l.keep))
        }
        None => None,
    };
    let entry = spawn_blocking(move || -> Result<LiveEntry, String> {
        let endpoint: tlmbench_live::Endpoint = args.endpoint.parse()?;
        let desc = match args.descriptor.filter(|d| !d.is_empty()) {
            Some(p) => {
                let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
                Some(tlmbench_decode_descriptor(&text).map_err(|e| format!("{p}: {e}"))?)
            }
            None => None,
        };
        let keep = shared.as_ref().map_or(args.record.unwrap_or(true), |s| s.1);
        let recording = match shared {
            Some((r, _)) => r,
            None => {
                let root = match (keep, args.dir.filter(|d| !d.trim().is_empty())) {
                    (true, Some(d)) => std::path::PathBuf::from(d),
                    (true, None) => tlmbench_live::recordings_dir(),
                    (false, _) => store.root().join("live-sessions"),
                };
                Arc::new(Mutex::new(tlmbench_live::Session::create(&root).map_err(|e| e.to_string())?))
            }
        };
        let (source, index, session) = {
            let mut session = recording.lock().unwrap();
            let source =
                session.add_source(endpoint.clone(), desc.as_ref(), &Default::default()).map_err(|e| e.to_string())?;
            (source, session.sources() - 1, session.dir().to_owned())
        };
        // the recording opens once it holds a sample
        let started = Instant::now();
        while source.records_written() == 0 {
            let h = source.health();
            if let Some(e) = h.error {
                return Err(e);
            }
            if h.ended || started.elapsed() > Duration::from_secs(15) {
                return Err(format!("no data from {endpoint} yet"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let files_now = source.files();
        let live = store.open_live(files_now.last().expect("a record was written")).map_err(|e| e.to_string())?;
        let _ = recording.lock().unwrap().update(index, &source);
        let name = args.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| endpoint.to_string());
        Ok(LiveEntry {
            last: Mutex::new(source.health()),
            source: Mutex::new(Some(source)),
            follow: RwLock::new(Arc::new(live)),
            derived: RwLock::new(None),
            part: AtomicUsize::new(files_now.len()),
            session,
            name,
            recording,
            keep,
            closed: AtomicBool::new(false),
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    let entry = Arc::new(entry);
    let id = files.next.fetch_add(1, Ordering::Relaxed) + 1;
    files.holds.add(id, window.label());
    files.live.lock().unwrap().insert(id, entry.clone());
    // follows the file as it grows (and a tailed file into its next part), until the tab closes
    let store = files.store.clone();
    let follower = entry.clone();
    std::thread::spawn(move || {
        while !follower.closed.load(Ordering::Acquire) {
            let parts = follower.source.lock().unwrap().as_ref().map(|s| s.files());
            if let Some(parts) = parts
                && parts.len() > follower.part.load(Ordering::Relaxed)
                && let Some(Ok(next)) = parts.last().map(|p| store.open_live(p))
            {
                // the trigger carries over to the new part
                let status = follower.follow.read().unwrap().trigger_status();
                *follower.follow.write().unwrap() = Arc::new(next);
                // derived channels followed the old part
                *follower.derived.write().unwrap() = None;
                follower.part.store(parts.len(), Ordering::Relaxed);
                if let (true, Some(spec)) = (status.armed, status.spec) {
                    let recipes = tlmbench_store::Library::new(store.root()).list();
                    let _ = follower.follow.read().unwrap().arm_trigger(spec, &recipes);
                }
            }
            let derived = follower.derived.read().unwrap().clone();
            match derived {
                Some(d) => drop(d.refresh()),
                None => drop(follower.follow.read().unwrap().clone().refresh()),
            }
            std::thread::sleep(LIVE_REFRESH);
        }
    });
    let o = entry.opened();
    let info = o.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    Ok(Opened { id, info, range, status: Status::Built, derived: Vec::new(), skipped: Vec::new(), missing: Vec::new() })
}

fn tlmbench_decode_descriptor(text: &str) -> Result<tlmbench_decode::descriptor::Descriptor, String> {
    tlmbench_decode::descriptor::Descriptor::parse(text).map_err(|e| e.to_string())
}

/// A live source now: its recording's extent (as of the last refresh), which part it is in, and its health.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveState {
    range: Option<(String, String)>,
    samples: usize,
    part: usize,
    info: RecordingInfo,
    session: String,
    name: String,
    /// Whether the session is being kept.
    recording: bool,
    /// The source was stopped (by hand): its recording is complete.
    stopped: bool,
    health: tlmbench_live::Health,
    trigger: tlmbench_store::trigger::Status,
}

#[tauri::command]
fn live_state(file: u32, files: State<'_, Files>) -> Result<LiveState, String> {
    let l = files.live.lock().unwrap().get(&file).cloned().ok_or("that source is not open")?;
    let o = l.opened();
    let info = o.recording().info().clone();
    // read apart: the guard would otherwise live to the end of the struct, while `health` locks the same mutex
    let stopped = l.source.lock().unwrap().is_none();
    Ok(LiveState {
        range: info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string())),
        samples: info.samples,
        part: l.part.load(Ordering::Relaxed),
        info,
        session: l.session.display().to_string(),
        name: l.name.clone(),
        recording: l.keep,
        stopped,
        health: l.health(),
        trigger: l.follow.read().unwrap().trigger_status(),
    })
}

/// Pauses or resumes a live source: a replay stops in time; a device keeps sending and the pause leaves a gap.
#[tauri::command]
fn live_pause(file: u32, paused: bool, files: State<'_, Files>) -> Result<(), String> {
    let l = files.live.lock().unwrap().get(&file).cloned().ok_or("that source is not open")?;
    if let Some(s) = l.source.lock().unwrap().as_ref() {
        s.set_paused(paused);
    }
    Ok(())
}

/// Arms a trigger on a live source (`None` stops it); the status is in [`live_state`].
#[tauri::command]
fn live_trigger(
    file: u32,
    spec: Option<tlmbench_store::trigger::Spec>,
    files: State<'_, Files>,
) -> Result<tlmbench_store::trigger::Status, String> {
    let l = files.live.lock().unwrap().get(&file).cloned().ok_or("that source is not open")?;
    let live = l.follow.read().unwrap().clone();
    match spec {
        Some(spec) => {
            let recipes = tlmbench_store::Library::new(files.store.root()).list();
            live.arm_trigger(spec, &recipes)?;
        }
        None => live.disarm_trigger(),
    }
    Ok(live.trigger_status())
}

/// What a live source would record, before it connects: a descriptor's frames, or a file's channels.
#[derive(Serialize)]
struct LivePreview {
    /// How the frames are laid out (`16 bytes a frame, 1000 Hz`), or the file's format.
    detail: String,
    channels: Vec<(String, Option<String>)>,
}

#[tauri::command]
async fn live_preview(endpoint: String, descriptor: Option<String>) -> Result<LivePreview, String> {
    spawn_blocking(move || -> Result<LivePreview, String> {
        use tlmbench_decode::descriptor::TimeSpec;
        let ep: tlmbench_live::Endpoint = endpoint.parse().unwrap_or(tlmbench_live::Endpoint::Tcp(String::new()));
        match (&ep, descriptor.filter(|d| !d.is_empty())) {
            (tlmbench_live::Endpoint::Replay(r), _) => preview_file(&r.path),
            (tlmbench_live::Endpoint::File(p), _) => preview_file(p),
            (_, Some(p)) => {
                let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
                let d = tlmbench_decode_descriptor(&text).map_err(|e| format!("{p}: {e}"))?;
                d.validate().map_err(|e| format!("{p}: {e}"))?;
                let (time, time_field) = match &d.time {
                    TimeSpec::Rate { rate_hz, .. } => (format!("{rate_hz} Hz"), None),
                    TimeSpec::Field { field, .. } => (format!("time from {field}"), Some(field.clone())),
                    TimeSpec::Counter { field, clock_hz, .. } => {
                        (format!("{field} counting at {clock_hz} Hz"), Some(field.clone()))
                    }
                    TimeSpec::Index => ("numbered".to_string(), None),
                };
                Ok(LivePreview {
                    detail: format!("{} bytes a frame, {time}", d.record_size),
                    channels: d
                        .fields
                        .iter()
                        .filter(|f| Some(&f.name) != time_field.as_ref() && Some(&f.name) != d.sequence.as_ref())
                        .map(|f| (f.name.clone(), f.unit.clone()))
                        .collect(),
                })
            }
            _ => Err("Choose the frames' descriptor to see how they decode".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn preview_file(path: &Path) -> Result<LivePreview, String> {
    let opts = tlmbench_decode::OpenOptions::default();
    let rec = tlmbench_decode::open(path, &opts).map_err(|e| e.to_string())?;
    let info = rec.info();
    Ok(LivePreview {
        detail: format!("{}, {} samples", info.format, info.samples),
        channels: info.channels.iter().map(|c| (c.name.clone(), c.unit.clone())).collect(),
    })
}

/// Stops reading a live source; its recording stays open (and complete) in its tab.
#[tauri::command]
fn live_stop(file: u32, files: State<'_, Files>) -> Result<tlmbench_live::Health, String> {
    let l = files.live.lock().unwrap().get(&file).cloned().ok_or("that source is not open")?;
    let h = match l.source.lock().unwrap().take() {
        Some(s) => s.stop(),
        None => return Ok(l.last.lock().unwrap().clone()),
    };
    *l.last.lock().unwrap() = h.clone();
    Ok(h)
}

/// Draw every sample at most draws this many per channel (about 80 MB of reply).
const RAW_MAX: u32 = 10_000_000;

/// One pane (or the overview) of a view batch.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewEntry {
    /// Echoed in the reply (the UI's pane number, or the overview's id).
    id: u32,
    channels: Vec<u32>,
    /// Per channel: a time jump over this many median intervals breaks its line (missing: the default, 4).
    #[serde(default)]
    gap_factors: Vec<f64>,
    /// Decimal strings: a JSON number can't hold i64 nanoseconds exactly.
    t0: String,
    t1: String,
    width_px: u32,
    /// Draw every sample: up to this many in view come back raw (0: only up to four per pixel column).
    #[serde(default)]
    raw_max: u32,
    /// Connect lines across gaps (dashed) instead of breaking them (Settings → Gaps in lines).
    #[serde(default)]
    connect_gaps: bool,
}

/// Every pane of a tab and the overview, each reduced over its own span and width under one generation, as a
/// batch of the binary wire format (`wire::encode_batch`), so the stack redraws together. `precision` is "coarse"
/// (from the pyramid alone, drawn first on a cold file) or "exact".
#[tauri::command]
async fn view(
    file: u32,
    generation: u32,
    precision: Option<String>,
    entries: Vec<ViewEntry>,
    files: State<'_, Files>,
) -> Result<Response, String> {
    let o = files.get(file)?;
    let g = files.group(file);
    let precision = if precision.as_deref() == Some("coarse") { Precision::Coarse } else { Precision::Exact };
    let n = g.as_ref().map_or_else(|| o.recording().info().channels.len(), |g| g.info().channels.len());
    let mut ids = Vec::with_capacity(entries.len());
    let mut requests = Vec::with_capacity(entries.len());
    for e in entries {
        let vp = Viewport {
            t0: parse_time(&e.t0)?,
            t1: parse_time(&e.t1)?,
            width_px: e.width_px.clamp(1, 16_384),
            raw_max: e.raw_max.min(RAW_MAX),
        };
        let (channels, gap_factors) = e
            .channels
            .iter()
            .enumerate()
            .filter(|&(_, &c)| (c as usize) < n)
            .map(|(i, &c)| (c as usize, e.gap_factors.get(i).copied().unwrap_or(DEFAULT_FACTOR)))
            .unzip();
        ids.push(e.id);
        requests.push(Request { channels, gap_factors, vp, connect_gaps: e.connect_gaps });
    }
    let bytes = spawn_blocking(move || {
        let started = Instant::now();
        let batch = match &g {
            Some(g) => g.view_batch(&requests, precision),
            None => o.view_batch(&requests, precision),
        };
        // channels and panes reduce together: each trace reports the whole call's time
        let us = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
        let parts: Vec<(u32, Vec<u8>)> = ids
            .iter()
            .zip(&requests)
            .zip(&batch)
            .map(|((&id, r), views)| {
                let built_until = views.first().and_then(|v| v.built_until);
                let outs: Vec<_> = views
                    .iter()
                    .map(|v| wire::Out {
                        channel: v.channel as u32,
                        trace: &v.trace,
                        reduce_us: us,
                        coarse: v.coarse,
                        from_pyramid: v.from_pyramid,
                        dashed: &v.dashed,
                    })
                    .collect();
                (id, wire::encode(generation, r.vp, built_until, &outs))
            })
            .collect();
        wire::encode_batch(generation, &parts)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

/// A channel's sample nearest a time.
#[derive(Serialize)]
struct Nearest {
    channel: u32,
    index: u64,
    t: String,
    /// None for a missing sample (NaN).
    value: Option<f64>,
}

/// Each channel's sample nearest `t`, for hover readouts and cursors: always a real sample.
#[tauri::command]
async fn values_at(file: u32, channels: Vec<u32>, t: String, files: State<'_, Files>) -> Result<Vec<Nearest>, String> {
    let o = files.get(file)?;
    let g = files.group(file);
    let t = parse_time(&t)?;
    spawn_blocking(move || {
        let n = g.as_ref().map_or_else(|| o.recording().info().channels.len(), |g| g.info().channels.len());
        let channels: Vec<u32> = channels.into_iter().filter(|&c| (c as usize) < n).collect();
        let chans: Vec<usize> = channels.iter().map(|&c| c as usize).collect();
        if let Some(g) = &g {
            return channels
                .iter()
                .zip(g.readings(&chans, t))
                .filter_map(|(&c, r)| {
                    r.map(|r| Nearest {
                        channel: c,
                        index: r.index as u64,
                        t: r.t.to_string(),
                        value: (!r.value.is_nan()).then_some(r.value),
                    })
                })
                .collect();
        }
        let Some((i, ts, values)) = o.values_at(&chans, t) else { return Vec::new() };
        channels
            .iter()
            .zip(values)
            .map(|(&c, v)| Nearest {
                channel: c,
                index: i as u64,
                t: ts.to_string(),
                value: (!v.is_nan()).then_some(v),
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Rows a `rows` call returns at most.
const MAX_ROWS: usize = 10_000;

/// The raw samples in `[t0, t1)`, rows `offset..offset + limit` of them, for a table pane. Little-endian: `TLMR`,
/// u32 channels, u64 the first row's sample index, u64 rows in the span, u32 rows returned, u32 0; then each row's
/// time (i64 ns), then each channel's values for those rows (f64 after scale and offset, NaN where missing).
#[tauri::command]
async fn rows(
    file: u32,
    channels: Vec<u32>,
    span: Span,
    offset: u64,
    limit: u32,
    query: Option<u32>,
    files: State<'_, Files>,
) -> Result<Response, String> {
    let o = files.get(file)?;
    let g = files.group(file);
    if let Some(token) = query {
        let q = files.queries.lock().unwrap().get(&token).cloned().ok_or("that query is gone")?;
        let bytes = spawn_blocking(move || {
            let channels: Vec<usize> = channels.into_iter().map(|c| c as usize).collect();
            encode_query_rows(o.recording(), g.as_deref(), &channels, &q, offset, limit)
        })
        .await
        .map_err(|e| e.to_string())?;
        return Ok(Response::new(bytes));
    }
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    let bytes = spawn_blocking(move || {
        let channels: Vec<usize> = channels.into_iter().map(|c| c as usize).collect();
        encode_rows(o.recording(), g.as_deref(), &channels, t0, t1, offset, limit)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

/// A sorted and filtered list of rows (see [`table_query`]).
struct RowQuery {
    /// Index of the span's first sample; `ids` are offsets from it.
    first: usize,
    ids: Vec<u32>,
    /// The filter's matches as runs of time `(first, last)` ns, those closer than 0.25 s joined; empty without a filter.
    hits: Vec<(i64, i64)>,
}

/// One column of a table query: `t` (the time) or `c<channel>`.
#[derive(Deserialize)]
struct ColSpec {
    col: String,
    /// The filter text (filters) or whether the sort is descending (sorts).
    #[serde(default)]
    expr: String,
    #[serde(default)]
    desc: bool,
}

/// Sorting and filtering read the first file's samples: a column of another file in the tab can't be used for them
/// (it shows, held between its samples, but has no row of its own).
fn own_col(o: &StoreOpened, c: Col) -> Result<Col, String> {
    match c {
        Col::Channel(n) if n >= o.recording().info().channels.len() => {
            Err("Sort and filter work on the columns of file 1; other files' columns show beside them".into())
        }
        c => Ok(c),
    }
}

fn col_of(s: &str) -> Result<Col, String> {
    match s {
        "t" => Ok(Col::Time),
        _ => s.strip_prefix('c').and_then(|n| n.parse().ok()).map(Col::Channel).ok_or_else(|| format!("column {s:?}")),
    }
}

/// What a table query found.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryDone {
    /// Rows left after the filters.
    matched: u64,
    /// Rows in the span.
    total: u64,
    /// False when it was cancelled (nothing is kept then).
    done: bool,
    /// Matches to step through (runs closer than 0.25 s count as one); 0 without a filter.
    hits: u64,
}

/// Sorts and filters the rows with times in `[t0, t1)` over their whole columns (`token` names the result, which
/// `rows` then pages through; a new query with a token replaces the table's last one via `drop_token`). Progress goes
/// out as `table-progress` events `{ token, done }` (0..1); `table_cancel` stops it.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn table_query(
    file: u32,
    span: Span,
    filters: Vec<ColSpec>,
    sort: Vec<ColSpec>,
    token: u32,
    drop_token: Option<u32>,
    app: AppHandle,
    files: State<'_, Files>,
) -> Result<QueryDone, String> {
    let o = files.get(file)?;
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    let mut fs = Vec::new();
    for f in &filters {
        if let Some(p) = Filter::parse(&f.expr)? {
            fs.push((own_col(&o, col_of(&f.col)?)?, p));
        }
    }
    let sort = sort
        .iter()
        .map(|k| Ok(SortKey { col: own_col(&o, col_of(&k.col)?)?, descending: k.desc }))
        .collect::<Result<Vec<_>, String>>()?;
    let stop = Arc::new(AtomicBool::new(false));
    files.cancels.lock().unwrap().insert(token, stop.clone());
    if let Some(d) = drop_token {
        files.queries.lock().unwrap().remove(&d);
    }
    let flag = stop.clone();
    let got = spawn_blocking(move || {
        let last = Mutex::new(Instant::now() - Duration::from_secs(1));
        let report = move |done: f64| {
            let mut last = last.lock().unwrap();
            if done >= 1.0 || last.elapsed() > Duration::from_millis(100) {
                *last = Instant::now();
                let _ = app.emit("table-progress", serde_json::json!({ "token": token, "done": done }));
            }
            !flag.load(Ordering::Relaxed)
        };
        let total = o.recording().lower_bound(t1).saturating_sub(o.recording().lower_bound(t0)) as u64;
        let filtered = !fs.is_empty();
        o.select_rows((t0, t1), fs, sort, &report).map(|(first, rows)| {
            let hits = if filtered { match_runs(o.recording(), first, &rows.ids) } else { Vec::new() };
            (total, first, rows, hits)
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    files.cancels.lock().unwrap().remove(&token);
    match got {
        None => Ok(QueryDone { matched: 0, total: 0, done: false, hits: 0 }),
        Some((total, first, rows, hits)) => {
            let matched = rows.ids.len() as u64;
            let n = hits.len() as u64;
            files.queries.lock().unwrap().insert(token, Arc::new(RowQuery { first, ids: rows.ids, hits }));
            Ok(QueryDone { matched, total, done: true, hits: n })
        }
    }
}

/// The matches of a filter as runs of time: `ids` (offsets from `first`, in any order) sorted by row, then rows
/// closer than 0.25 s joined, as event rules join theirs.
fn match_runs(rec: &dyn tlmbench_core::Recording, first: usize, ids: &[u32]) -> Vec<(i64, i64)> {
    let mut asc;
    let ids = if ids.is_sorted() {
        ids
    } else {
        asc = ids.to_vec();
        asc.sort_unstable();
        &asc
    };
    let times: Vec<i64> = ids.iter().map(|&i| rec.time_at(first + i as usize)).collect();
    table::runs(&times, 250_000_000)
}

/// The `k`th match of a table query's filter, for stepping with ▲ ▼: its run of time (ns as text) and the place of
/// its first row in the query's list.
#[derive(Serialize)]
struct Hit {
    t0: String,
    t1: String,
    pos: u64,
}

#[tauri::command]
fn table_hit(file: u32, token: u32, k: u64, files: State<'_, Files>) -> Result<Option<Hit>, String> {
    let o = files.get(file)?;
    let Some(q) = files.queries.lock().unwrap().get(&token).cloned() else { return Ok(None) };
    let Some(&(t0, t1)) = q.hits.get(k as usize) else { return Ok(None) };
    let row = (o.recording().lower_bound(t0).saturating_sub(q.first)) as u32;
    // the list is in row order unless sorted; either way the first row of the run is the first id at or after `row`
    let pos = if q.ids.is_sorted() {
        q.ids.partition_point(|&i| i < row)
    } else {
        let row = q.ids.iter().copied().filter(|&i| i >= row).min().unwrap_or(row);
        q.ids.iter().position(|&i| i == row).unwrap_or(0)
    };
    Ok(Some(Hit { t0: t0.to_string(), t1: t1.to_string(), pos: pos as u64 }))
}

const MARKS_MAX: usize = 4000;

/// What the charts mark over a span: where a table's filter matches and where Apply to charts leaves rows out, as
/// runs of time (ns as text).
#[derive(Serialize)]
struct Marks {
    hits: Vec<(String, String)>,
    excluded: Vec<(String, String)>,
}

/// The runs (sorted, not overlapping) that touch `[t0, t1)`, with those closer than `min_gap` joined, and joined
/// further until at most `cap` are left.
fn runs_in(runs: &[(i64, i64)], t0: i64, t1: i64, min_gap: i64, cap: usize) -> Vec<(i64, i64)> {
    let a = runs.partition_point(|r| r.1 < t0);
    let b = a + runs[a..].partition_point(|r| r.0 < t1);
    let mut gap = min_gap.max(0);
    loop {
        let mut out: Vec<(i64, i64)> = Vec::new();
        for &(x, y) in &runs[a..b] {
            match out.last_mut() {
                Some(last) if x - last.1 <= gap => last.1 = y,
                _ => out.push((x, y)),
            }
        }
        if out.len() <= cap {
            return out;
        }
        gap = (gap * 2).max(1);
    }
}

/// The marks for a span drawn `width_px` wide: the filter matches of a query (`token`, none without a filter) and
/// the rows any table's Apply to charts leaves out. Runs under a pixel apart are joined, and there are at most 4000
/// of each.
#[tauri::command]
fn table_marks(
    file: u32,
    token: Option<u32>,
    t0: String,
    t1: String,
    width_px: u32,
    files: State<'_, Files>,
) -> Result<Marks, String> {
    let o = files.get(file)?;
    let (t0, t1) = (parse_time(&t0)?, parse_time(&t1)?);
    let px = ((t1 as i128 - t0 as i128) / i128::from(width_px.max(1))).clamp(1, i64::MAX as i128) as i64;
    let text = |v: Vec<(i64, i64)>| v.into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let hits = token
        .and_then(|t| files.queries.lock().unwrap().get(&t).cloned())
        .map(|q| runs_in(&q.hits, t0, t1, px, MARKS_MAX))
        .unwrap_or_default();
    let rec = o.recording();
    let mask = o.mask();
    let timed: Vec<(i64, i64)> = mask.runs().iter().map(|r| (rec.time_at(r.start), rec.time_at(r.end - 1))).collect();
    Ok(Marks { hits: text(hits), excluded: text(runs_in(&timed, t0, t1, px, MARKS_MAX)) })
}

/// Apply to charts: leaves the rows the filters don't pass out of every chart, statistic and value of the file
/// (`owner` names the table; a new call replaces its last). Always over the whole file. `table_unmask` takes it back.
#[tauri::command]
async fn table_mask(
    file: u32,
    owner: u32,
    filters: Vec<ColSpec>,
    token: u32,
    app: AppHandle,
    files: State<'_, Files>,
) -> Result<QueryDone, String> {
    let o = files.get(file)?;
    let mut fs = Vec::new();
    for f in &filters {
        if let Some(p) = Filter::parse(&f.expr)? {
            fs.push((own_col(&o, col_of(&f.col)?)?, p));
        }
    }
    if fs.is_empty() {
        o.set_mask(owner, None);
        return Ok(QueryDone { matched: 0, total: 0, done: true, hits: 0 });
    }
    let stop = Arc::new(AtomicBool::new(false));
    files.cancels.lock().unwrap().insert(token, stop.clone());
    let info = o.recording().info().clone();
    let (t0, t1) = (info.t_first.unwrap_or(0), info.t_last.map_or(1, |t| t + 1));
    let o2 = o.clone();
    let got = spawn_blocking(move || {
        let report = move |done: f64| {
            let _ = app.emit("table-progress", serde_json::json!({ "token": token, "done": done }));
            !stop.load(Ordering::Relaxed)
        };
        o2.filter_mask((t0, t1), fs, &report)
    })
    .await
    .map_err(|e| e.to_string())?;
    files.cancels.lock().unwrap().remove(&token);
    match got {
        None => Ok(QueryDone { matched: 0, total: 0, done: false, hits: 0 }),
        Some(mask) => {
            let total = info.samples as u64;
            let excluded = mask.excluded() as u64;
            o.set_mask(owner, Some(mask));
            Ok(QueryDone { matched: total - excluded, total, done: true, hits: 0 })
        }
    }
}

/// Takes back what `table_mask` left out.
#[tauri::command]
fn table_unmask(file: u32, owner: u32, files: State<'_, Files>) -> Result<(), String> {
    files.get(file)?.set_mask(owner, None);
    Ok(())
}

/// Stops a running `table_query`.
#[tauri::command]
fn table_cancel(token: u32, files: State<'_, Files>) {
    if let Some(c) = files.cancels.lock().unwrap().get(&token) {
        c.store(true, Ordering::Relaxed);
    }
}

/// Whether a filter's text is understood: the problem, or nothing.
#[tauri::command]
fn filter_check(expr: String) -> Option<String> {
    Filter::parse(&expr).err()
}

/// A `rows` reply for rows `offset..offset + limit` of a query's list, flagged `1` at byte 28 with each row's
/// sample index (u64) between the times and the values. Runs of consecutive rows are read together.
fn encode_query_rows(
    rec: &dyn tlmbench_core::Recording,
    group: Option<&Group>,
    channels: &[usize],
    q: &RowQuery,
    offset: u64,
    limit: u32,
) -> Vec<u8> {
    let own = rec.info().channels.len();
    let n = group.map_or(own, |g| g.info().channels.len());
    let chans: Vec<usize> = channels.iter().copied().filter(|&c| c < n).collect();
    let start = (offset as usize).min(q.ids.len());
    let end = start + (q.ids.len() - start).min(limit as usize).min(MAX_ROWS);
    let window = &q.ids[start..end];
    let k = window.len();
    let mut times: Vec<i64> = Vec::with_capacity(k);
    let mut index: Vec<u64> = Vec::with_capacity(k);
    let mut cols: Vec<Vec<f64>> = vec![Vec::with_capacity(k); chans.len()];
    let mut at = 0;
    while at < k {
        let mut e = at + 1;
        while e < k && window[e] == window[e - 1] + 1 {
            e += 1;
        }
        let (a, b) = (q.first + window[at] as usize, q.first + window[e - 1] as usize + 1);
        times.extend(rec.read_times(a, b));
        index.extend((a..b).map(|i| i as u64));
        for (col, &c) in cols.iter_mut().zip(&chans) {
            if c < own {
                let (scale, off) = rec.scale_offset(c);
                col.extend(rec.read(c, a, b).to_f64(scale, off));
            }
        }
        at = e;
    }
    // another file's columns follow the rows' times, held between their samples
    if let Some(g) = group {
        for (col, &c) in cols.iter_mut().zip(&chans) {
            if c >= own {
                *col = g.held_column(c, &times);
            }
        }
    }
    let mut out = Vec::with_capacity(32 + 8 * k * (2 + chans.len()));
    out.extend_from_slice(b"TLMR");
    out.extend_from_slice(&(chans.len() as u32).to_le_bytes());
    out.extend_from_slice(&(q.first as u64).to_le_bytes());
    out.extend_from_slice(&(q.ids.len() as u64).to_le_bytes());
    out.extend_from_slice(&(k as u32).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    times.iter().for_each(|t| out.extend_from_slice(&t.to_le_bytes()));
    index.iter().for_each(|i| out.extend_from_slice(&i.to_le_bytes()));
    cols.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
    out
}

/// A `rows` reply (see [`rows`]); channels past the recording's are left out.
fn encode_rows(
    rec: &dyn tlmbench_core::Recording,
    group: Option<&Group>,
    channels: &[usize],
    t0: i64,
    t1: i64,
    offset: u64,
    limit: u32,
) -> Vec<u8> {
    let own = rec.info().channels.len();
    let n = group.map_or(own, |g| g.info().channels.len());
    let chans: Vec<usize> = channels.iter().copied().filter(|&c| c < n).collect();
    let a = rec.lower_bound(t0);
    let b = rec.lower_bound(t1).max(a);
    let start = (a as u64).saturating_add(offset).min(b as u64) as usize;
    let end = start + (b - start).min(limit as usize).min(MAX_ROWS);
    let k = end - start;
    let mut out = Vec::with_capacity(32 + 8 * k * (1 + chans.len()));
    out.extend_from_slice(b"TLMR");
    out.extend_from_slice(&(chans.len() as u32).to_le_bytes());
    out.extend_from_slice(&(start as u64).to_le_bytes());
    out.extend_from_slice(&((b - a) as u64).to_le_bytes());
    out.extend_from_slice(&(k as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    let times = rec.read_times(start, end);
    for t in &times {
        out.extend_from_slice(&t.to_le_bytes());
    }
    for &c in &chans {
        let values = match group {
            Some(g) if c >= own => g.held_column(c, &times),
            _ => {
                let (scale, off) = rec.scale_offset(c);
                rec.read(c, start, end).to_f64(scale, off)
            }
        };
        for v in values {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Exact statistics of a channel over a span.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelStats {
    channel: u32,
    count: u64,
    min: Option<f64>,
    min_t: Option<String>,
    max: Option<f64>,
    max_t: Option<String>,
    mean: Option<f64>,
    std: Option<f64>,
}

#[derive(Deserialize)]
struct Span {
    t0: String,
    t1: String,
}

/// What one widget reads (see [`widgets_frame`]).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WidgetAsk {
    channel: u32,
    #[serde(default)]
    minmax: bool,
    #[serde(default)]
    run: bool,
    /// Pixel columns of the view's trace (0: none).
    #[serde(default)]
    trace: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WidgetAnswer {
    value: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    run_start: Option<String>,
    /// `[x0, y0, x1, y1, ...]`: fractions of the view and values (null where the line breaks).
    trace: Vec<Option<f32>>,
}

#[derive(Serialize)]
struct WidgetsFrame {
    index: u64,
    t: String,
    widgets: Vec<WidgetAnswer>,
}

/// Every widget of a Gauges pane in one call: the value at `t` (the nearest sample), the lowest and highest value
/// from the start of `view` to `t`, and the time the current run of equal values began. None (null) for an empty
/// recording; min and max are null until the pyramid is built.
#[tauri::command]
async fn widgets_frame(
    file: u32,
    t: String,
    view: Span,
    widgets: Vec<WidgetAsk>,
    files: State<'_, Files>,
) -> Result<Option<WidgetsFrame>, String> {
    let o = files.get(file)?;
    let g = files.group(file);
    let (t, v0, v1) = (parse_time(&t)?, parse_time(&view.t0)?, parse_time(&view.t1)?);
    spawn_blocking(move || {
        let n = g.as_ref().map_or_else(|| o.recording().info().channels.len(), |g| g.info().channels.len());
        let reqs: Vec<tlmbench_store::widgets::WidgetReq> = widgets
            .iter()
            .map(|w| tlmbench_store::widgets::WidgetReq {
                channel: (w.channel as usize).min(n.saturating_sub(1)),
                minmax: w.minmax,
                run: w.run,
                trace: w.trace.min(600),
            })
            .collect();
        let frame = match &g {
            Some(g) => g.widgets_frame(t, (v0, v1), &reqs),
            None => o.widgets_frame(t, (v0, v1), &reqs),
        };
        frame.map(|f| WidgetsFrame {
            index: f.index as u64,
            t: f.t.to_string(),
            widgets: f
                .widgets
                .into_iter()
                .map(|w| WidgetAnswer {
                    value: w.value,
                    min: w.min,
                    max: w.max,
                    run_start: w.run_start.map(|t| t.to_string()),
                    trace: w.trace.iter().flat_map(|&(x, y)| [Some(x), (!y.is_nan()).then_some(y)]).collect(),
                })
                .collect(),
        })
    })
    .await
    .map_err(|e| e.to_string())
}

/// What an XY pane asks for (see [`xy_points`]).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct XyAsk {
    x: u32,
    ys: Vec<u32>,
    /// `[x0, x1, y0, y1]` of a zoomed pane.
    rect: Option<[f64; 4]>,
    grid: (u32, u32),
    max_points: Option<u32>,
}

/// Points of Y channels against an X channel over a span, reduced to a pixel grid (see `tlmbench_store::xy`).
/// Little-endian: `TLMX`, u32 series, u32 flags (bit 0 sampled, bit 1 has a range), u32 0, f64 x0, x1, y0, y1; then
/// per series u32 channel, u32 n, and n × i64 time (ns), n × f64 x, n × f64 y (NaN pairs break the line).
#[tauri::command]
async fn xy_points(file: u32, span: Span, ask: XyAsk, files: State<'_, Files>) -> Result<Response, String> {
    let o = files.get(file)?;
    let g = files.group(file);
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    let bytes = spawn_blocking(move || {
        let n = g.as_ref().map_or_else(|| o.recording().info().channels.len(), |g| g.info().channels.len()) as u32;
        let ok = |c: &u32| *c < n;
        let q = tlmbench_store::xy::XyReq {
            x: ask.x.min(n.saturating_sub(1)) as usize,
            ys: ask.ys.iter().filter(|c| ok(c)).map(|&c| c as usize).collect(),
            t0,
            t1,
            rect: ask.rect,
            grid: ask.grid,
            max_points: ask.max_points.unwrap_or(100_000) as usize,
        };
        let r = match &g {
            Some(g) => g.xy_points(&q).ok_or_else(|| "An XY plot pairs channels of one file".to_string())?,
            None => o.xy_points(&q),
        };
        let mut out = Vec::with_capacity(48 + r.series.iter().map(|s| 8 + 24 * s.x.len()).sum::<usize>());
        out.extend_from_slice(b"TLMX");
        out.extend_from_slice(&(r.series.len() as u32).to_le_bytes());
        out.extend_from_slice(&(u32::from(r.sampled) | (u32::from(r.range.is_some()) << 1)).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for v in r.range.unwrap_or([f64::NAN; 4]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for s in &r.series {
            out.extend_from_slice(&(s.channel as u32).to_le_bytes());
            out.extend_from_slice(&(s.x.len() as u32).to_le_bytes());
            s.t.iter().for_each(|t| out.extend_from_slice(&t.to_le_bytes()));
            s.x.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
            s.y.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        }
        Ok::<_, String>(out)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Response::new(bytes))
}

/// Exact stats of each channel over `[t0, t1)`; None until the pyramid is built (exact answers wait for it).
#[tauri::command]
async fn stats(
    file: u32,
    channels: Vec<u32>,
    span: Span,
    files: State<'_, Files>,
) -> Result<Option<Vec<ChannelStats>>, String> {
    let o = files.get(file)?;
    let g = files.group(file);
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    spawn_blocking(move || {
        let num = |v: f64| (!v.is_nan()).then_some(v);
        let n = g.as_ref().map_or_else(|| o.recording().info().channels.len(), |g| g.info().channels.len());
        let channels: Vec<u32> = channels.into_iter().filter(|&c| (c as usize) < n).collect();
        let chans: Vec<usize> = channels.iter().map(|&c| c as usize).collect();
        let all = match &g {
            Some(g) => g.stats(&chans, t0, t1)?,
            None => o.stats(&chans, t0, t1)?,
        };
        Some(
            channels
                .iter()
                .zip(all)
                .map(|(&c, s)| {
                    let at = |m: Option<(f64, usize)>| {
                        m.map(|(_, i)| {
                            match g.as_ref().and_then(|g| g.route(c as usize).map(|(si, _)| &g.slots()[si])) {
                                Some(slot) => slot.map().apply(slot.opened.recording().time_at(i)).to_string(),
                                None => o.recording().time_at(i).to_string(),
                            }
                        })
                    };
                    ChannelStats {
                        channel: c,
                        count: s.count,
                        min: s.min.map(|m| m.0),
                        min_t: at(s.min),
                        max: s.max.map(|m| m.0),
                        max_t: at(s.max),
                        mean: num(s.mean),
                        std: num(s.std),
                    }
                })
                .collect(),
        )
    })
    .await
    .map_err(|e| e.to_string())
}

/// A view shared in a folder (`.tlmbench/views/<file>`): its file name and its contents, or why it can't be read.
#[derive(Serialize)]
struct SharedView {
    file: String,
    view: Option<serde_json::Value>,
    error: Option<String>,
}

/// Where a folder's shared views live.
fn views_dir(folder: &str) -> std::path::PathBuf {
    Path::new(folder).join(".tlmbench").join("views")
}

/// The views shared in `folder`, by file name (none when it has no `.tlmbench/views/`). Only reads.
#[tauri::command]
async fn shared_views(folder: String) -> Result<Vec<SharedView>, String> {
    spawn_blocking(move || {
        let Ok(dir) = std::fs::read_dir(views_dir(&folder)) else { return Vec::new() };
        let mut out: Vec<SharedView> = dir
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|f| f.ends_with(".tlmview.json"))
            .map(|file| {
                let read = std::fs::read_to_string(views_dir(&folder).join(&file))
                    .map_err(|e| e.to_string())
                    .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()));
                let (view, error) = match read {
                    Ok(v) => (Some(v), None),
                    Err(e) => (None, Some(e)),
                };
                SharedView { file, view, error }
            })
            .collect();
        out.sort_by(|a, b| a.file.cmp(&b.file));
        out
    })
    .await
    .map_err(|e| e.to_string())
}

/// Who shares a view: the account name.
fn user_name() -> Option<String> {
    ["USER", "USERNAME"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

/// Share…: writes `view` (with `by` set to the account name) to `folder/.tlmbench/views/<file>`, the one write into a
/// folder besides exports, and only when asked. `file` is a bare `*.tlmview.json` name.
#[tauri::command]
async fn share_view(folder: String, file: String, mut view: serde_json::Value) -> Result<String, String> {
    let bare = Path::new(&file).file_name().is_some_and(|n| n == file.as_str());
    if !bare || !file.ends_with(".tlmview.json") {
        return Err(format!("{file:?} isn't a view file name"));
    }
    if let (Some(o), Some(by)) = (view.as_object_mut(), user_name()) {
        o.insert("by".into(), by.into());
    }
    spawn_blocking(move || {
        let dir = views_dir(&folder);
        std::fs::create_dir_all(&dir).map_err(|e| format!("can't write to {}: {e}", dir.display()))?;
        let path = dir.join(&file);
        let text = serde_json::to_string_pretty(&view).map_err(|e| e.to_string())? + "\n";
        std::fs::write(&path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        Ok(path.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------------------------------------------
// Shared configuration: the folder's `.tlmbench/` (Share…, Use the folder's, Export…)

/// The `.tlmbench/` of the folder Explorer shows.
fn tlm_dir(folder: &str) -> std::path::PathBuf {
    Path::new(folder).join(".tlmbench")
}

/// Fills in the identity of each recording the UI named by its open file's id.
fn resolve_own(own: &mut tlmbench_store::config::Own, files: &Files) {
    for r in &mut own.recordings {
        if let (true, Some(id)) = (r.identity.is_empty(), r.file)
            && let Ok(b) = files.base(id)
        {
            r.identity = b.identity().to_owned();
        }
    }
}

/// What `config_use` tells the UI about the folder it was pointed at.
#[derive(Serialize)]
struct ConfigFolder {
    /// The `.tlmbench/` read alongside the cache: the nearest one at or above the folder.
    dir: Option<String>,
    /// Its categories, widgets and icons (the app's settings own those).
    from: tlmbench_store::config::FromFolder,
    /// Names of the recipes and check sets it holds, so the lists can mark them as shared.
    recipes: Vec<String>,
    checks: Vec<String>,
    /// Files in it that don't read, with why.
    problems: Vec<tlmbench_store::config::Problem>,
}

/// Points the app at a folder's shared configuration: recipes, check sets, import profiles, decoders and events of
/// the nearest `.tlmbench/` are read besides the cache from now on (yours win a name). Only reads.
#[tauri::command]
async fn config_use(folder: Option<String>) -> Result<ConfigFolder, String> {
    spawn_blocking(move || {
        let dir =
            folder.as_deref().filter(|f| !f.is_empty()).and_then(|f| tlmbench_store::config::nearest(Path::new(f)));
        tlmbench_store::config::set_overlay(dir.iter().cloned().collect());
        tlmbench_decode::descriptor::set_config_dirs(Vec::new());
        let Some(d) = dir else {
            return ConfigFolder {
                dir: None,
                from: Default::default(),
                recipes: vec![],
                checks: vec![],
                problems: vec![],
            };
        };
        let dirs = std::slice::from_ref(&d);
        ConfigFolder {
            dir: Some(d.display().to_string()),
            from: tlmbench_store::config::from_folder(&d),
            recipes: tlmbench_store::config::shared_recipes(dirs).into_iter().map(|r| r.name).collect(),
            checks: tlmbench_store::config::shared_sets(dirs).into_iter().map(|s| s.name).collect(),
            problems: tlmbench_store::config::validate(&d),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// The Share dialog's rows: every object you have against the folder's `.tlmbench/`.
#[derive(Serialize)]
struct ConfigScan {
    rows: Vec<tlmbench_store::config::Row>,
    problems: Vec<tlmbench_store::config::Problem>,
    /// False when the folder looks read-only: the dialog offers Export instead of Share.
    writable: bool,
    /// The folder is a git repository, so Share can offer a `.gitignore` line for the recordings.
    git: bool,
}

#[tauri::command]
async fn config_scan(
    folder: String,
    mut own: tlmbench_store::config::Own,
    files: State<'_, Files>,
) -> Result<ConfigScan, String> {
    resolve_own(&mut own, &files);
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let dir = tlm_dir(&folder);
        let (theirs, problems) = tlmbench_store::config::read_folder(&dir);
        let mine = tlmbench_store::config::cache_items(&root, &own);
        let _ = tlmbench_store::config::save_mirror(&root, &own);
        ConfigScan {
            rows: tlmbench_store::config::compare(&mine, &theirs),
            problems,
            writable: tlmbench_store::config::writable(&dir),
            git: Path::new(&folder).join(".git").exists(),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Keeps the app's own settings (categories, widgets, icons, layouts, open recordings) in the cache root, where
/// `tlmbench config export` finds them.
#[tauri::command]
async fn config_mirror(mut own: tlmbench_store::config::Own, files: State<'_, Files>) -> Result<(), String> {
    resolve_own(&mut own, &files);
    let root = files.store.root().to_owned();
    spawn_blocking(move || tlmbench_store::config::save_mirror(&root, &own).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// A row key as the dialog sends it: `kind:path`.
fn wanted(keys: &[String], i: &tlmbench_store::config::Item) -> bool {
    let k = format!(
        "{}:{}",
        serde_json::to_value(i.kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default(),
        i.key()
    );
    keys.contains(&k)
}

/// Share…: writes the chosen objects to the folder's `.tlmbench/` (only what changed; events merge by id), the one
/// write into a folder besides exports, and only when asked. Returns the files written.
#[tauri::command]
async fn config_share(
    folder: String,
    mut own: tlmbench_store::config::Own,
    keys: Vec<String>,
    files: State<'_, Files>,
) -> Result<Vec<String>, String> {
    resolve_own(&mut own, &files);
    let root = files.store.root().to_owned();
    let by = user_name();
    spawn_blocking(move || {
        let mut own = own;
        for v in &mut own.views {
            if let (Some(o), Some(by)) = (v.view.as_object_mut(), &by) {
                o.entry("by").or_insert_with(|| by.clone().into());
            }
        }
        for w in &mut own.widgets {
            if w.by.is_none() {
                w.by = by.clone();
            }
        }
        let items: Vec<_> =
            tlmbench_store::config::cache_items(&root, &own).into_iter().filter(|i| wanted(&keys, i)).collect();
        let dir = tlm_dir(&folder);
        tlmbench_store::config::share(&dir, &items).map_err(|e| format!("can't write to {}: {e}", dir.display()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What Use the folder's brings back: the objects the app's settings own.
#[derive(Serialize)]
struct Pulled {
    from: tlmbench_store::config::FromFolder,
    views: Vec<tlmbench_store::config::View>,
    /// The keys taken.
    taken: Vec<String>,
}

/// Use the folder's: copies the chosen objects of the folder's `.tlmbench/` into the cache (recipes, check sets,
/// import profiles, decoders, time maps, events) and returns the ones the settings own (categories, widgets, icons,
/// layouts) for the UI to take. Reads the folder, writes only the app cache.
#[tauri::command]
async fn config_pull(
    folder: String,
    mut own: tlmbench_store::config::Own,
    keys: Vec<String>,
    files: State<'_, Files>,
) -> Result<Pulled, String> {
    resolve_own(&mut own, &files);
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let dir = tlm_dir(&folder);
        let (theirs, _) = tlmbench_store::config::read_folder(&dir);
        let chosen: Vec<_> = theirs.into_iter().filter(|i| wanted(&keys, i)).collect();
        let taken = tlmbench_store::config::pull(&root, &chosen, &own).map_err(|e| e.to_string())?;
        let mut from = tlmbench_store::config::from_folder(&dir);
        let has = |kind, key_ends: &dyn Fn(&str) -> bool| chosen.iter().any(|i| i.kind == kind && key_ends(&i.key()));
        if !has(tlmbench_store::config::Kind::Categories, &|_| true) {
            from.categories.clear();
        }
        from.widgets
            .retain(|w| has(tlmbench_store::config::Kind::Widgets, &|k| k.rsplit('/').next() == Some(w.id.as_str())));
        from.icons.retain(|i| has(tlmbench_store::config::Kind::Icons, &|k| k == format!("icons/{}.svg", i.name)));
        let views = chosen
            .iter()
            .filter(|i| i.kind == tlmbench_store::config::Kind::Views)
            .filter_map(|i| {
                let file = i.files[0].0.rsplit('/').next()?.to_owned();
                Some(tlmbench_store::config::View { file, view: serde_json::from_slice(&i.files[0].1).ok()? })
            })
            .collect();
        Ok(Pulled { from, views, taken })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Export…: everything you have, as a `.tlmbench/` folder at `out` (a read-only folder can't be shared to; the same
/// as `tlmbench config export`).
#[tauri::command]
async fn config_export(
    out: String,
    mut own: tlmbench_store::config::Own,
    files: State<'_, Files>,
) -> Result<Vec<String>, String> {
    resolve_own(&mut own, &files);
    let root = files.store.root().to_owned();
    spawn_blocking(move || tlmbench_store::config::export(&root, &own, Path::new(&out)).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Share's `.gitignore` line for recordings: adds `*.csv`-style patterns for the folder's recording files to the
/// folder's `.gitignore` (creating it), unless they are in it. Only when asked, only in a git repository.
#[tauri::command]
async fn config_gitignore(folder: String, lines: Vec<String>) -> Result<Vec<String>, String> {
    spawn_blocking(move || {
        let path = Path::new(&folder).join(".gitignore");
        if !Path::new(&folder).join(".git").exists() {
            return Err("that folder isn't a git repository".to_owned());
        }
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        let add: Vec<String> =
            lines.into_iter().filter(|l| !l.trim().is_empty() && !old.lines().any(|o| o.trim() == l.trim())).collect();
        if !add.is_empty() {
            let mut text = old;
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&add.join("\n"));
            text.push('\n');
            std::fs::write(&path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        }
        Ok(add)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A file's alignment as the UI keeps it, with the open files it is between.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimeMapIn {
    /// The folder Explorer shows (names are relative to it) and the two files' ids.
    folder: String,
    primary: u32,
    file: u32,
    method: String,
    #[serde(default)]
    seconds: f64,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    locked: bool,
    /// `[t_file, t_shared]` in nanoseconds as decimal strings.
    #[serde(default)]
    knots: Vec<[String; 2]>,
    #[serde(default)]
    pairs: Vec<[f64; 2]>,
}

/// A file's name for a time map: relative to the folder with `/`, else the path.
fn rel_name(folder: &str, path: &Path) -> String {
    match path.strip_prefix(folder) {
        Ok(r) if !folder.is_empty() => r.to_string_lossy().replace('\\', "/"),
        _ => path.to_string_lossy().into_owned(),
    }
}

/// The alignment of a file against its tab's primary, kept per pair in the app cache (`timemaps/`) in the form
/// Share writes, so the pair opens aligned the next time.
#[tauri::command]
async fn timemap_save(map: TimeMapIn, files: State<'_, Files>) -> Result<(), String> {
    let (a, b) = (files.base(map.primary)?, files.base(map.file)?);
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let knots = map
            .knots
            .iter()
            .map(|[f, s]| {
                Ok([f.parse::<i64>().map_err(|e| e.to_string())?, s.parse::<i64>().map_err(|e| e.to_string())?])
            })
            .collect::<Result<Vec<_>, String>>()?;
        let t = tlmbench_store::config::TimeMapFile {
            schema: tlmbench_store::config::SCHEMA,
            primary: rel_name(&map.folder, &a.recording().info().path),
            file: rel_name(&map.folder, &b.recording().info().path),
            primary_id: a.identity().to_owned(),
            file_id: b.identity().to_owned(),
            method: map.method,
            seconds: map.seconds,
            channel: map.channel,
            locked: map.locked,
            knots,
            pairs: map.pairs,
        };
        let dir = root.join("timemaps");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(t.file_name());
        let text = tlmbench_store::config::timemap_to_toml(&t);
        if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
            return Ok(());
        }
        std::fs::write(&path, text).map_err(|e| format!("can't write {}: {e}", path.display()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The saved alignment for a pair, as the UI keeps it.
#[derive(Serialize)]
struct SavedMap {
    method: String,
    seconds: f64,
    channel: String,
    locked: bool,
    knots: Vec<[String; 2]>,
    pairs: Vec<[f64; 2]>,
}

/// The saved alignment for two open files: yours first, then the folder's `.tlmbench/timemaps/`; matched by the
/// files' identities, else by their paths relative to the folder.
#[tauri::command]
async fn timemap_find(
    folder: String,
    primary: u32,
    file: u32,
    files: State<'_, Files>,
) -> Result<Option<SavedMap>, String> {
    let (a, b) = (files.base(primary)?, files.base(file)?);
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let (pn, fnm) = (rel_name(&folder, &a.recording().info().path), rel_name(&folder, &b.recording().info().path));
        let mut dirs = vec![root.join("timemaps")];
        dirs.extend(tlmbench_store::config::overlay().into_iter().map(|d| d.join("timemaps")));
        for d in dirs {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            let mut maps: Vec<_> = rd
                .flatten()
                .filter_map(|e| {
                    tlmbench_store::config::timemap_from_toml(&std::fs::read_to_string(e.path()).ok()?).ok()
                })
                .collect();
            maps.sort_by_key(|m| m.file_name());
            let by_id = |m: &tlmbench_store::config::TimeMapFile| {
                !m.primary_id.is_empty() && m.primary_id == a.identity() && m.file_id == b.identity()
            };
            if let Some(m) = maps.into_iter().find(|m| by_id(m) || (m.primary == pn && m.file == fnm)) {
                return Ok(Some(SavedMap {
                    method: m.method,
                    seconds: m.seconds,
                    channel: m.channel,
                    locked: m.locked,
                    knots: m.knots.iter().map(|k| [k[0].to_string(), k[1].to_string()]).collect(),
                    pairs: m.pairs,
                }));
            }
        }
        Ok(None)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What Export data… writes: the open file's samples in a time span, as the CLI's `convert` writes them.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportRequest {
    file: u32,
    /// `csv`, `parquet`, `arrow`, `hdf5` or `mat`.
    format: String,
    /// The file to write (its channel list goes beside it as `<name>.channels.csv`).
    out: String,
    /// Channel numbers in the file, in order.
    channels: Vec<u32>,
    /// The span as decimal nanosecond strings; none: from the first / to the last sample.
    from: Option<String>,
    until: Option<String>,
    /// `wide` or `long`.
    layout: String,
    /// `auto`, `seconds`, `iso` or `ns`.
    time: String,
    raw: bool,
    resample_hz: Option<f64>,
}

#[derive(Serialize)]
struct Exported {
    path: String,
    channels_file: String,
    rows: u64,
    bytes: u64,
}

/// Export data…: writes samples of an open file to `out`, chunk by chunk (flat memory). Masks and filters are views:
/// an export reads the full data.
#[tauri::command]
async fn export_data(req: ExportRequest, files: State<'_, Files>) -> Result<Exported, String> {
    use tlmbench_export::{Format, Layout, Options, TimeFormat};
    let o = files.get(req.file)?;
    spawn_blocking(move || {
        let format = match req.format.as_str() {
            "csv" => Format::Csv,
            "parquet" => Format::Parquet,
            "arrow" => Format::Arrow,
            "hdf5" => Format::Hdf5,
            "mat" => Format::Mat,
            other => return Err(format!("{other:?} isn't an export format")),
        };
        let rec = o.recording();
        let t0 = req.from.as_deref().map(parse_time).transpose()?;
        let t1 = req.until.as_deref().map(parse_time).transpose()?;
        let n = rec.info().samples;
        let (start, end) = (t0.map_or(0, |t| rec.lower_bound(t)), t1.map_or(n, |t| rec.lower_bound(t)));
        let mut opts = Options::new(format, req.channels.iter().map(|&c| c as usize).collect(), start, end);
        opts.layout = if req.layout == "long" { Layout::Long } else { Layout::Wide };
        opts.time = match req.time.as_str() {
            "seconds" => TimeFormat::Seconds,
            "iso" => TimeFormat::Iso,
            "ns" => TimeFormat::Ns,
            _ => TimeFormat::Auto,
        };
        opts.resample_hz = req.resample_hz.filter(|h| *h > 0.0);
        opts.raw = req.raw;
        opts.rate_hz = match rec.info().time {
            tlmbench_core::TimeSource::Rate { hz } => Some(hz),
            _ => o.median_interval_ns().filter(|&m| m > 0).map(|m| 1e9 / m as f64),
        };
        let w = tlmbench_export::export(rec, &opts, Path::new(&req.out)).map_err(|e| e.to_string())?;
        Ok(Exported {
            path: w.path.display().to_string(),
            channels_file: w.manifest.display().to_string(),
            rows: w.rows,
            bytes: w.bytes,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Save the view as an image…: writes the bytes the page made (a PNG or SVG) to the file the person chose.
#[tauri::command]
async fn save_file(path: String, bytes: Vec<u8>) -> Result<(), String> {
    spawn_blocking(move || std::fs::write(&path, bytes).map_err(|e| format!("can't write {path}: {e}")))
        .await
        .map_err(|e| e.to_string())?
}

/// Export file…: writes a view to a `.json` file the person chose in the save dialog.
#[tauri::command]
async fn export_view(path: String, view: serde_json::Value) -> Result<(), String> {
    if !path.ends_with(".json") {
        return Err(format!("{path} isn't a .json file"));
    }
    spawn_blocking(move || {
        let text = serde_json::to_string_pretty(&view).map_err(|e| e.to_string())? + "\n";
        std::fs::write(&path, text).map_err(|e| format!("can't write {path}: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A file or folder in a listed folder.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FolderEntry {
    path: String,
    name: String,
    dir: bool,
    bytes: u64,
    /// Files: whether the app can open it (a recording format); the others are listed so Explorer can offer to show them.
    openable: bool,
    /// A binary file no descriptor claims: opening it asks for a descriptor (the form).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    needs_descriptor: bool,
    /// Folders: what they hold, recordings and the folders that lead to some.
    children: Vec<FolderEntry>,
}

/// Extensions of files that are likely a logger's binary format (a file without one counts too): these are looked
/// into, and offered the descriptor form when nothing claims them.
fn binary_ext(ext: Option<&str>) -> bool {
    matches!(ext, None | Some("bin" | "dat" | "raw" | "pkt" | "log" | "tlmbin" | "frames"))
}

/// The files under `path`, in their folders, sorted by name (recordings are `openable`: CSV, TSV, TXT, .tlm and
/// Parquet, and files a descriptor's `files` patterns name; a binary file nothing claims is `needs_descriptor`);
/// folders without any file, and hidden ones, are left out. Only reads the folder (Explorer's Open folder).
#[tauri::command]
async fn list_folder(path: String, files: State<'_, Files>) -> Result<FolderEntry, String> {
    use tlmbench_decode::descriptor::{Known, known, name_matches};
    fn walk(p: &Path, depth: u32, root: &Path) -> Option<FolderEntry> {
        let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
        let meta = std::fs::metadata(p).ok()?;
        if !meta.is_dir() {
            let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
            let mut openable = matches!(ext.as_deref(), Some("csv" | "tsv" | "txt" | "tlm" | "parquet"));
            let mut needs_descriptor = false;
            if !openable {
                let claimed: Vec<Known> = p.parent().map(|d| known(d, root)).unwrap_or_default();
                openable = claimed.iter().any(|k| k.descriptor.files.iter().any(|g| name_matches(g, &name)));
                if !openable && binary_ext(ext.as_deref()) {
                    let head = tlmbench_decode::read_head(p, 512).unwrap_or_default();
                    needs_descriptor = !head.is_empty() && tlmbench_decode::preview::looks_binary(&head);
                    openable = needs_descriptor;
                }
            }
            return Some(FolderEntry {
                path: p.display().to_string(),
                name,
                dir: false,
                bytes: meta.len(),
                openable,
                needs_descriptor,
                children: Vec::new(),
            });
        }
        let mut children: Vec<FolderEntry> = if depth < 8 {
            std::fs::read_dir(p)
                .ok()?
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter_map(|e| walk(&e.path(), depth + 1, root))
                .collect()
        } else {
            Vec::new()
        };
        children.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        (depth == 0 || !children.is_empty()).then(|| FolderEntry {
            path: p.display().to_string(),
            name,
            dir: true,
            bytes: 0,
            openable: false,
            needs_descriptor: false,
            children,
        })
    }
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        walk(Path::new(&path), 0, &root).ok_or_else(|| format!("{path} isn't a folder that can be read"))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- format descriptors: the form on a binary file nothing claims ----------

/// Whether `path` is a binary file no descriptor claims (the form is offered for it).
#[tauri::command]
async fn needs_descriptor(path: String, files: State<'_, Files>) -> Result<bool, String> {
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let p = Path::new(&path);
        let head = tlmbench_decode::read_head(p, 4096).unwrap_or_default();
        !head.is_empty()
            && tlmbench_decode::preview::looks_binary(&head)
            && tlmbench_decode::descriptor::discover(p, &root).is_none()
    })
    .await
    .map_err(|e| e.to_string())
}

/// A descriptor that could decode a file, and whether its `files` patterns name this one.
#[derive(Serialize)]
struct Listed {
    #[serde(flatten)]
    known: tlmbench_decode::descriptor::Known,
    claims: bool,
}

/// The descriptors that could decode a file: its folder's `.tlmbench/decoders/` and the app cache's `decoders/`.
#[tauri::command]
async fn descriptor_list(path: String, files: State<'_, Files>) -> Result<Vec<Listed>, String> {
    let root = files.store.root().to_owned();
    spawn_blocking(move || {
        let p = Path::new(&path);
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let known = p.parent().map(|d| tlmbench_decode::descriptor::known(d, &root)).unwrap_or_default();
        known
            .into_iter()
            .map(|k| {
                let claims = k.descriptor.files.iter().any(|g| tlmbench_decode::descriptor::name_matches(g, &name));
                Listed { known: k, claims }
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// A descriptor as the form edits it (the JSON of `Descriptor`).
fn parse_descriptor(d: serde_json::Value) -> Result<tlmbench_decode::descriptor::Descriptor, String> {
    serde_json::from_value(d).map_err(|e| e.to_string())
}

/// The first records of `path` decoded through `descriptor`, the TOML it makes, and the problems it has.
#[tauri::command]
async fn descriptor_preview(
    path: String,
    descriptor: serde_json::Value,
) -> Result<tlmbench_decode::preview::Preview, String> {
    spawn_blocking(move || {
        let head = tlmbench_decode::read_head(Path::new(&path), tlmbench_decode::preview::HEAD_BYTES)
            .map_err(|e| e.to_string())?;
        Ok(match parse_descriptor(descriptor) {
            Ok(d) => tlmbench_decode::preview::preview(&head, &d),
            Err(e) => tlmbench_decode::preview::Preview { problem: Some(e), ..Default::default() },
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A sync word and frame length that repeat in the first bytes of `path` (Find frames).
#[tauri::command]
async fn descriptor_find_frames(path: String) -> Result<Option<tlmbench_decode::descriptor::Frames>, String> {
    spawn_blocking(move || {
        let head = tlmbench_decode::read_head(Path::new(&path), tlmbench_decode::preview::HEAD_BYTES)
            .map_err(|e| e.to_string())?;
        Ok(tlmbench_decode::descriptor::find_frames(&head))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Save: writes a descriptor to the app cache's `decoders/` (`replace` is the id of the one being edited).
#[tauri::command]
async fn descriptor_save(
    descriptor: serde_json::Value,
    replace: Option<String>,
    files: State<'_, Files>,
) -> Result<tlmbench_decode::descriptor::Known, String> {
    let (root, d) = (files.store.root().to_owned(), parse_descriptor(descriptor)?);
    spawn_blocking(move || tlmbench_decode::descriptor::save(&d, replace.as_deref(), &root).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Share…: writes a descriptor to `<the file's folder>/.tlmbench/decoders/`, a write beside the data, only when the
/// person asks.
#[tauri::command]
async fn descriptor_share(path: String, descriptor: serde_json::Value) -> Result<String, String> {
    let d = parse_descriptor(descriptor)?;
    spawn_blocking(move || {
        let folder = Path::new(&path).parent().ok_or("that file has no folder")?;
        let out = tlmbench_decode::descriptor::share(&d, folder).map_err(|e| e.to_string())?;
        Ok(out.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What the status bar shows about the core: its memory, the chunk cache's share of it and budget, and the CPU time
/// it has used (the UI turns two readings into a load).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    memory: Option<u64>,
    chunks: u64,
    chunk_budget: u64,
    cpu_seconds: Option<f64>,
}

#[tauri::command]
fn usage(files: State<'_, Files>) -> Usage {
    let cache = files.store.chunk_cache();
    Usage {
        memory: tlmbench_store::sys::resident(),
        chunks: cache.bytes(),
        chunk_budget: cache.budget(),
        cpu_seconds: tlmbench_store::sys::cpu_seconds(),
    }
}

/// The cache folder (in the app's cache, never next to the data), what it holds and its cap, for Settings.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheInfo {
    path: String,
    bytes: u64,
    cap: u64,
}

#[tauri::command]
async fn cache_info(files: State<'_, Files>) -> Result<CacheInfo, String> {
    let store = &files.store;
    let bytes = store.entries().map_err(|e| e.to_string())?.iter().map(|e| e.bytes).sum();
    Ok(CacheInfo { path: store.root().display().to_string(), bytes, cap: store.limits.cap })
}

/// Settings → Clear cache: removes every entry not in use (the open files keep theirs); returns how many went.
#[tauri::command]
async fn cache_clear(files: State<'_, Files>) -> Result<usize, String> {
    files.store.clear().map(|removed| removed.len()).map_err(|e| e.to_string())
}

/// Settings → Cache → Open folder: the OS file manager shows the cache folder (reveal only; the app runs no program).
#[tauri::command]
async fn cache_reveal(files: State<'_, Files>) -> Result<(), String> {
    let root = files.store.root().to_path_buf();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(root).map_err(|e| e.to_string())
}

/// `bytes` zero bytes: the IPC transfer cost alone, for the gate report.
#[tauri::command]
fn echo(bytes: u32) -> Response {
    Response::new(vec![0; bytes.min(64 << 20) as usize])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Launch {
    file: Option<String>,
    /// "1" (Phase 1), "2" (Phase 2), "3" (Phase 3: `file` is the source's endpoint), "4" (Phase 4: `file` is the
    /// format matrix's manifest), or None.
    gate: Option<&'static str>,
    /// Gate 2: "reopen" (the pyramid is cached) or "first" (a first-ever open). Gate 3: the frames' descriptor.
    gate_mode: Option<String>,
    /// Gate 3: how long to follow the stream, in seconds.
    gate_seconds: Option<f64>,
}

#[tauri::command]
fn launch() -> Launch {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(endpoint) = env("TLMBENCH_GATE3") {
        let seconds = env("TLMBENCH_GATE3_SECONDS").and_then(|s| s.parse().ok());
        return Launch {
            file: Some(endpoint),
            gate: Some("3"),
            gate_mode: env("TLMBENCH_GATE3_DESCRIPTOR"),
            gate_seconds: seconds,
        };
    }
    if let Some(manifest) = env("TLMBENCH_GATE4") {
        return Launch { file: Some(manifest), gate: Some("4"), gate_mode: None, gate_seconds: None };
    }
    if let Some(file) = env("TLMBENCH_GATE2") {
        return Launch { file: Some(file), gate: Some("2"), gate_mode: env("TLMBENCH_GATE2_MODE"), gate_seconds: None };
    }
    match env("TLMBENCH_GATE") {
        Some(file) => Launch { file: Some(file), gate: Some("1"), gate_mode: None, gate_seconds: None },
        None => Launch {
            file: std::env::args().nth(1).filter(|a| !a.starts_with('-')),
            gate: None,
            gate_mode: None,
            gate_seconds: None,
        },
    }
}

/// Gate 4's manifest: the files to open and what each must read as (written by `ci/gate4.py`).
#[tauri::command]
fn gate_manifest(path: String) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// Prints the gate report and quits: exit code 0 when it passed, 1 when not.
#[tauri::command]
fn gate_report(report: serde_json::Value, app: AppHandle) {
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    let pass = report.get("pass").and_then(serde_json::Value::as_bool).unwrap_or(false);
    app.exit(if pass { 0 } else { 1 });
}

/// The page code widgets run in, served as `tlmwidget://localhost/host.html` (`http://tlmwidget.localhost/host.html` on
/// Windows). It has a policy of its own: the app's (`default-src 'self'`) would stop a widget's inline script and
/// `new Function`. The pane embeds it with `sandbox="allow-scripts"`: no same-origin access, and `default-src 'none'`
/// leaves it no network, frames or images; values arrive by postMessage.
const WIDGET_HOST: &str = include_str!("widget-host.html");

fn widget_host(
    _: tauri::UriSchemeContext<'_, tauri::Wry>,
    _: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .header(
            "Content-Security-Policy",
            "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline' 'unsafe-eval'",
        )
        .header("Cache-Control", "no-store")
        .body(WIDGET_HOST.as_bytes().to_vec())
        .expect("a fixed response")
}

fn main() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("tlmwidget", widget_host)
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|w, ev| {
            if let tauri::WindowEvent::Destroyed = ev
                && let Some(files) = w.try_state::<Files>()
            {
                release_window(w.label(), &files);
            }
        })
        .manage(Files {
            store: Store::new(None),
            next: AtomicU32::new(0),
            holds: Holds::default(),
            handoffs: Mutex::default(),
            next_window: AtomicU32::new(2),
            open: Mutex::default(),
            bases: Mutex::default(),
            live: Mutex::default(),
            groups: Mutex::default(),
            queries: Mutex::default(),
            cancels: Mutex::default(),
        })
        .invoke_handler(tauri::generate_handler![
            open,
            config_use,
            config_scan,
            config_mirror,
            config_share,
            config_pull,
            config_export,
            config_gitignore,
            timemap_save,
            timemap_find,
            group_set,
            group_map,
            group_state,
            align_estimate,
            derive_apply,
            recipes_list,
            recipe_save,
            recipe_delete,
            recipes_import,
            recipes_export,
            derive_check,
            set_expr_env,
            adjust_apply,
            missing_between,
            check_sets_list,
            check_set_save,
            check_set_delete,
            checks_run,
            checks_run_path,
            write_text_file,
            check_preview,
            events_list,
            event_save,
            event_delete,
            spectrum,
            packet_parts,
            import_inspect,
            profile_save,
            usage,
            cache_info,
            cache_clear,
            cache_reveal,
            close,
            hold,
            holders,
            window_new,
            window_take,
            window_frame,
            window_place,
            settings_get,
            settings_save,
            view,
            values_at,
            widgets_frame,
            xy_points,
            stats,
            rows,
            table_query,
            table_cancel,
            table_hit,
            table_marks,
            table_mask,
            table_unmask,
            filter_check,
            list_folder,
            needs_descriptor,
            descriptor_list,
            descriptor_preview,
            descriptor_find_frames,
            descriptor_save,
            descriptor_share,
            shared_views,
            share_view,
            export_view,
            export_data,
            save_file,
            live_connect,
            live_state,
            live_stop,
            live_pause,
            live_trigger,
            live_preview,
            echo,
            launch,
            gate_report,
            gate_manifest
        ])
        .run(tauri::generate_context!())
        .expect("TelemetryBench failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_closes_with_its_last_window() {
        let h = Holds::default();
        h.add(1, "a");
        h.add(1, "b");
        assert_eq!(h.others(1, "a"), vec!["b".to_string()]);
        assert!(!h.release(1, "a"), "b still holds it");
        assert!(h.release(1, "b"), "the last hold closes it");
        assert!(h.release(9, "a"), "an id nobody holds closes at once");
        h.add(2, "a");
        h.add(3, "a");
        h.add(3, "b");
        let mut of = h.of("a");
        of.sort();
        assert_eq!(of, vec![2, 3]);
    }

    #[test]
    fn query_rows_follow_the_sorted_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("q.csv");
        let body: String = (0..10).map(|i| format!("{i},{},{}\n", i * 10, (i * 7 % 10) as f64)).collect();
        std::fs::write(&path, format!("time,a,b\n{body}")).unwrap();
        let opts = tlmbench_decode::OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
        let rec = tlmbench_decode::open(&path, &opts).unwrap();
        // rows 2..8 of the file, ids as offsets from 2: (b descending) then the ones 3, 4, 5 are consecutive
        let q = RowQuery { first: 2, ids: vec![5, 0, 1, 2, 3], hits: Vec::new() };
        let out = encode_query_rows(rec.as_ref(), None, &[1], &q, 1, 10);
        let u32_at = |o: usize| u32::from_le_bytes(out[o..o + 4].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(out[o..o + 8].try_into().unwrap());
        assert_eq!((u64_at(16), u32_at(24), u32_at(28)), (5, 4, 1), "total, rows returned, indexed");
        let index: Vec<u64> = (0..4).map(|k| u64_at(32 + 32 + 8 * k)).collect();
        assert_eq!(index, vec![2, 3, 4, 5]);
        let times: Vec<i64> = (0..4).map(|k| u64_at(32 + 8 * k) as i64).collect();
        assert_eq!(times, (2..6).map(|i| rec.time_at(i)).collect::<Vec<_>>());
        let b: Vec<f64> = (0..4).map(|k| f64::from_bits(u64_at(32 + 64 + 8 * k))).collect();
        assert_eq!(b, vec![4.0, 1.0, 8.0, 5.0], "b = i * 7 % 10 at rows 2..6");
    }

    #[test]
    fn marks_join_runs_under_a_pixel_and_stay_capped() {
        let runs = [(0, 10), (12, 20), (100, 110), (300, 310)];
        assert_eq!(runs_in(&runs, 0, 1000, 5, 10), [(0, 20), (100, 110), (300, 310)]);
        assert_eq!(runs_in(&runs, 50, 200, 0, 10), [(100, 110)], "only what touches the span");
        assert_eq!(runs_in(&runs, 20, 21, 0, 10), [(12, 20)], "a run ending at the span's start touches it");
        assert_eq!(runs_in(&runs, 0, 1000, 0, 2).len(), 2, "joined further to fit the cap");
    }

    #[test]
    fn rows_window() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.csv");
        let body: String = (0..10).map(|i| format!("{i},{},{}\n", i * 10, i as f64 / 2.0)).collect();
        std::fs::write(&path, format!("time,a,b\n{body}")).unwrap();
        let opts = tlmbench_decode::OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
        let rec = tlmbench_decode::open(&path, &opts).unwrap();
        let t = |i: usize| rec.time_at(i);
        // samples 2..8 are in view; rows 1..4 of them, channel 9 doesn't exist
        let out = encode_rows(rec.as_ref(), None, &[1, 9], t(2), t(8), 1, 3);
        let u32_at = |o: usize| u32::from_le_bytes(out[o..o + 4].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(out[o..o + 8].try_into().unwrap());
        assert_eq!(&out[..4], b"TLMR");
        assert_eq!((u32_at(4), u64_at(8), u64_at(16), u32_at(24)), (1, 3, 6, 3));
        let times: Vec<i64> = (0..3).map(|k| u64_at(32 + 8 * k) as i64).collect();
        assert_eq!(times, vec![t(3), t(4), t(5)]);
        let b: Vec<f64> = (0..3).map(|k| f64::from_bits(u64_at(56 + 8 * k))).collect();
        assert_eq!(b, vec![1.5, 2.0, 2.5]);
        // past the end: no rows, the count still given
        let out = encode_rows(rec.as_ref(), None, &[0], t(2), t(8), 100, 10);
        assert_eq!((u64::from_le_bytes(out[16..24].try_into().unwrap()), out.len()), (6, 32));
    }
}

#[cfg(test)]
mod derive_tests {
    use super::*;

    fn recipe(name: &str, expr: &str, automatic: bool) -> tlmbench_expr::Recipe {
        tlmbench_expr::Recipe {
            name: name.into(),
            expr: expr.into(),
            unit: Some("W".into()),
            category: None,
            decimals: None,
            automatic,
        }
    }

    /// Automatic recipes join a file's channels, attached ones too, and one that can't apply is reported only when asked for.
    #[test]
    fn a_file_shows_its_automatic_and_attached_recipes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.csv");
        let body: String = (0..50).map(|i| format!("{i},{},{}\n", 10 + i % 3, 1 + i % 4)).collect();
        std::fs::write(&path, format!("time,v,i\n{body}")).unwrap();
        let store = Store::new(Some(&dir.path().join("cache")));
        let lib = tlmbench_store::Library::new(store.root());
        lib.save(&recipe("Power", "v * i", true), None).unwrap();
        lib.save(&recipe("Needs gps", "gps + 1", true), None).unwrap(); // automatic, cannot apply: silent
        lib.save(&recipe("Half", "v / 2", false), None).unwrap(); // only when attached
        lib.save(&recipe("Bad", "nope + 1", false), None).unwrap();
        let base = Arc::new(store.open(&path, None).unwrap());

        let (shown, derived, skipped) = with_derived(&store, &base, &[], &[], &[]).unwrap();
        let names: Vec<&str> = shown.recording().info().channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["v", "i", "Power"]);
        assert_eq!(
            (derived.len(), derived[0].channel, derived[0].expr.as_str(), derived[0].automatic),
            (1, 2, "v * i", true)
        );
        assert!(skipped.is_empty());

        let (shown, derived, skipped) =
            with_derived(&store, &base, &["Half".into(), "Bad".into(), "Gone".into()], &[], &[]).unwrap();
        assert_eq!(shown.recording().info().channels.len(), 4);
        assert_eq!(
            derived.iter().map(|d| (d.name.as_str(), d.channel)).collect::<Vec<_>>(),
            [("Half", 2), ("Power", 3)]
        );
        let mut s: Vec<(String, String)> = skipped.into_iter().map(|s| (s.name, s.why)).collect();
        s.sort();
        assert_eq!(
            s,
            [
                ("Bad".into(), "There's no channel called nope".into()),
                ("Gone".into(), "There is no recipe with that name".into())
            ]
        );
        // the derived channel's values are in the joined recording
        assert_eq!(shown.recording().value_at(3, 1, &Default::default()), 22.0);

        // an automatic recipe can be left out of one file
        let (shown, derived, _) = with_derived(&store, &base, &[], &["Power".into()], &[]).unwrap();
        assert!(derived.is_empty() && Arc::ptr_eq(&shown, &base));

        // with no recipes the file is just the recording
        for r in ["Power", "Needs gps", "Half", "Bad"] {
            lib.delete(r).unwrap();
        }
        let (plain, derived, _) = with_derived(&store, &base, &[], &[], &[]).unwrap();
        assert!(Arc::ptr_eq(&plain, &base) && derived.is_empty());
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;
    use tauri::async_runtime::block_on as rt;

    #[test]
    fn share_then_list() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().display().to_string();
        assert!(rt(shared_views(folder.clone())).unwrap().is_empty());
        let view = serde_json::json!({ "schema": 1, "name": "Review", "panes": [] });
        let path = rt(share_view(folder.clone(), "review.tlmview.json".into(), view)).unwrap();
        assert!(path.ends_with("review.tlmview.json"));
        std::fs::write(views_dir(&folder).join("bad.tlmview.json"), "{").unwrap();
        let got = rt(shared_views(folder.clone())).unwrap();
        assert_eq!(
            got.iter().map(|v| v.file.as_str()).collect::<Vec<_>>(),
            ["bad.tlmview.json", "review.tlmview.json"]
        );
        assert!(got[0].view.is_none() && got[0].error.is_some());
        assert_eq!(got[1].view.as_ref().unwrap()["name"], "Review");
        // only bare view names
        for bad in ["../x.tlmview.json", "x.json", "a/b.tlmview.json"] {
            assert!(rt(share_view(folder.clone(), bad.into(), serde_json::json!({}))).is_err(), "{bad}");
        }
    }

    /// The form's descriptor is the JSON of `Descriptor`: the preview decodes through it, a half-edited one reports
    /// its problem, and Share… writes only on its own.
    #[test]
    fn the_descriptor_form_commands() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.bin");
        // 12 frames: sync A55A, a u16 counter, 20 bytes
        let frames: Vec<u8> =
            (0..12u8).flat_map(|k| [0xA5, 0x5A, k, 0, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 1, 2, 3, 4, 5, 6, 7, k]).collect();
        std::fs::write(&path, &frames).unwrap();
        let p = path.display().to_string();
        let f = rt(descriptor_find_frames(p.clone())).unwrap().unwrap();
        assert_eq!((f.sync.as_str(), f.stride, f.start), ("A55A", 22, 0));
        let d = serde_json::json!({
            "schema": 1, "name": "Log", "files": ["log*.bin"], "endian": "little", "record_size": 20,
            "time": { "source": "rate", "rate_hz": 100.0 },
            "fields": [ { "name": "n", "type": "u16", "offset": 2 }, { "name": "tail", "type": "u8", "offset": 21, "bits": { "shift": 0, "width": 4 } } ],
            "packets": { "sync": "A55A", "header_size": 2 },
        });
        let pv = rt(descriptor_preview(p.clone(), d.clone())).unwrap();
        assert!(pv.problem.is_none(), "{:?}", pv.problem);
        assert_eq!((pv.rows.len(), pv.stride), (8, 22));
        assert_eq!(pv.rows[3].values, [Some(3.0), Some(3.0)]);
        let mut half = d.clone();
        half["fields"][0]["offset"] = serde_json::json!(40);
        assert!(rt(descriptor_preview(p.clone(), half)).unwrap().problem.unwrap().contains("runs past"));
        let junk = serde_json::json!({ "schema": 1, "endian": "sideways" });
        assert!(rt(descriptor_preview(p.clone(), junk)).unwrap().problem.is_some());
        // Share… is the one write beside the data
        assert!(!dir.path().join(".tlmbench").exists());
        let out = rt(descriptor_share(p, d)).unwrap();
        assert!(Path::new(&out).ends_with(Path::new(".tlmbench").join("decoders").join("log.toml")), "{out}");
    }
}
