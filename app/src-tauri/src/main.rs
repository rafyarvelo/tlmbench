//! TelemetryBench desktop app: the core runs in this process, and the web UI asks it for viewport reductions over
//! Tauri IPC, which return the binary wire format (`tlmbench_core::wire`) as raw bytes.
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
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, State};
use tlmbench_core::gaps::DEFAULT_FACTOR;
use tlmbench_core::{RecordingInfo, Viewport, wire};
use tlmbench_store::{Opened as StoreOpened, Precision, Status, Store};

/// Open files, by id. Each window keeps its own view; files are shared.
struct Files {
    store: Store,
    next: AtomicU32,
    open: Mutex<HashMap<u32, Arc<StoreOpened>>>,
}

impl Files {
    fn get(&self, id: u32) -> Result<Arc<StoreOpened>, String> {
        self.open.lock().unwrap().get(&id).cloned().ok_or_else(|| "that file is not open".into())
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
}

/// Channels of `file` reduced for `[t0, t1)` over `width_px` columns, as the binary wire format. Times are
/// decimal strings: a JSON number can't hold i64 nanoseconds exactly. `precision` is "coarse" (from the pyramid
/// alone, drawn first on a cold file) or "exact".
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn view(
    file: u32,
    channels: Vec<u32>,
    t0: String,
    t1: String,
    width_px: u32,
    generation: u32,
    precision: Option<String>,
    files: State<'_, Files>,
) -> Result<Response, String> {
    let o = files.get(file)?;
    let vp = Viewport { t0: parse_time(&t0)?, t1: parse_time(&t1)?, width_px: width_px.clamp(1, 16_384) };
    let precision = if precision.as_deref() == Some("coarse") { Precision::Coarse } else { Precision::Exact };
    let bytes = spawn_blocking(move || {
        let n = o.recording().info().channels.len();
        let chans: Vec<usize> = channels.iter().map(|&c| c as usize).filter(|&c| c < n).collect();
        let started = Instant::now();
        let views = o.views(&chans, vp, DEFAULT_FACTOR, precision);
        // channels reduce in parallel: each trace reports the whole call's time
        let us = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
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
        wire::encode(generation, vp, built_until, &outs)
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
        channels
            .iter()
            .filter_map(|&c| {
                let (i, ts, v) = o.nearest(c as usize, t)?;
                Some(Nearest { channel: c, index: i as u64, t: ts.to_string(), value: (!v.is_nan()).then_some(v) })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
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
        channels
            .iter()
            .map(|&c| {
                let s = o.stats(c as usize, t0, t1)?;
                let at = |m: Option<(f64, usize)>| m.map(|(_, i)| o.recording().time_at(i).to_string());
                Some(ChannelStats {
                    channel: c,
                    count: s.count,
                    min: s.min.map(|m| m.0),
                    min_t: at(s.min),
                    max: s.max.map(|m| m.0),
                    max_t: at(s.max),
                    mean: num(s.mean),
                    std: num(s.std),
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
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
    /// "1" (Phase 1), "2" (Phase 2), or None.
    gate: Option<&'static str>,
    /// Gate 2: "reopen" (the pyramid is cached) or "first" (a first-ever open).
    gate_mode: Option<String>,
}

#[tauri::command]
fn launch() -> Launch {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(file) = env("TLMBENCH_GATE2") {
        return Launch { file: Some(file), gate: Some("2"), gate_mode: env("TLMBENCH_GATE2_MODE") };
    }
    match env("TLMBENCH_GATE") {
        Some(file) => Launch { file: Some(file), gate: Some("1"), gate_mode: None },
        None => Launch { file: std::env::args().nth(1).filter(|a| !a.starts_with('-')), gate: None, gate_mode: None },
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
        .manage(Files { store: Store::new(None), next: AtomicU32::new(0), open: Mutex::default() })
        .invoke_handler(tauri::generate_handler![open, close, view, values_at, stats, echo, launch, gate_report])
        .run(tauri::generate_context!())
        .expect("TelemetryBench failed to start");
}
