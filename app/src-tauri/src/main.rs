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
use tauri::{AppHandle, Emitter, State};
use tlmbench_core::gaps::DEFAULT_FACTOR;
use tlmbench_core::table::{self, Col, Filter, SortKey};
use tlmbench_core::{RecordingInfo, Viewport, wire};
use tlmbench_decode::profile::ImportProfile;
use tlmbench_store::{Opened as StoreOpened, Precision, Request, Status, Store};

/// Open files, by id, and live sources (each a file that grows). Each window keeps its own view; files are shared.
struct Files {
    store: Store,
    next: AtomicU32,
    open: Mutex<HashMap<u32, Arc<StoreOpened>>>,
    /// A file with derived channels shows them joined to its recording: the recording alone, to join again when the
    /// recipes change.
    bases: Mutex<HashMap<u32, Arc<StoreOpened>>>,
    live: Mutex<HashMap<u32, Arc<LiveEntry>>>,
    /// Sorted and filtered row lists, by query token (the latest per table; see `table_query`).
    queries: Mutex<HashMap<u32, Arc<RowQuery>>>,
    /// Running queries' cancel flags, by token.
    cancels: Mutex<HashMap<u32, Arc<AtomicBool>>>,
}

impl Files {
    /// The file's recording, or a live source's as of its last refresh.
    fn get(&self, id: u32) -> Result<Arc<StoreOpened>, String> {
        if let Some(o) = self.open.lock().unwrap().get(&id) {
            return Ok(o.clone());
        }
        let live = self.live.lock().unwrap().get(&id).cloned();
        live.map(|l| l.follow.read().unwrap().opened()).ok_or_else(|| "that file is not open".into())
    }
}

/// A live source: recorded into a session (tlmbench_live) and followed as its file grows (Store::open_live).
struct LiveEntry {
    source: Mutex<Option<tlmbench_live::Source>>,
    /// The source's newest part, followed (a tailed file's rotation starts a new one).
    follow: RwLock<Arc<tlmbench_store::Live>>,
    part: AtomicUsize,
    session: std::path::PathBuf,
    /// The last health while the source ran (kept once it stops).
    last: Mutex<tlmbench_live::Health>,
    closed: AtomicBool,
}

impl LiveEntry {
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

/// `base` with the library's recipes that apply to it (the automatic ones, and those named in `attached`) as channels
/// after its own: what the file shows, the channels derived, and the attached recipes that didn't apply.
fn with_derived(
    store: &Store,
    base: &Arc<StoreOpened>,
    attached: &[String],
) -> Result<(Arc<StoreOpened>, Vec<DerivedInfo>, Vec<Skipped>), String> {
    let lib = tlmbench_store::Library::new(store.root());
    let recipes: Vec<tlmbench_expr::Recipe> =
        lib.list().into_iter().filter(|r| r.automatic || attached.contains(&r.name)).collect();
    let mut skipped: Vec<Skipped> = attached
        .iter()
        .filter(|n| !recipes.iter().any(|r| &r.name == *n))
        .map(|n| Skipped { name: n.clone(), why: "There is no recipe with that name".into() })
        .collect();
    if recipes.is_empty() {
        return Ok((base.clone(), Vec::new(), skipped));
    }
    let d = store.open_derived(base, &recipes).map_err(|e| e.to_string())?;
    // an automatic recipe that needs channels the file lacks just doesn't apply; one the user attached is reported
    skipped.extend(
        d.skipped
            .iter()
            .filter(|(n, _)| attached.contains(n))
            .map(|(n, e)| Skipped { name: n.clone(), why: e.msg.clone() }),
    );
    let Some(opened) = d.opened else { return Ok((base.clone(), Vec::new(), skipped)) };
    let first = base.recording().info().channels.len();
    let derived = d
        .names
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let r = recipes.iter().find(|r| &r.name == name).expect("computed from a recipe in the list");
            DerivedInfo { channel: first + k, name: name.clone(), expr: r.expr.clone(), automatic: r.automatic }
        })
        .collect();
    let joined = store.join(base.clone(), Arc::new(opened)).map_err(|e| e.to_string())?;
    Ok((Arc::new(joined), derived, skipped))
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
fn saved_profile(root: &Path, path: &Path) -> Option<ImportProfile> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "csv" | "tsv" | "txt") {
        return None;
    }
    let i = tlmbench_decode::csv::inspect_file(path).ok()?;
    let header: Vec<String> = i.columns.into_iter().map(|c| c.source).collect();
    tlmbench_decode::profile::find(root, &header).exact
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
                        let found = tlmbench_decode::profile::find(&root, &header);
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

