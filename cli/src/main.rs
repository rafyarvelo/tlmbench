//! `tlmbench`: the command line over the same core as the desktop app. It has `info`, `cache`, `query` and
//! `record`; convert, check, export html, validate, derive and config export land in later phases.
//!
//! It reads recordings and never writes beside them: the only writes are cache entries under the cache root
//! (`--cache`, `$TLMBENCH_CACHE`, or the OS cache dir shared with the app), and `record`'s session folders (`--out`,
//! `$TLMBENCH_RECORDINGS`, or the app's Recordings folder).
//!
//! Exit codes: 0 done; 2 a file couldn't be read; 3 a configuration error (a bad descriptor or argument).

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use tlmbench_core::datetime::{format_duration, format_time, parse_decimal, parse_iso};
use tlmbench_core::gaps::DEFAULT_FACTOR;
use tlmbench_core::{RecordingInfo, TimeSource, Timestamp, Viewport};
use tlmbench_store::{Opened, Precision, Status, Store};

#[derive(Parser, Debug)]
#[command(name = "tlmbench", version, about = "TelemetryBench on the command line")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Debug)]
struct Common {
    /// Format descriptor (TOML) for fixed-width files without one
    #[arg(long, global = true)]
    descriptor: Option<PathBuf>,
    /// Cache folder (default: $TLMBENCH_CACHE, else the OS cache dir)
    #[arg(long, global = true)]
    cache: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Channels, units, types, sample count, time range, time source and gaps of each file
    Info {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// One JSON object per file, one per line
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        common: Common,
    },
    /// Build, list or clear the cache (pyramids, gap indexes and CSV columns)
    Cache {
        #[command(subcommand)]
        action: CacheAction,
        #[command(flatten)]
        common: Common,
    },
    /// Read a view, values at a time, or exact stats from a recording
    Query {
        #[command(subcommand)]
        what: Query,
        #[command(flatten)]
        common: Common,
    },
    /// Record a live source into a new session folder until it ends (or --seconds pass); prints its stream health
    Record {
        /// tcp://host:port (connects to it) or udp://host:port (listens on it)
        endpoint: tlmbench_live::Endpoint,
        /// The frames' format descriptor (TOML)
        #[arg(long)]
        descriptor: PathBuf,
        /// Where the session folder goes (default: $TLMBENCH_RECORDINGS, else the app's Recordings folder)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Stop after this many seconds
        #[arg(long)]
        seconds: Option<f64>,
        /// UDP: stop once no datagram has come for this many seconds
        #[arg(long)]
        idle_end: Option<f64>,
        /// The ring buffer's size in MB (default: about 10 s of the descriptor's rate)
        #[arg(long)]
        ring_mb: Option<usize>,
        /// The result as one JSON object
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CacheAction {
    /// Build each file's pyramid and gap index (resumes an interrupted build)
    Build {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// No progress on stderr
        #[arg(long)]
        quiet: bool,
    },
    /// Every cache entry, most recently opened first
    List {
        #[arg(long)]
        json: bool,
    },
    /// Remove every entry no program has open
    Clear,
}

#[derive(Args, Debug)]
struct Pick {
    file: PathBuf,
    /// Channel name or number (repeat for several; default: all)
    #[arg(long = "channel", short = 'c')]
    channels: Vec<String>,
}

#[derive(Args, Debug)]
struct Span {
    /// Start: an ISO 8601 time, or seconds from the first sample (default: the first sample)
    #[arg(long)]
    from: Option<String>,
    /// End, exclusive, as for --from (default: after the last sample)
    #[arg(long)]
    until: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Query {
    /// The points a chart of the span draws (M4, or raw samples when few), as CSV: channel, time_ns, value
    View {
        #[command(flatten)]
        pick: Pick,
        #[command(flatten)]
        span: Span,
        /// Pixel columns
        #[arg(long, default_value_t = 1000)]
        width: u32,
        /// A time jump over this many median intervals breaks the line
        #[arg(long, default_value_t = DEFAULT_FACTOR)]
        gap_factor: f64,
        /// From the pyramid alone, as charts first draw on a cold file (every extreme kept, edges within a
        /// quarter pixel)
        #[arg(long)]
        coarse: bool,
    },
    /// Each channel's sample nearest a time, as CSV: channel, index, time_ns, value
    Values {
        #[command(flatten)]
        pick: Pick,
        /// The time, as for --from of `query view`
        #[arg(long)]
        at: String,
    },
    /// Exact count, min, max, mean and standard deviation over a span, as CSV (builds the pyramid first)
    Stats {
        #[command(flatten)]
        pick: Pick,
        #[command(flatten)]
        span: Span,
    },
}

/// A failure and its exit code.
struct Fail(u8, String);

impl From<tlmbench_store::Error> for Fail {
    fn from(e: tlmbench_store::Error) -> Self {
        let code = match &e {
            tlmbench_store::Error::Decode(tlmbench_decode::Error::Descriptor(_)) => 3,
            _ => 2,
        };
        Fail(code, e.to_string())
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Info { files, json, common } => info(&files, json, &common),
        Command::Cache { action, common } => cache(action, &common),
        Command::Query { what, common } => query(what, &common),
        Command::Record { endpoint, descriptor, out, seconds, idle_end, ring_mb, json } => {
            record(endpoint, &descriptor, out, seconds, idle_end, ring_mb, json)
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail(code, msg)) => {
            if !msg.is_empty() {
                eprintln!("tlmbench: {msg}");
            }
            ExitCode::from(code)
        }
    }
}

fn store(common: &Common) -> Store {
    Store::new(common.cache.as_deref())
}

#[derive(Serialize)]
struct InfoJson<'a> {
    #[serde(flatten)]
    info: &'a RecordingInfo,
    duration_ns: Option<i64>,
    /// None until the gap index is built (`tlmbench cache build`).
    gaps: Option<GapsJson>,
}

#[derive(Serialize)]
struct GapsJson {
    median_interval_ns: i64,
    /// Time jumps over 4 × the median interval.
    count: usize,
}

fn info(files: &[PathBuf], json: bool, common: &Common) -> Result<(), Fail> {
    let store = store(common);
    let mut worst = None;
    for (k, path) in files.iter().enumerate() {
        match store.open(path, common.descriptor.as_deref()) {
            Ok(o) => {
                let info = o.recording().info();
                let gaps = gaps(&o);
                if json {
                    let duration_ns = info.t_first.zip(info.t_last).map(|(a, b)| b - a);
                    println!("{}", serde_json::to_string(&InfoJson { info, duration_ns, gaps }).unwrap());
                } else {
                    if k > 0 {
                        println!();
                    }
                    print_info(info, gaps.as_ref());
                }
            }
            Err(e) => {
                let f = Fail::from(e);
                eprintln!("tlmbench: {}", f.1);
                worst = worst.max(Some(f.0));
            }
        }
    }
    worst.map_or(Ok(()), |code| Err(Fail(code, String::new())))
}

fn gaps(o: &Opened) -> Option<GapsJson> {
    let median = o.median_interval_ns()?;
    Some(GapsJson { median_interval_ns: median, count: o.breaks(DEFAULT_FACTOR).len() })
}

fn print_info(info: &RecordingInfo, gaps: Option<&GapsJson>) {
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
    match (&info.time, gaps) {
        (TimeSource::Column { .. }, Some(g)) => println!(
            "  Gaps      {} over 4 × the median interval ({})",
            group(g.count),
            format_duration(g.median_interval_ns)
        ),
        (TimeSource::Column { .. }, None) => println!("  Gaps      not indexed yet (tlmbench cache build)"),
        _ => println!("  Gaps      none (fixed rate)"),
    }
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

fn cache(action: CacheAction, common: &Common) -> Result<(), Fail> {
    let store = store(common);
    match action {
        CacheAction::Build { files, quiet } => {
            for path in &files {
                let o = store.open(path, common.descriptor.as_deref())?;
                let started = std::time::Instant::now();
                let show = !quiet && std::io::stderr().is_terminal();
                let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into());
                o.build(&mut |done, total| {
                    if show {
                        eprint!("\r{name}: {:.0}%", 100.0 * done as f64 / total.max(1) as f64);
                        let _ = std::io::stderr().flush();
                    }
                })?;
                if show {
                    eprint!("\r");
                }
                let bytes = o.recording().info().samples as f64 * o.recording().info().channels.len() as f64;
                println!(
                    "{}: built in {:.2} s ({:.0}M samples/s), key {}",
                    path.display(),
                    started.elapsed().as_secs_f64(),
                    bytes / started.elapsed().as_secs_f64().max(1e-9) / 1e6,
                    o.key()
                );
            }
            Ok(())
        }
        CacheAction::List { json } => {
            let entries = store.entries()?;
            if json {
                for e in &entries {
                    println!("{}", serde_json::to_string(e).unwrap());
                }
            } else {
                println!("{}", store.root().display());
                for e in &entries {
                    let state = if e.building { "  (build not finished)" } else { "" };
                    let name = e.path.file_name().unwrap_or_default().to_string_lossy();
                    println!("  {:8} {name}  {:>10}{state}", e.kind, size(e.bytes));
                }
                let total: u64 = entries.iter().map(|e| e.bytes).sum();
                println!("  {} entries, {}", entries.len(), size(total));
            }
            Ok(())
        }
        CacheAction::Clear => {
            let removed = store.clear()?;
            println!("Removed {} cache entries from {}", removed.len(), store.root().display());
            Ok(())
        }
    }
}

fn query(what: Query, common: &Common) -> Result<(), Fail> {
    let store = store(common);
    let open = |p: &Pick| -> Result<(Opened, Vec<usize>), Fail> {
        let o = store.open(&p.file, common.descriptor.as_deref())?;
        let chans = channels(&o, &p.channels)?;
        Ok((o, chans))
    };
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let w = |out: &mut std::io::BufWriter<std::io::StdoutLock>, line: String| {
        writeln!(out, "{line}").map_err(|e| Fail(2, e.to_string()))
    };
    match what {
        Query::View { pick, span, width, gap_factor, coarse } => {
            let (o, chans) = open(&pick)?;
            let (t0, t1) = span_of(&o, &span)?;
            w(&mut out, "channel,time_ns,value".into())?;
            let precision = if coarse { Precision::Coarse } else { Precision::Exact };
            let factors = vec![gap_factor; chans.len()];
            for v in o.views(&chans, Viewport { t0, t1, width_px: width.max(1) }, &factors, precision) {
                let name = &o.recording().info().channels[v.channel].name;
                for (t, y) in v.trace.t.iter().zip(&v.trace.y) {
                    let y = if y.is_nan() { String::new() } else { y.to_string() };
                    w(&mut out, format!("{},{t},{y}", csv_field(name)))?;
                }
            }
        }
        Query::Values { pick, at } => {
            let (o, chans) = open(&pick)?;
            let t = time_arg(&o, &at)?;
            w(&mut out, "channel,index,time_ns,value".into())?;
            if let Some((i, ts, values)) = o.values_at(&chans, t) {
                for (c, v) in chans.iter().zip(values) {
                    let name = &o.recording().info().channels[*c].name;
                    let v = if v.is_nan() { String::new() } else { v.to_string() };
                    w(&mut out, format!("{},{i},{ts},{v}", csv_field(name)))?;
                }
            }
        }
        Query::Stats { pick, span } => {
            let (o, chans) = open(&pick)?;
            if o.status() != Status::Built {
                o.build(&mut |_, _| {})?;
            }
            let (t0, t1) = span_of(&o, &span)?;
            w(&mut out, "channel,count,min,min_time_ns,max,max_time_ns,mean,std".into())?;
            let num = |v: f64| if v.is_nan() { String::new() } else { v.to_string() };
            for (&c, s) in chans.iter().zip(o.stats(&chans, t0, t1).expect("built above")) {
                let name = &o.recording().info().channels[c].name;
                let at = |m: Option<(f64, usize)>| {
                    m.map_or((String::new(), String::new()), |(v, i)| (num(v), o.recording().time_at(i).to_string()))
                };
                let ((mn, mnt), (mx, mxt)) = (at(s.min), at(s.max));
                w(
                    &mut out,
                    format!("{},{},{mn},{mnt},{mx},{mxt},{},{}", csv_field(name), s.count, num(s.mean), num(s.std)),
                )?;
            }
        }
    }
    out.flush().map_err(|e| Fail(2, e.to_string()))
}

/// Channel numbers from names or numbers; all when none are given.
fn channels(o: &Opened, picks: &[String]) -> Result<Vec<usize>, Fail> {
    let chans = &o.recording().info().channels;
    if picks.is_empty() {
        return Ok((0..chans.len()).collect());
    }
    picks
        .iter()
        .map(|p| {
            chans
                .iter()
                .position(|c| &c.name == p)
                .or_else(|| p.parse::<usize>().ok().filter(|&i| i < chans.len()))
                .ok_or_else(|| Fail(3, format!("no channel {p:?} in {}", o.recording().info().path.display())))
        })
        .collect()
}

/// An ISO 8601 time, or seconds from the first sample.
fn time_arg(o: &Opened, s: &str) -> Result<Timestamp, Fail> {
    let first = o.recording().info().t_first.unwrap_or(0);
    parse_iso(s)
        .or_else(|| parse_decimal(s, 1_000_000_000).map(|d| first.saturating_add(d)))
        .ok_or_else(|| Fail(3, format!("{s:?} is not a time (ISO 8601, or seconds from the start)")))
}

fn span_of(o: &Opened, s: &Span) -> Result<(Timestamp, Timestamp), Fail> {
    let info = o.recording().info();
    let t0 = s.from.as_deref().map(|v| time_arg(o, v)).transpose()?.or(info.t_first).unwrap_or(0);
    let t1 = s.until.as_deref().map(|v| time_arg(o, v)).transpose()?.or(info.t_last.map(|t| t + 1)).unwrap_or(1);
    if t1 <= t0 {
        return Err(Fail(3, "--until must be after --from".into()));
    }
    Ok((t0, t1))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_owned() }
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b => format!("{:.0} KB", b as f64 / 1024.0),
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

fn record(
    endpoint: tlmbench_live::Endpoint,
    descriptor: &std::path::Path,
    out: Option<PathBuf>,
    seconds: Option<f64>,
    idle_end: Option<f64>,
    ring_mb: Option<usize>,
    json: bool,
) -> Result<(), Fail> {
    use std::time::{Duration, Instant};
    let text = std::fs::read_to_string(descriptor).map_err(|e| Fail(3, format!("{}: {e}", descriptor.display())))?;
    let desc = tlmbench_decode::descriptor::Descriptor::parse(&text)
        .map_err(|e| Fail(3, format!("{}: {e}", descriptor.display())))?;
    let root = out.unwrap_or_else(tlmbench_live::recordings_dir);
    let mut session = tlmbench_live::Session::create(&root).map_err(|e| Fail(2, format!("{}: {e}", root.display())))?;
    let opts = tlmbench_live::Options {
        ring_bytes: ring_mb.map(|mb| mb << 20),
        idle_end: idle_end.map(Duration::from_secs_f64),
        ..Default::default()
    };
    let source = session.add_source(endpoint, &desc, &opts).map_err(|e| Fail(2, e.to_string()))?;
    let file = source.path().to_owned();
    let (started, live) = (Instant::now(), std::io::stderr().is_terminal() && !json);
    let mut last = Instant::now();
    while !source.health().ended && seconds.is_none_or(|s| started.elapsed().as_secs_f64() < s) {
        std::thread::sleep(Duration::from_millis(50));
        if live && last.elapsed() >= Duration::from_secs(1) {
            last = Instant::now();
            let h = source.health();
            eprint!(
                "\r{} records, {} dropped, {} missing   ",
                h.records_written, h.records_dropped, h.sequence_missing
            );
        }
    }
    let h = source.stop();
    if live {
        eprintln!();
    }
    if json {
        let v = serde_json::json!({ "session": session.dir(), "file": file, "health": h });
        println!("{v}");
    } else {
        println!("{}", file.display());
        println!(
            "{} records in {:.1} s · {} dropped · {} missing, {} repeated by sequence · ring high water {:.0}%",
            h.records_written,
            h.seconds,
            h.records_dropped,
            h.sequence_missing,
            h.sequence_repeated,
            100.0 * h.ring_high_water_bytes as f64 / h.ring_bytes.max(1) as f64
        );
    }
    match h.error {
        Some(e) => Err(Fail(2, e)),
        None => Ok(()),
    }
}
