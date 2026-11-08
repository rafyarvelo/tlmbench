//! TelemetryBench desktop app: the core runs in this process, and the web UI asks it for viewport reductions over
//! Tauri IPC, which return the binary wire format (`tlmbench_core::wire`) as raw bytes: one batch per request, for
//! every pane and the overview.
//!
//! Recordings open through the store (`tlmbench_store`): read-only, with their pyramid and gap index built in the
//! background into the shared cache root, never beside the data. While a build runs, views are exact from the
//! summarized part and the UI hatches the rest; its progress streams over a channel.
//!
//! `tlmbench-app <file>` opens a file at launch. `TLMBENCH_GATE=<file>` runs the Phase 1 gate and
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
use tauri::{AppHandle, State};
use tlmbench_core::gaps::DEFAULT_FACTOR;
use tlmbench_core::{RecordingInfo, Viewport, wire};
use tlmbench_store::{Opened as StoreOpened, Precision, Request, Status, Store};

/// Open files, by id, and live sources (each a file that grows). Each window keeps its own view; files are shared.
struct Files {
    store: Store,
    next: AtomicU32,
    open: Mutex<HashMap<u32, Arc<StoreOpened>>>,
    live: Mutex<HashMap<u32, Arc<LiveEntry>>>,
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

#[tauri::command]
async fn open(path: String, progress: Channel<Progress>, files: State<'_, Files>) -> Result<Opened, String> {
    let store = files.store.clone();
    let o = spawn_blocking(move || store.open(Path::new(&path), None))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let o = Arc::new(o);
    let id = files.next.fetch_add(1, Ordering::Relaxed) + 1;
    let info = o.recording().info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    let status = o.status();
    files.open.lock().unwrap().insert(id, o.clone());
    if status != Status::Built {
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
    Ok(Opened { id, info, range, status })
}

#[tauri::command]
fn close(file: u32, files: State<'_, Files>) {
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
    Ok(Opened { id, info, range, status: Status::Built })
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
        requests.push(Request { channels, gap_factors, vp });
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
    files: State<'_, Files>,
) -> Result<Response, String> {
    let o = files.get(file)?;
    let (t0, t1) = (parse_time(&span.t0)?, parse_time(&span.t1)?);
    let bytes = spawn_blocking(move || {
        let channels: Vec<usize> = channels.into_iter().map(|c| c as usize).collect();
        encode_rows(o.recording(), &channels, t0, t1, offset, limit)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
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
    /// Folders: what they hold, recordings and the folders that lead to some.
    children: Vec<FolderEntry>,
}

/// The files under `path`, in their folders, sorted by name (recordings are `openable`: CSV, TSV, TXT, .tlm and
/// Parquet); folders without any file, and hidden ones, are left out. Only reads the folder (Explorer's Open folder).
#[tauri::command]
async fn list_folder(path: String) -> Result<FolderEntry, String> {
    fn walk(p: &Path, depth: u32) -> Option<FolderEntry> {
        let name = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
        let meta = std::fs::metadata(p).ok()?;
        if !meta.is_dir() {
            let ext = p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
            let openable = matches!(ext.as_deref(), Some("csv" | "tsv" | "txt" | "tlm" | "parquet"));
            return Some(FolderEntry {
                path: p.display().to_string(),
                name,
                dir: false,
                bytes: meta.len(),
                openable,
                children: Vec::new(),
            });
        }
        let mut children: Vec<FolderEntry> = if depth < 8 {
            std::fs::read_dir(p)
                .ok()?
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter_map(|e| walk(&e.path(), depth + 1))
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
            children,
        })
    }
    spawn_blocking(move || walk(Path::new(&path), 0).ok_or_else(|| format!("{path} isn't a folder that can be read")))
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

/// `bytes` zero bytes: the IPC transfer cost alone, for the gate report.
#[tauri::command]
fn echo(bytes: u32) -> Response {
    Response::new(vec![0; bytes.min(64 << 20) as usize])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Launch {
    file: Option<String>,
    /// "1" (Phase 1), "2" (Phase 2), "3" (Phase 3: `file` is the source's endpoint), or None.
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
            live: Mutex::default(),
        })
        .invoke_handler(tauri::generate_handler![
            open,
            usage,
            cache_info,
            cache_clear,
            close,
            view,
            values_at,
            stats,
            rows,
            list_folder,
            shared_views,
            share_view,
            export_view,
            live_connect,
            live_state,
            live_stop,
            echo,
            launch,
            gate_report
        ])
        .run(tauri::generate_context!())
        .expect("TelemetryBench failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
