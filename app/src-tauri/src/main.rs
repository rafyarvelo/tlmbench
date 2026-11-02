//! TelemetryBench desktop app (Phase 1): the core runs in this process, and the web UI asks it for viewport
//! reductions over Tauri IPC, which return the binary wire format (`tlmbench_core::wire`) as raw bytes.
//!
//! Files open read-only; CSV builds its cache in the shared cache root. Nothing is written beside the data.
//!
//! `tlmbench-app <file>` opens a file at launch. `TLMBENCH_GATE=<file>` runs the Phase 1 gate: a scripted
//! 30-second sweep over the file, then the report on stdout, and exit code 0 (pass) or 1.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::async_runtime::spawn_blocking;
use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tlmbench_core::{Recording, RecordingInfo, Viewport, wire};
use tlmbench_decode::OpenOptions;

/// Open files, by id. Each window keeps its own view; files are shared.
#[derive(Default)]
struct Files {
    next: AtomicU32,
    open: Mutex<HashMap<u32, Arc<dyn Recording>>>,
}

#[derive(Serialize)]
struct Opened {
    id: u32,
    info: RecordingInfo,
    /// First and last sample times as decimal strings: JSON numbers lose i64 nanoseconds past 2^53.
    range: Option<(String, String)>,
}

#[tauri::command]
async fn open(path: String, files: State<'_, Files>) -> Result<Opened, String> {
    let rec = spawn_blocking(move || tlmbench_decode::open(Path::new(&path), &OpenOptions::default()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let rec: Arc<dyn Recording> = Arc::from(rec);
    let id = files.next.fetch_add(1, Ordering::Relaxed) + 1;
    let info = rec.info().clone();
    let range = info.t_first.zip(info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    files.open.lock().unwrap().insert(id, rec);
    Ok(Opened { id, info, range })
}

#[tauri::command]
fn close(file: u32, files: State<'_, Files>) {
    files.open.lock().unwrap().remove(&file);
}

/// Channels of `file` reduced for `[t0, t1)` over `width_px` columns, as the binary wire format. Times are
/// decimal strings: a JSON number can't hold i64 nanoseconds exactly.
#[tauri::command]
async fn view(
    file: u32,
    channels: Vec<u32>,
    t0: String,
    t1: String,
    width_px: u32,
    generation: u32,
    files: State<'_, Files>,
) -> Result<Response, String> {
    let rec = files.open.lock().unwrap().get(&file).cloned().ok_or("that file is not open")?;
    let parse = |s: &str| s.parse::<i64>().map_err(|e| format!("time {s:?}: {e}"));
    let vp = Viewport { t0: parse(&t0)?, t1: parse(&t1)?, width_px: width_px.clamp(1, 16_384) };
    let bytes = spawn_blocking(move || {
        let n = rec.info().channels.len();
        let traces: Vec<_> = channels
            .iter()
            .filter(|&&c| (c as usize) < n)
            .map(|&c| {
                let started = Instant::now();
                let trace = rec.trace(c as usize, vp);
                (c, trace, started.elapsed().as_micros().min(u32::MAX as u128) as u32)
            })
            .collect();
        let outs: Vec<_> =
            traces.iter().map(|(c, t, us)| wire::Out { channel: *c, trace: t, reduce_us: *us }).collect();
        wire::encode(generation, vp, &outs)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

/// `bytes` zero bytes: the IPC transfer cost alone, for the gate report.
#[tauri::command]
fn echo(bytes: u32) -> Response {
    Response::new(vec![0; bytes.min(64 << 20) as usize])
}

#[derive(Serialize)]
struct Launch {
    file: Option<String>,
    gate: bool,
}

#[tauri::command]
fn launch() -> Launch {
    match std::env::var("TLMBENCH_GATE").ok().filter(|v| !v.is_empty()) {
        Some(file) => Launch { file: Some(file), gate: true },
        None => Launch { file: std::env::args().nth(1).filter(|a| !a.starts_with('-')), gate: false },
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
        .manage(Files::default())
        .invoke_handler(tauri::generate_handler![open, close, view, echo, launch, gate_report])
        .run(tauri::generate_context!())
        .expect("TelemetryBench failed to start");
}