#[tauri::command]
async fn open(
    path: String,
    profile: Option<ImportProfile>,
    part: Option<String>,
    recipes: Option<Vec<String>>,
    progress: Channel<Progress>,
    files: State<'_, Files>,
) -> Result<Opened, String> {
    let store = files.store.clone();
    let attached = recipes.unwrap_or_default();
    let (base, shown, derived, skipped) = spawn_blocking(move || -> Result<_, String> {
        let profile = profile.or_else(|| saved_profile(store.root(), Path::new(&path)));
        let base = store
            .clone()
            .with_profile(profile)
            .open_part(Path::new(&path), None, part.as_deref())
            .map_err(|e| e.to_string())?;
        let base = Arc::new(base);
        let (shown, derived, skipped) = with_derived(&store, &base, &attached)?;
        Ok((base, shown, derived, skipped))
    })
    .await
    .map_err(|e| e.to_string())??;
    let id = files.next.fetch_add(1, Ordering::Relaxed) + 1;
    let info = shown.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    let status = shown.status();
    files.open.lock().unwrap().insert(id, shown.clone());
    files.bases.lock().unwrap().insert(id, base);
    if status != Status::Built {
        build_in_background(shown, progress);
    }
    Ok(Opened { id, info, range, status, derived, skipped })
}

