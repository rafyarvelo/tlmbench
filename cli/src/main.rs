//! `tlmbench`: the command line over the same core as the desktop app. Phase 1 has `info`; convert, check,
//! export html, validate, derive and config export land in later phases.
//!
//! It reads recordings and never writes beside them: the only writes are cache files under the cache root
//! (`--cache`, `$TLMBENCH_CACHE`, or the OS cache dir shared with the app).
//!
//! Exit codes: 0 done; 2 a file couldn't be read; 3 a configuration error (a bad descriptor).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde::Serialize;
use tlmbench_core::datetime::{format_duration, format_time};
use tlmbench_core::{RecordingInfo, TimeSource};
use tlmbench_decode::{Error, OpenOptions};

#[derive(Parser, Debug)]
#[command(name = "tlmbench", version, about = "TelemetryBench on the command line")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Channels, units, types, sample count, time range and time source of each file
    Info {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// One JSON object per file, one per line
        #[arg(long)]
        json: bool,
        /// Format descriptor (TOML) for fixed-width files without one
        #[arg(long)]
        descriptor: Option<PathBuf>,
        /// Cache folder (default: $TLMBENCH_CACHE, else the OS cache dir)
        #[arg(long)]
        cache: Option<PathBuf>,
    },
}

#[derive(Serialize)]
struct InfoJson<'a> {
    #[serde(flatten)]
    info: &'a RecordingInfo,
    duration_ns: Option<i64>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Info { files, json, descriptor, cache } => {
            let opts = OpenOptions { descriptor, cache_dir: cache };
            let mut code = ExitCode::SUCCESS;
            for (k, path) in files.iter().enumerate() {
                match tlmbench_decode::open(path, &opts) {
                    Ok(rec) => {
                        let info = rec.info();
                        if json {
                            let duration_ns = info.t_first.zip(info.t_last).map(|(a, b)| b - a);
                            println!("{}", serde_json::to_string(&InfoJson { info, duration_ns }).unwrap());
                        } else {
                            if k > 0 {
                                println!();
                            }
                            print_info(info);
                        }
                    }
                    Err(e) => {
                        eprintln!("tlmbench: {e}");
                        code = ExitCode::from(match e {
                            Error::Descriptor(_) => 3,
                            _ => 2,
                        });
                    }
                }
            }
            code
        }
    }
}

fn print_info(info: &RecordingInfo) {
    println!("{}", info.path.display());
    println!("  Format    {}", info.format);
    let range = match (info.t_first, info.t_last) {
        (Some(a), Some(b)) if info.time == TimeSource::Index => {
            format!("rows {}–{}", a / 1_000_000_000, b / 1_000_000_000)
        }
        (Some(a), Some(b)) => format!("{} → {} ({})", format_time(a), format_time(b), format_duration(b - a)),
        _ => "no samples".into(),
    };
    let source = match &info.time {
        TimeSource::Column { name } => format!("column {name:?}"),
        TimeSource::Rate { hz } => format!("{} Hz", trim(*hz)),
        TimeSource::Index => "row number (no time column)".into(),
    };
    println!("  Time      {source}, {range}");
    println!("  Samples   {}", group(info.samples));
    println!("  Channels  {}", info.channels.len());
    let width = info.channels.iter().map(|c| c.name.chars().count()).max().unwrap_or(0);
    for c in &info.channels {
        let unit = c.unit.as_deref().map(|u| format!("  {u}")).unwrap_or_default();
        println!("    {:width$}  {}{unit}", c.name, c.dtype);
    }
    if !info.skipped.is_empty() {
        println!("  Skipped   {} (text)", info.skipped.join(", "));
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// 100000000 → 100,000,000
fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