/// Changes which recipes an open file shows (the automatic ones, and these): the same file id, with its channels
/// recomputed. The recording's own channels keep their numbers; derived ones come after.
#[tauri::command]
async fn derive_apply(
    file: u32,
    recipes: Vec<String>,
    progress: Channel<Progress>,
    files: State<'_, Files>,
) -> Result<Opened, String> {
    let base = files.bases.lock().unwrap().get(&file).cloned().ok_or("that file is not open")?;
    let store = files.store.clone();
    let b = base.clone();
    let (shown, derived, skipped) =
        spawn_blocking(move || with_derived(&store, &b, &recipes)).await.map_err(|e| e.to_string())??;
    let info = shown.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    let status = shown.status();
    if let Some(old) = files.open.lock().unwrap().insert(file, shown.clone()) {
        old.cancel();
    }
    if status != Status::Built {
        build_in_background(shown, progress);
    }
    Ok(Opened { id: file, info, range, status, derived, skipped })
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

#[derive(Serialize)]
struct CheckError {
    msg: String,
    at: usize,
}

/// The Derive dialog's check: an expression against a file's channels, with a summary of what it gives over the start
/// of the file, or the mistake and the character it is at.
#[tauri::command]
async fn derive_check(
    file: u32,
    expr: String,
    files: State<'_, Files>,
) -> Result<Result<tlmbench_store::Preview, CheckError>, String> {
    let o = files.get(file)?;
    spawn_blocking(move || tlmbench_store::check(o.recording(), &expr).map_err(|e| CheckError { msg: e.msg, at: e.at }))
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

#[tauri::command]
fn close(file: u32, files: State<'_, Files>) {
    files.bases.lock().unwrap().remove(&file);
    if let Some(o) = files.open.lock().unwrap().remove(&file) {
        o.cancel();
    }
    if let Some(l) = files.live.lock().unwrap().remove(&file) {
        l.closed.store(true, Ordering::Release);
        if let Some(s) = l.source.lock().unwrap().take() {
            *l.last.lock().unwrap() = s.stop();
        }
    }
}

/// How often a live source's file is read again (new samples folded into its pyramid).
const LIVE_REFRESH: Duration = Duration::from_millis(50);

/// Connects a live source (`tcp://`, `udp://` or `file://`; sockets need the frames' descriptor, a TOML file) into
/// a new session in the Recordings folder, and returns it opened like a file once its first samples are recorded.
#[tauri::command]
async fn live_connect(endpoint: String, descriptor: Option<String>, files: State<'_, Files>) -> Result<Opened, String> {
    let store = files.store.clone();
    let entry = spawn_blocking(move || -> Result<LiveEntry, String> {
        let endpoint: tlmbench_live::Endpoint = endpoint.parse()?;
        let desc = match descriptor.filter(|d| !d.is_empty()) {
            Some(p) => {
                let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
                Some(tlmbench_decode_descriptor(&text).map_err(|e| format!("{p}: {e}"))?)
            }
            None => None,
        };
        let mut session =
            tlmbench_live::Session::create(&tlmbench_live::recordings_dir()).map_err(|e| e.to_string())?;
        let source =
            session.add_source(endpoint.clone(), desc.as_ref(), &Default::default()).map_err(|e| e.to_string())?;
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
        let _ = session.update(0, &source);
        Ok(LiveEntry {
            last: Mutex::new(source.health()),
            source: Mutex::new(Some(source)),
            follow: RwLock::new(Arc::new(live)),
            part: AtomicUsize::new(files_now.len()),
            session: session.dir().to_owned(),
            closed: AtomicBool::new(false),
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    let entry = Arc::new(entry);
    let id = files.next.fetch_add(1, Ordering::Relaxed) + 1;
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
                *follower.follow.write().unwrap() = Arc::new(next);
                follower.part.store(parts.len(), Ordering::Relaxed);
            }
            let live = follower.follow.read().unwrap().clone();
            let _ = live.refresh();
            std::thread::sleep(LIVE_REFRESH);
        }
    });
    let o = entry.follow.read().unwrap().opened();
    let info = o.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    Ok(Opened { id, info, range, status: Status::Built, derived: Vec::new(), skipped: Vec::new() })
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
    health: tlmbench_live::Health,
}

#[tauri::command]
fn live_state(file: u32, files: State<'_, Files>) -> Result<LiveState, String> {
    let l = files.live.lock().unwrap().get(&file).cloned().ok_or("that source is not open")?;
    let o = l.follow.read().unwrap().opened();
    let info = o.recording().info().clone();
    Ok(LiveState {
        range: info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string())),
        samples: info.samples,
        part: l.part.load(Ordering::Relaxed),
        info,
        session: l.session.display().to_string(),
        health: l.health(),
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
    let precision = if precision.as_deref() == Some("coarse") { Precision::Coarse } else { Precision::Exact };
    let n = o.recording().info().channels.len();
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
        let batch = o.view_batch(&requests, precision);
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
    let t = parse_time(&t)?;
    spawn_blocking(move || {
        let n = o.recording().info().channels.len();
        let channels: Vec<u32> = channels.into_iter().filter(|&c| (c as usize) < n).collect();
        let chans: Vec<usize> = channels.iter().map(|&c| c as usize).collect();
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
    if let Some(token) = query {
        let q = files.queries.lock().unwrap().get(&token).cloned().ok_or("that query is gone")?;
        let bytes = spawn_blocking(move || {
            let channels: Vec<usize> = channels.into_iter().map(|c| c as usize).collect();
            encode_query_rows(o.recording(), &channels, &q, offset, limit)
        })
        .await
        .map_err(|e| e.to_string())?;
        return Ok(Response::new(bytes));
    }
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    let bytes = spawn_blocking(move || {
        let channels: Vec<usize> = channels.into_iter().map(|c| c as usize).collect();
        encode_rows(o.recording(), &channels, t0, t1, offset, limit)
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
            fs.push((col_of(&f.col)?, p));
        }
    }
    let sort = sort
        .iter()
        .map(|k| Ok(SortKey { col: col_of(&k.col)?, descending: k.desc }))
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
            fs.push((col_of(&f.col)?, p));
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
    channels: &[usize],
    q: &RowQuery,
    offset: u64,
    limit: u32,
) -> Vec<u8> {
    let n = rec.info().channels.len();
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
            let (scale, off) = rec.scale_offset(c);
            col.extend(rec.read(c, a, b).to_f64(scale, off));
        }
        at = e;
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
    channels: &[usize],
    t0: i64,
    t1: i64,
    offset: u64,
    limit: u32,
) -> Vec<u8> {
    let n = rec.info().channels.len();
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
    for t in rec.read_times(start, end) {
        out.extend_from_slice(&t.to_le_bytes());
    }
    for &c in &chans {
        let (scale, off) = rec.scale_offset(c);
        for v in rec.read(c, start, end).to_f64(scale, off) {
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

/// Exact stats of each channel over `[t0, t1)`; None until the pyramid is built (exact answers wait for it).
#[tauri::command]
async fn stats(
    file: u32,
    channels: Vec<u32>,
    span: Span,
    files: State<'_, Files>,
) -> Result<Option<Vec<ChannelStats>>, String> {
    let o = files.get(file)?;
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    spawn_blocking(move || {
        let num = |v: f64| (!v.is_nan()).then_some(v);
        let n = o.recording().info().channels.len();
        let channels: Vec<u32> = channels.into_iter().filter(|&c| (c as usize) < n).collect();
        let chans: Vec<usize> = channels.iter().map(|&c| c as usize).collect();
        let all = o.stats(&chans, t0, t1)?;
        Some(
            channels
                .iter()
                .zip(all)
                .map(|(&c, s)| {
                    let at = |m: Option<(f64, usize)>| m.map(|(_, i)| o.recording().time_at(i).to_string());
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Files {
            store: Store::new(None),
            next: AtomicU32::new(0),
            open: Mutex::default(),
            bases: Mutex::default(),
            live: Mutex::default(),
            queries: Mutex::default(),
            cancels: Mutex::default(),
        })
        .invoke_handler(tauri::generate_handler![
            open,
            derive_apply,
            recipes_list,
            recipe_save,
            recipe_delete,
            recipes_import,
            recipes_export,
            derive_check,
            spectrum,
            packet_parts,
            import_inspect,
            profile_save,
            usage,
            cache_info,
            cache_clear,
            cache_reveal,
            close,
            view,
            values_at,
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
    fn query_rows_follow_the_sorted_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("q.csv");
        let body: String = (0..10).map(|i| format!("{i},{},{}\n", i * 10, (i * 7 % 10) as f64)).collect();
        std::fs::write(&path, format!("time,a,b\n{body}")).unwrap();
        let opts = tlmbench_decode::OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
        let rec = tlmbench_decode::open(&path, &opts).unwrap();
        // rows 2..8 of the file, ids as offsets from 2: (b descending) then the ones 3, 4, 5 are consecutive
        let q = RowQuery { first: 2, ids: vec![5, 0, 1, 2, 3], hits: Vec::new() };
        let out = encode_query_rows(rec.as_ref(), &[1], &q, 1, 10);
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
        let out = encode_rows(rec.as_ref(), &[1, 9], t(2), t(8), 1, 3);
        let u32_at = |o: usize| u32::from_le_bytes(out[o..o + 4].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(out[o..o + 8].try_into().unwrap());
        assert_eq!(&out[..4], b"TLMR");
        assert_eq!((u32_at(4), u64_at(8), u64_at(16), u32_at(24)), (1, 3, 6, 3));
        let times: Vec<i64> = (0..3).map(|k| u64_at(32 + 8 * k) as i64).collect();
        assert_eq!(times, vec![t(3), t(4), t(5)]);
        let b: Vec<f64> = (0..3).map(|k| f64::from_bits(u64_at(56 + 8 * k))).collect();
        assert_eq!(b, vec![1.5, 2.0, 2.5]);
        // past the end: no rows, the count still given
        let out = encode_rows(rec.as_ref(), &[0], t(2), t(8), 100, 10);
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

        let (shown, derived, skipped) = with_derived(&store, &base, &[]).unwrap();
        let names: Vec<&str> = shown.recording().info().channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["v", "i", "Power"]);
        assert_eq!(
            (derived.len(), derived[0].channel, derived[0].expr.as_str(), derived[0].automatic),
            (1, 2, "v * i", true)
        );
        assert!(skipped.is_empty());

        let (shown, derived, skipped) =
            with_derived(&store, &base, &["Half".into(), "Bad".into(), "Gone".into()]).unwrap();
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

        // with no recipes the file is just the recording
        for r in ["Power", "Needs gps", "Half", "Bad"] {
            lib.delete(r).unwrap();
        }
        let (plain, derived, _) = with_derived(&store, &base, &[]).unwrap();
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
