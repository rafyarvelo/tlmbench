//! `tlmbench`: the command line over the same core as the desktop app. It has `info`, `cache`, `query`, `record`
//! and `convert`; check, export html, validate, derive and config export land in later phases.
//!
//! It reads recordings and never writes beside them: the only writes are cache entries under the cache root
//! (`--cache`, `$TLMBENCH_CACHE`, or the OS cache dir shared with the app), `record`'s session folders (`--out`,
//! `$TLMBENCH_RECORDINGS`, or the app's Recordings folder), and `convert`'s files (`--out`).
//!
//! Exit codes: 0 done; 2 a file couldn't be read; 3 a configuration error (a bad descriptor or argument).

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

mod check;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use tlmbench_core::datetime::{format_duration, format_time, parse_decimal, parse_iso};
use tlmbench_core::gaps::DEFAULT_FACTOR;
use tlmbench_core::table::{Col, Filter, SortKey};
use tlmbench_core::{RecordingInfo, TimeSource, Timestamp, Viewport};
use tlmbench_store::{Opened, Precision, Request, Status, Store};

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
    /// The packet type to read, for a packet file with several (default: all in `info` and `convert`)
    #[arg(long = "type", global = true)]
    part: Option<String>,
    /// Import profile (JSON from the app cache, or TOML from `.tlmbench/profiles/`) for CSV files: names, units, types,
    /// the time column and the rate
    #[arg(long, global = true)]
    profile: Option<PathBuf>,
    /// Cache folder (default: $TLMBENCH_CACHE, else the OS cache dir)
    #[arg(long, global = true)]
    cache: Option<PathBuf>,
    /// A `.tlmbench/` folder to read the shared configuration from instead of the nearest one above each input
    /// (repeat; later ones win): recipes, check sets, decoders and import profiles
    #[arg(long, global = true)]
    config: Vec<PathBuf>,
    /// Add the derived channels of the recipes marked automatic (from --recipes, else the nearest `.tlmbench/recipes`
    /// above the file) as channels of the recording
    #[arg(long, global = true)]
    derived: bool,
    /// A recipe file (TOML) or a folder of them (repeat; later ones win a name)
    #[arg(long, global = true)]
    recipes: Vec<PathBuf>,
    /// A derived channel of this one expression, `name=expression` (repeat), as channels of the recording
    #[arg(long = "expr", global = true)]
    exprs: Vec<String>,
    /// Your plugins folder (default: `plugins/` in the app's data folder). WASM decoders and alignment plugins in it,
    /// and in the `plugins/` of each --config folder, are loaded
    #[arg(long, global = true)]
    plugins: Option<PathBuf>,
    /// Turn on a native plugin of your plugins folder, by file name (repeat). Native plugins run with your rights, so
    /// each stays off until named
    #[arg(long = "native-plugin", global = true)]
    native_plugins: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// The plugins found in your plugins folder and the --config folders, with tier, version and whether they loaded
    Plugins {
        /// JSON instead of a table
        #[arg(long)]
        json: bool,
        /// Try one plugin (a `.wasm` file) instead of listing: what it says about itself and, with a file, what it
        /// decodes (schema, samples, time range) or why it can't. For writing a plugin
        #[arg(long)]
        check: Option<PathBuf>,
        /// The file to decode with --check
        file: Option<PathBuf>,
        #[command(flatten)]
        common: Common,
    },
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
    /// What a CSV file's import dialog would show: its columns as detected, the first rows, the time column and
    /// rate, and the saved import profile for its header (JSON)
    Inspect {
        file: PathBuf,
        #[command(flatten)]
        common: Common,
    },
    /// Evaluate one expression over a recording, given as `--expr "sqrt(accel_x^2 + accel_y^2)"` (no name): a series
    /// as CSV (time_ns, value) on stdout, or one number
    Derive {
        file: PathBuf,
        #[command(flatten)]
        span: Span,
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
    /// Write recordings as CSV, Parquet, Arrow IPC, HDF5 or MAT v7.3: one file per input in --out, plus
    /// <name>.channels.csv
    Convert {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Output format
        #[arg(long, value_enum)]
        to: ToFormat,
        /// Folder for the output files (created if missing)
        #[arg(long)]
        out: PathBuf,
        /// Channels by name, `*` matching any text (repeat, or separate with commas; default: all)
        #[arg(long = "channels", short = 'c', value_delimiter = ',')]
        channels: Vec<String>,
        #[command(flatten)]
        span: Span,
        /// One row per step of this rate instead of per sample (`100Hz`, `2kHz`, or a number in Hz); each channel
        /// holds its last value
        #[arg(long, conflicts_with = "layout")]
        resample: Option<String>,
        /// wide: a column per channel; long (CSV, Parquet, Arrow): one row per sample and channel
        #[arg(long, value_enum, default_value_t = LayoutArg::Wide)]
        layout: LayoutArg,
        /// How times are written (auto: exact; seconds for relative times, ISO 8601 for dates in CSV,
        /// nanoseconds in Parquet and Arrow)
        #[arg(long, value_enum, default_value_t = TimeArg::Auto)]
        time: TimeArg,
        /// Stored values in their own type, without scale and offset
        #[arg(long)]
        raw: bool,
        /// CSV: the field delimiter
        #[arg(long, default_value_t = ',')]
        delimiter: char,
        /// CSV: units in a second header row instead of `name [unit]`
        #[arg(long)]
        units_row: bool,
        /// Files converted at once
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        /// One JSON object per file written
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        common: Common,
    },
    /// Run check sets on recordings and report: exit 0 all pass, 1 a check failed, 2 a file or check couldn't run, 3 a
    /// configuration error (the highest applies)
    Check(check::Args),
    /// Lint a `.tlmbench/` folder (the nearest one above the current directory by default): every file that doesn't
    /// read, and recipes and checks whose expressions don't parse. Exit 0 clean, 3 problems
    Validate {
        /// The `.tlmbench/` folder, or the folder that holds one
        dir: Option<PathBuf>,
    },
    /// Work with the shared configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
        #[command(flatten)]
        common: Common,
    },
    /// Record a live source into a new session folder until it ends (or --seconds pass); prints its stream health
    Record {
        /// tcp://host:port (connects to it), udp://host:port (listens on it), serial://port?baud=rate (a serial port,
        /// 115200 baud when not given), or file:///path (follows a growing CSV or .tlm from its start; only read)
        endpoint: tlmbench_live::Endpoint,
        /// The frames' format descriptor (TOML); sockets and serial ports need one, files bring their own
        #[arg(long)]
        descriptor: Option<PathBuf>,
        /// Where the session folder goes (default: $TLMBENCH_RECORDINGS, else the app's Recordings folder)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Stop after this many seconds
        #[arg(long)]
        seconds: Option<f64>,
        /// UDP: stop once no datagram has come for this many seconds; a file: once it hasn't grown for that long
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

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ToFormat {
    Csv,
    Parquet,
    Arrow,
    Hdf5,
    /// MAT v7.3 (HDF5 underneath; opens in MATLAB with `load`)
    Mat,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum LayoutArg {
    Wide,
    Long,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum TimeArg {
    Auto,
    Seconds,
    Iso,
    Ns,
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
        /// Draw every sample: up to this many in the span come back raw, not reduced (0: only up to four per pixel
        /// column)
        #[arg(long, default_value_t = 0)]
        raw_max: u32,
        /// Connect the line across gaps (time jumps, missing values) instead of breaking it; adds a `dashed` column
        /// (1: the segment ending at this point is a connected gap)
        #[arg(long)]
        connect_gaps: bool,
    },
    /// Each channel's sample nearest a time, as CSV: channel, index, time_ns, value
    Values {
        #[command(flatten)]
        pick: Pick,
        /// The time, as for --from of `query view`
        #[arg(long)]
        at: String,
    },
    /// The rows of the span a table shows, filtered and sorted over the whole span, as CSV: row, time_ns, then the
    /// channels (`row` is the sample index)
    Rows {
        #[command(flatten)]
        pick: Pick,
        #[command(flatten)]
        span: Span,
        /// A column filter, `channel=expression` (repeat; all must pass): `> 1.5`, `1..2`, `blank`, joined with
        /// `and` / `or`. The channel `time` filters the time in ns
        #[arg(long = "filter")]
        filters: Vec<String>,
        /// A sort key: a channel (or `time`), `:desc` after it for descending, as `volts:desc` (repeat for several;
        /// the first is the main key)
        #[arg(long = "sort")]
        sort: Vec<String>,
        /// Rows to skip, after sorting
        #[arg(long, default_value_t = 0)]
        offset: usize,
        /// Rows to print at most
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },
    /// Exact count, min, max, mean and standard deviation over a span, as CSV (builds the pyramid first)
    Stats {
        #[command(flatten)]
        pick: Pick,
        #[command(flatten)]
        span: Span,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Write the app cache's shared kinds to a `.tlmbench/` folder, so CI uses exactly what you tested with: recipes,
    /// check sets, import profiles, decoders and time maps, and (as the app last saved them, `own.json` in the cache)
    /// event categories, widgets, icons, layouts and the events of the recordings it had open. Only changed files are
    /// written
    Export {
        /// The folder to write (it becomes a `.tlmbench/`; created if missing)
        #[arg(long)]
        out: PathBuf,
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
        Command::Plugins { json, check, file, common } => match check {
            Some(plugin) => check_plugin(&plugin, file.as_deref(), &common),
            None => plugins(json, &common),
        },
        Command::Info { files, json, common } => info(&files, json, &common),
        Command::Inspect { file, common } => inspect(&file, &common),
        Command::Derive { file, span, common } => derive(&file, &span, &common),
        Command::Cache { action, common } => cache(action, &common),
        Command::Query { what, common } => query(what, &common),
        Command::Convert {
            files,
            to,
            out,
            channels,
            span,
            resample,
            layout,
            time,
            raw,
            delimiter,
            units_row,
            jobs,
            json,
            common,
        } => {
            let c = ConvertArgs { to, out, channels, span, resample, layout, time, raw, delimiter, units_row, json };
            convert(&files, &c, jobs, &common)
        }
        Command::Check(a) => check::run(a),
        Command::Validate { dir } => validate(dir),
        Command::Config { action, common } => config(action, &common),
        Command::Record { endpoint, descriptor, out, seconds, idle_end, ring_mb, json } => {
            record(endpoint, descriptor.as_deref(), out, seconds, idle_end, ring_mb, json)
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

/// `tlmbench validate`.
fn validate(dir: Option<PathBuf>) -> Result<(), Fail> {
    let here = dir.unwrap_or_else(|| PathBuf::from("."));
    let folder = if here.file_name().is_some_and(|n| n == ".tlmbench") {
        here.clone()
    } else if here.join(".tlmbench").is_dir() {
        here.join(".tlmbench")
    } else {
        tlmbench_store::config::nearest(&here)
            .ok_or_else(|| Fail(3, format!("{}: no .tlmbench/ folder", here.display())))?
    };
    let problems = tlmbench_store::config::validate(&folder);
    for p in &problems {
        eprintln!("{}/{}: {}", folder.display(), p.path, p.why);
    }
    if problems.is_empty() {
        println!("{}: ok", folder.display());
        Ok(())
    } else {
        Err(Fail(
            3,
            format!("{} problem{} in {}", problems.len(), if problems.len() == 1 { "" } else { "s" }, folder.display()),
        ))
    }
}

/// `tlmbench config export`.
fn config(action: ConfigAction, common: &Common) -> Result<(), Fail> {
    match action {
        ConfigAction::Export { out } => {
            let root = tlmbench_decode::cache::cache_root(common.cache.as_deref());
            let own = tlmbench_store::config::load_mirror(&root);
            let written = tlmbench_store::config::export(&root, &own, &out)?;
            for w in &written {
                println!("{}", out.join(w).display());
            }
            eprintln!(
                "{} file{} written to {}",
                written.len(),
                if written.len() == 1 { "" } else { "s" },
                out.display()
            );
            Ok(())
        }
    }
}

/// `tlmbench plugins --check PLUGIN [FILE]`: the author's harness.
fn check_plugin(plugin: &std::path::Path, file: Option<&std::path::Path>, common: &Common) -> Result<(), Fail> {
    use tlmbench_plugin::Loaded;
    let loaded = tlmbench_plugin::load_wasm(plugin).map_err(|e| Fail(2, e))?;
    let (info, decoder) = match &loaded {
        Loaded::Decoder(d) => (tlmbench_decode::plugin::DecoderPlugin::info(d).clone(), Some(d)),
        Loaded::Estimator(e) => (e.info().clone(), None),
    };
    println!(
        "{} {} ({}), {}",
        info.name,
        info.version,
        if decoder.is_some() { "decoder" } else { "alignment" },
        plugin.display()
    );
    if let Some(d) = &info.description {
        println!("  {d}");
    }
    let (Some(dec), Some(file)) = (decoder, file) else {
        if decoder.is_some() {
            println!("  Give a file to see what it decodes.");
        }
        return Ok(());
    };
    use tlmbench_decode::plugin::DecoderPlugin;
    let head = tlmbench_decode::read_head(file, 4096).map_err(tlmbench_store::Error::from)?;
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let by_name = info.files.iter().any(|p| tlmbench_decode::descriptor::name_matches(p, &name));
    println!(
        "  claims this file: by name {}, by sniff {}",
        if by_name { "yes" } else { "no" },
        if dec.sniff(&head, &name) { "yes" } else { "no" }
    );
    let root = tlmbench_decode::cache::cache_root(common.cache.as_deref());
    let started = std::time::Instant::now();
    let rec = tlmbench_decode::plugin::open_with(file, &root, dec).map_err(|e| Fail(2, e.to_string()))?;
    let i = tlmbench_core::Recording::info(&rec);
    println!(
        "  decoded {} samples in {:.2} s (cached: a second run is instant)",
        i.samples,
        started.elapsed().as_secs_f64()
    );
    for c in &i.channels {
        println!("  channel {:<20} {:<4} {}", c.name, c.dtype, c.unit.as_deref().unwrap_or(""));
    }
    match (i.t_first, i.t_last) {
        (Some(a), Some(b)) => println!("  time {a} .. {b} ns ({:?})", i.time),
        _ => println!("  no samples"),
    }
    Ok(())
}

fn plugins(json: bool, common: &Common) -> Result<(), Fail> {
    let found = load_plugins(common);
    if json {
        println!("{}", serde_json::to_string_pretty(&found).expect("json prints"));
        return Ok(());
    }
    if found.is_empty() {
        println!(
            "No plugins. Put .wasm files in {}",
            own_plugins(common).map(|d| d.display().to_string()).unwrap_or_default()
        );
    }
    for f in &found {
        use tlmbench_plugin::{Kind, Status, Tier};
        let kind = match f.kind {
            Some(Kind::Decoder) => "decoder",
            Some(Kind::Estimator) => "alignment",
            None => "?",
        };
        let tier = if f.tier == Tier::Wasm { "wasm" } else { "native" };
        let state = match &f.status {
            Status::Loaded => "loaded".to_string(),
            Status::Off => "off (turn on with --native-plugin)".to_string(),
            Status::Untrusted => "waiting for your OK".to_string(),
            Status::Failed(why) => format!("failed: {why}"),
        };
        println!(
            "{:<24} {:<8} {:<7} {:<10} {}{}",
            f.file,
            f.version.as_deref().unwrap_or("-"),
            tier,
            kind,
            state,
            if f.shared { "  [from --config]" } else { "" }
        );
    }
    Ok(())
}

fn inspect(file: &std::path::Path, common: &Common) -> Result<(), Fail> {
    let i = tlmbench_decode::csv::inspect_file(file).map_err(tlmbench_store::Error::from)?;
    let root = tlmbench_decode::cache::cache_root(common.cache.as_deref());
    let header: Vec<String> = i.columns.iter().map(|c| c.source.clone()).collect();
    let mut found = tlmbench_decode::profile::find(&root, &header);
    if found.exact.is_none() {
        // the nearest `.tlmbench/profiles/` (or --config) shares one for this header
        found.exact =
            tlmbench_store::config::shared_profile(&tlmbench_store::config::dirs_for(file, &common.config), &header);
    }
    let mut v = serde_json::to_value(&i).expect("inspections serialize");
    v["profile"] = serde_json::to_value(&found).expect("profiles serialize");
    println!("{}", serde_json::to_string_pretty(&v).expect("json prints"));
    Ok(())
}

/// Your plugins folder: `--plugins`, else the app's (the same one Settings › Plugins opens).
fn own_plugins(common: &Common) -> Option<PathBuf> {
    common.plugins.clone().or_else(|| dirs::data_dir().map(|d| d.join("io.github.rafyarvelo.tlmbench").join("plugins")))
}

/// Loads the plugins once (nothing starts when no folder has any).
fn load_plugins(common: &Common) -> Vec<tlmbench_plugin::Found> {
    static LOADED: std::sync::OnceLock<Vec<tlmbench_plugin::Found>> = std::sync::OnceLock::new();
    LOADED
        .get_or_init(|| {
            let own = own_plugins(common).filter(|d| d.is_dir());
            let shared: Vec<PathBuf> = common.config.iter().map(|d| d.join("plugins")).filter(|d| d.is_dir()).collect();
            if own.is_none() && shared.is_empty() {
                return Vec::new();
            }
            // a folder named with --config is the person's own choice: its plugins are trusted
            let trusted: Vec<String> = shared.iter().map(|d| d.to_string_lossy().into_owned()).collect();
            tlmbench_plugin::install(own.as_deref(), &shared, &trusted, &common.native_plugins).found
        })
        .clone()
}

fn store(common: &Common) -> Store {
    tlmbench_decode::descriptor::set_config_dirs(common.config.clone());
    load_plugins(common);
    Store::new(common.cache.as_deref()).with_profile(common.profile.as_deref().map(load_profile))
}

/// Reads `--profile`; a bad file is a configuration error.
fn load_profile(path: &std::path::Path) -> tlmbench_decode::profile::ImportProfile {
    let fail = |msg: String| -> ! {
        eprintln!("tlmbench: {}: {msg}", path.display());
        std::process::exit(3)
    };
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(e.to_string()));
    // a profile shared in `.tlmbench/profiles/` is TOML, the app cache's is JSON
    if path.extension().is_some_and(|x| x == "toml") {
        return tlmbench_store::config::profile_from_toml(&text)
            .unwrap_or_else(|e| fail(format!("not an import profile: {e}")));
    }
    serde_json::from_str(&text).unwrap_or_else(|e| fail(format!("not an import profile: {e}")))
}

#[derive(Serialize)]
struct InfoJson<'a> {
    #[serde(flatten)]
    info: &'a RecordingInfo,
    duration_ns: Option<i64>,
    /// None until the gap index is built (`tlmbench cache build`).
    gaps: Option<GapsJson>,
    /// The derived channels `--derived` and `--expr` add.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    derived: &'a [tlmbench_core::ChannelInfo],
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
        match open_parts(&store, path, common) {
            Ok(parts) => {
                for (j, o) in parts.iter().enumerate() {
                    let info = o.recording().info();
                    let gaps = gaps(o);
                    let derived = match derived_of(&store, o, common) {
                        Ok(d) => d.and_then(|d| d.opened),
                        Err(Fail(code, msg)) => {
                            eprintln!("tlmbench: {msg}");
                            worst = worst.max(Some(code));
                            None
                        }
                    };
                    let extra: &[tlmbench_core::ChannelInfo] =
                        derived.as_ref().map_or(&[], |d| &d.recording().info().channels);
                    if json {
                        let duration_ns = info.t_first.zip(info.t_last).map(|(a, b)| b - a);
                        println!(
                            "{}",
                            serde_json::to_string(&InfoJson { info, duration_ns, gaps, derived: extra }).unwrap()
                        );
                    } else {
                        if k + j > 0 {
                            println!();
                        }
                        print_info(info, gaps.as_ref(), extra);
                    }
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

/// What a file holds: one recording, or one per packet type (just `--type`'s).
fn open_parts(store: &Store, path: &std::path::Path, common: &Common) -> Result<Vec<Opened>, tlmbench_store::Error> {
    match &common.part {
        Some(part) => Ok(vec![store.open_part(path, common.descriptor.as_deref(), Some(part))?]),
        None => store.open_all(path, common.descriptor.as_deref()),
    }
}

fn gaps(o: &Opened) -> Option<GapsJson> {
    let median = o.median_interval_ns()?;
    Some(GapsJson { median_interval_ns: median, count: o.breaks(DEFAULT_FACTOR).len() })
}

fn print_info(info: &RecordingInfo, gaps: Option<&GapsJson>, derived: &[tlmbench_core::ChannelInfo]) {
    match &info.part {
        Some(part) => println!("{} · {part}", info.path.display()),
        None => println!("{}", info.path.display()),
    }
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
    if !derived.is_empty() {
        println!("  Derived   {}", derived.len());
        let width = derived.iter().map(|c| c.name.chars().count()).max().unwrap_or(0);
        for c in derived {
            let unit = c.unit.as_deref().map(|u| format!("  {u}")).unwrap_or_default();
            println!("    ƒ {:width$}  {}{unit}", c.name, c.dtype);
        }
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
    let open = |p: &Pick| Targets::open(&store, &p.file, common);
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let w = |out: &mut std::io::BufWriter<std::io::StdoutLock>, line: String| {
        writeln!(out, "{line}").map_err(|e| Fail(2, e.to_string()))
    };
    match what {
        Query::View { pick, span, width, gap_factor, coarse, raw_max, connect_gaps } => {
            let targets = open(&pick)?;
            w(&mut out, if connect_gaps { "channel,time_ns,value,dashed" } else { "channel,time_ns,value" }.into())?;
            for (o, chans) in targets.groups(&pick.channels)? {
                let (t0, t1) = span_of(o, &span)?;
                let precision = if coarse { Precision::Coarse } else { Precision::Exact };
                let factors = vec![gap_factor; chans.len()];
                let req = Request {
                    channels: chans,
                    gap_factors: factors,
                    vp: Viewport { t0, t1, width_px: width.max(1), raw_max },
                    connect_gaps,
                };
                for v in o.view_batch(std::slice::from_ref(&req), precision).pop().unwrap_or_default() {
                    let name = &o.recording().info().channels[v.channel].name;
                    for (i, (t, y)) in v.trace.t.iter().zip(&v.trace.y).enumerate() {
                        let y = if y.is_nan() { String::new() } else { y.to_string() };
                        let dash = if connect_gaps {
                            format!(",{}", u8::from(v.dashed.contains(&(i as u32))))
                        } else {
                            String::new()
                        };
                        w(&mut out, format!("{},{t},{y}{dash}", csv_field(name)))?;
                    }
                }
            }
        }
        Query::Values { pick, at } => {
            let targets = open(&pick)?;
            w(&mut out, "channel,index,time_ns,value".into())?;
            for (o, chans) in targets.groups(&pick.channels)? {
                let t = time_arg(o, &at)?;
                if let Some((i, ts, values)) = o.values_at(&chans, t) {
                    for (c, v) in chans.iter().zip(values) {
                        let name = &o.recording().info().channels[*c].name;
                        let v = if v.is_nan() { String::new() } else { v.to_string() };
                        w(&mut out, format!("{},{i},{ts},{v}", csv_field(name)))?;
                    }
                }
            }
        }
        Query::Rows { pick, span, filters, sort, offset, limit } => {
            let targets = open(&pick)?;
            let mut groups = targets.groups(&pick.channels)?;
            if groups.len() != 1 {
                return Err(Fail(
                    3,
                    "a table is of one recording's channels: pick raw channels, or derived ones, not both".into(),
                ));
            }
            let (o, chans) = groups.pop().unwrap();
            let (t0, t1) = span_of(o, &span)?;
            let col = |name: &str| -> Result<Col, Fail> {
                if name == "time" { Ok(Col::Time) } else { Ok(Col::Channel(channels(o, &[name.to_owned()])?[0])) }
            };
            let filters = filters
                .iter()
                .map(|f| {
                    let (c, expr) =
                        f.split_once('=').ok_or_else(|| Fail(3, format!("--filter {f:?}: use channel=expression")))?;
                    let parsed = Filter::parse(expr).map_err(|e| Fail(3, format!("--filter {f:?}: {e}")))?;
                    parsed.map(|p| col(c.trim()).map(|c| (c, p))).transpose()
                })
                .collect::<Result<Vec<_>, Fail>>()?
                .into_iter()
                .flatten()
                .collect();
            let sort = sort
                .iter()
                .map(|k| {
                    let (name, descending) = match k.strip_suffix(":desc") {
                        Some(c) => (c, true),
                        None => (k.strip_suffix(":asc").unwrap_or(k), false),
                    };
                    col(name).map(|col| SortKey { col, descending })
                })
                .collect::<Result<Vec<_>, Fail>>()?;
            let Some((first, rows)) = o.select_rows((t0, t1), filters, sort, &|_| true) else {
                return Err(Fail(2, "too many rows to sort in one go".into()));
            };
            let names: Vec<String> = chans.iter().map(|&c| csv_field(&o.recording().info().channels[c].name)).collect();
            w(&mut out, format!("row,time_ns,{}", names.join(",")))?;
            for &id in rows.ids.iter().skip(offset).take(limit) {
                let i = first + id as usize;
                let vals = (0..chans.len()).map(|k| {
                    let v = o.recording().value_at(chans[k], i, &Default::default());
                    if v.is_nan() { String::new() } else { v.to_string() }
                });
                w(&mut out, format!("{i},{},{}", o.recording().time_at(i), vals.collect::<Vec<_>>().join(",")))?;
            }
            eprintln!("{} rows match", rows.ids.len());
        }
        Query::Stats { pick, span } => {
            let targets = open(&pick)?;
            w(&mut out, "channel,count,min,min_time_ns,max,max_time_ns,mean,std".into())?;
            let num = |v: f64| if v.is_nan() { String::new() } else { v.to_string() };
            for (o, chans) in targets.groups(&pick.channels)? {
                if o.status() != Status::Built {
                    o.build(&mut |_, _| {})?;
                }
                let (t0, t1) = span_of(o, &span)?;
                for (&c, s) in chans.iter().zip(o.stats(&chans, t0, t1).expect("built above")) {
                    let name = &o.recording().info().channels[c].name;
                    let at = |m: Option<(f64, usize)>| {
                        m.map_or((String::new(), String::new()), |(v, i)| {
                            (num(v), o.recording().time_at(i).to_string())
                        })
                    };
                    let ((mn, mnt), (mx, mxt)) = (at(s.min), at(s.max));
                    w(
                        &mut out,
                        format!("{},{},{mn},{mnt},{mx},{mxt},{},{}", csv_field(name), s.count, num(s.mean), num(s.std)),
                    )?;
                }
            }
        }
    }
    out.flush().map_err(|e| Fail(2, e.to_string()))
}

#[derive(Debug)]
struct ConvertArgs {
    to: ToFormat,
    out: PathBuf,
    channels: Vec<String>,
    span: Span,
    resample: Option<String>,
    layout: LayoutArg,
    time: TimeArg,
    raw: bool,
    delimiter: char,
    units_row: bool,
    json: bool,
}

#[derive(Serialize)]
struct ConvertJson<'a> {
    input: &'a std::path::Path,
    output: &'a std::path::Path,
    channels_file: &'a std::path::Path,
    rows: u64,
    bytes: u64,
}

fn convert(files: &[PathBuf], a: &ConvertArgs, jobs: usize, common: &Common) -> Result<(), Fail> {
    use tlmbench_export::{Format, Layout, Options, TimeFormat};
    let format = match a.to {
        ToFormat::Csv => Format::Csv,
        ToFormat::Parquet => Format::Parquet,
        ToFormat::Arrow => Format::Arrow,
        ToFormat::Hdf5 => Format::Hdf5,
        ToFormat::Mat => Format::Mat,
    };
    if !a.delimiter.is_ascii() {
        return Err(Fail(3, "--delimiter must be one ASCII character".into()));
    }
    let resample = a.resample.as_deref().map(rate_arg).transpose()?;
    // every output is named after its input: two inputs of one name would overwrite each other
    let mut names = std::collections::HashSet::new();
    let outputs: Vec<PathBuf> = files
        .iter()
        .map(|f| {
            let stem = f.file_stem().unwrap_or_default().to_string_lossy();
            let out = a.out.join(format!("{stem}.{}", format.extension()));
            if !names.insert(out.clone()) {
                return Err(Fail(3, format!("two inputs would both write {}", out.display())));
            }
            if std::fs::canonicalize(&out).ok().is_some_and(|o| std::fs::canonicalize(f).is_ok_and(|i| i == o)) {
                return Err(Fail(3, format!("{} would overwrite its input", out.display())));
            }
            Ok(out)
        })
        .collect::<Result<_, _>>()?;
    std::fs::create_dir_all(&a.out).map_err(|e| Fail(2, format!("{}: {e}", a.out.display())))?;
    let store = store(common);
    let one = |input: &PathBuf, output: &PathBuf| -> Result<String, Fail> {
        let mut lines = Vec::new();
        for o in open_parts(&store, input, common)? {
            let rec = o.recording();
            // a packet type is a recording of its own: <name>.<type>.<ext>
            let output = match &rec.info().part {
                Some(part) => {
                    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                    output.with_file_name(format!("{stem}.{part}.{}", format.extension()))
                }
                None => output.clone(),
            };
            let derived = derived_of(&store, &o, common)?.and_then(|d| d.opened);
            let chans = select(&o, derived.as_ref(), &a.channels)?;
            let (t0, t1) = span_of(&o, &a.span)?;
            let mut opts = Options::new(format, chans, rec.lower_bound(t0), rec.lower_bound(t1));
            opts.layout = match a.layout {
                LayoutArg::Wide => Layout::Wide,
                LayoutArg::Long => Layout::Long,
            };
            opts.time = match a.time {
                TimeArg::Auto => TimeFormat::Auto,
                TimeArg::Seconds => TimeFormat::Seconds,
                TimeArg::Iso => TimeFormat::Iso,
                TimeArg::Ns => TimeFormat::Ns,
            };
            opts.resample_hz = resample;
            opts.raw = a.raw;
            opts.delimiter = a.delimiter as u8;
            opts.units_row = a.units_row;
            opts.rate_hz = match rec.info().time {
                TimeSource::Rate { hz } => Some(hz),
                _ => o.median_interval_ns().filter(|&m| m > 0).map(|m| 1e9 / m as f64),
            };
            let extra: Vec<&dyn tlmbench_core::Recording> = derived.iter().map(|d| d.recording()).collect();
            let w = tlmbench_export::export_with(rec, &extra, &opts, &output).map_err(|e| Fail(2, e.to_string()))?;
            lines.push(if a.json {
                let j =
                    ConvertJson { input, output: &w.path, channels_file: &w.manifest, rows: w.rows, bytes: w.bytes };
                serde_json::to_string(&j).unwrap()
            } else {
                format!(
                    "{} → {} ({} rows, {})",
                    input.display(),
                    w.path.display(),
                    group(w.rows as usize),
                    size(w.bytes)
                )
            });
        }
        Ok(lines.join("\n"))
    };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let worst = std::sync::Mutex::new(None::<u8>);
    std::thread::scope(|s| {
        for _ in 0..jobs.clamp(1, files.len().max(1)) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(input) = files.get(k) else { break };
                    match one(input, &outputs[k]) {
                        Ok(line) => println!("{line}"),
                        Err(Fail(code, msg)) => {
                            eprintln!("tlmbench: {msg}");
                            let mut w = worst.lock().unwrap();
                            *w = (*w).max(Some(code));
                        }
                    }
                }
            });
        }
    });
    worst.into_inner().unwrap().map_or(Ok(()), |code| Err(Fail(code, String::new())))
}

/// A rate: `100Hz`, `2kHz`, `0.5 Hz` or a plain number of Hz.
fn rate_arg(s: &str) -> Result<f64, Fail> {
    let t = s.trim().to_ascii_lowercase();
    let (num, mul) = match t.strip_suffix("khz") {
        Some(n) => (n, 1e3),
        None => (t.strip_suffix("hz").unwrap_or(&t), 1.0),
    };
    num.trim()
        .parse::<f64>()
        .ok()
        .map(|v| v * mul)
        .filter(|v| *v > 0.0 && v.is_finite())
        .ok_or_else(|| Fail(3, format!("{s:?} is not a rate (100Hz, 2kHz, or a number in Hz)")))
}

/// Channels whose names match any of `patterns` (`*` matches any text), in the recording's order; all when none
/// are given. A pattern that matches nothing is an error.
fn select(o: &Opened, derived: Option<&Opened>, patterns: &[String]) -> Result<Vec<usize>, Fail> {
    let chans: Vec<&tlmbench_core::ChannelInfo> = o
        .recording()
        .info()
        .channels
        .iter()
        .chain(derived.iter().flat_map(|d| d.recording().info().channels.iter()))
        .collect();
    let patterns: Vec<&str> = patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if patterns.is_empty() {
        return Ok((0..chans.len()).collect());
    }
    if let Some(p) = patterns.iter().find(|p| !chans.iter().any(|c| glob(p, &c.name))) {
        return Err(Fail(3, format!("no channel matches {p:?} in {}", o.recording().info().path.display())));
    }
    Ok((0..chans.len()).filter(|&i| patterns.iter().any(|p| glob(p, &chans[i].name))).collect())
}

/// Whether `name` matches `pattern`, where `*` matches any text.
fn glob(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || !name[first.len()..].ends_with(last) || name.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for p in &parts[1..parts.len() - 1] {
        match rest.find(p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    true
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
    descriptor: Option<&std::path::Path>,
    out: Option<PathBuf>,
    seconds: Option<f64>,
    idle_end: Option<f64>,
    ring_mb: Option<usize>,
    json: bool,
) -> Result<(), Fail> {
    use std::time::{Duration, Instant};
    let desc = match descriptor {
        Some(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| Fail(3, format!("{}: {e}", p.display())))?;
            Some(
                tlmbench_decode::descriptor::Descriptor::parse(&text)
                    .map_err(|e| Fail(3, format!("{}: {e}", p.display())))?,
            )
        }
        None => None,
    };
    let root = out.unwrap_or_else(tlmbench_live::recordings_dir);
    let mut session = tlmbench_live::Session::create(&root).map_err(|e| Fail(2, format!("{}: {e}", root.display())))?;
    let opts = tlmbench_live::Options {
        ring_bytes: ring_mb.map(|mb| mb << 20),
        idle_end: idle_end.map(Duration::from_secs_f64),
        ..Default::default()
    };
    let source = session.add_source(endpoint, desc.as_ref(), &opts).map_err(|e| Fail(3, e.to_string()))?;
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
    let _ = session.update(0, &source);
    let h = source.stop();
    let _ = session.update_files(0, &h.files);
    if live {
        eprintln!();
    }
    if json {
        let v = serde_json::json!({ "session": session.dir(), "file": h.files.first(), "health": h });
        println!("{v}");
    } else {
        for f in &h.files {
            println!("{}", f.display());
        }
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

// ---------------------------------------------------------------------------------------------------------------
// Derived channels

/// The recipes `--derived`, `--recipes` and `--expr` ask for, for the recording at `path`: the library's automatic ones
/// (with `--derived`), then the one-offs. One-offs are marked so a mistake in one is an error, not a skipped recipe.
fn recipes_for(path: &std::path::Path, common: &Common) -> Result<(Vec<tlmbench_expr::Recipe>, usize), Fail> {
    recipe_library(path, common, common.derived, false)
}

/// The recipe library for `path` (`load`: read it even without `--derived`; `every`: not only the automatic ones), then
/// the `--expr` one-offs.
fn recipe_library(
    path: &std::path::Path,
    common: &Common,
    load: bool,
    every: bool,
) -> Result<(Vec<tlmbench_expr::Recipe>, usize), Fail> {
    let mut out: Vec<tlmbench_expr::Recipe> = Vec::new();
    if load {
        let mut files = Vec::new();
        let mut roots = common.recipes.clone();
        if roots.is_empty() {
            roots.extend(
                tlmbench_store::config::dirs_for(path, &common.config)
                    .iter()
                    .map(|d| d.join("recipes"))
                    .filter(|d| d.is_dir()),
            );
        }
        for r in &roots {
            if r.is_dir() {
                let mut v: Vec<PathBuf> = std::fs::read_dir(r)
                    .map_err(|e| Fail(3, format!("{}: {e}", r.display())))?
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                    .collect();
                v.sort();
                files.extend(v);
            } else {
                files.push(r.clone());
            }
        }
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| Fail(3, format!("{}: {e}", f.display())))?;
            for r in tlmbench_expr::recipe::from_toml(&text).map_err(|e| Fail(3, format!("{}: {e}", f.display())))? {
                if let Some(old) = out.iter_mut().find(|o| o.name == r.name) {
                    *old = r;
                } else {
                    out.push(r);
                }
            }
        }
        if !every {
            out.retain(|r| r.automatic);
        }
    }
    let library = out.len();
    for e in &common.exprs {
        let (name, expr) = e.split_once('=').ok_or_else(|| Fail(3, format!("--expr {e:?}: use name=expression")))?;
        let (name, expr) = (name.trim(), expr.trim());
        if name.is_empty() {
            return Err(Fail(3, format!("--expr {e:?}: the channel needs a name")));
        }
        out.retain(|r| r.name != name);
        out.push(tlmbench_expr::Recipe {
            name: name.into(),
            expr: expr.into(),
            unit: None,
            category: None,
            decimals: None,
            automatic: true,
        });
    }
    Ok((out, library))
}

/// The derived channels of `base`, when the flags ask for any. Library recipes that don't apply are noted on stderr;
/// a `--expr` that can't be computed is an error.
fn derived_of(store: &Store, base: &Opened, common: &Common) -> Result<Option<tlmbench_store::Derived>, Fail> {
    if !common.derived && common.exprs.is_empty() {
        return Ok(None);
    }
    let (recipes, library) = recipes_for(&base.recording().info().path, common)?;
    let d = store.open_derived(base, &recipes)?;
    for (name, e) in &d.skipped {
        if recipes.iter().position(|r| &r.name == name).is_some_and(|k| k >= library) {
            return Err(Fail(3, format!("--expr {name}: {} (at {})", e.msg, e.at)));
        }
        eprintln!("tlmbench: recipe {name:?} doesn't apply to {}: {}", base.recording().info().path.display(), e.msg);
    }
    Ok(Some(d))
}

/// A recording and its derived channels, as one set of channels to pick from.
struct Targets {
    base: Opened,
    derived: Option<Opened>,
}

impl Targets {
    fn open(store: &Store, path: &std::path::Path, common: &Common) -> Result<Self, Fail> {
        let base = store.open_part(path, common.descriptor.as_deref(), common.part.as_deref())?;
        let derived = derived_of(store, &base, common)?.and_then(|d| d.opened);
        Ok(Self { base, derived })
    }

    /// Channels by name or number (numbers count the recording's channels, then the derived ones), grouped by the
    /// recording that holds them; everything when none are given.
    fn groups(&self, picks: &[String]) -> Result<Vec<(&Opened, Vec<usize>)>, Fail> {
        let all: Vec<&Opened> = std::iter::once(&self.base).chain(self.derived.iter()).collect();
        let names: Vec<(usize, usize, &str)> = all
            .iter()
            .enumerate()
            .flat_map(|(r, o)| {
                o.recording().info().channels.iter().enumerate().map(move |(c, ch)| (r, c, ch.name.as_str()))
            })
            .collect();
        let picked: Vec<(usize, usize)> = if picks.is_empty() {
            names.iter().map(|&(r, c, _)| (r, c)).collect()
        } else {
            picks
                .iter()
                .map(|p| {
                    names
                        .iter()
                        .position(|(.., name)| name == p)
                        .or_else(|| p.parse::<usize>().ok().filter(|&i| i < names.len()))
                        .map(|i| (names[i].0, names[i].1))
                        .ok_or_else(|| {
                            Fail(3, format!("no channel {p:?} in {}", self.base.recording().info().path.display()))
                        })
                })
                .collect::<Result<_, _>>()?
        };
        Ok((0..all.len())
            .map(|r| (all[r], picked.iter().filter(|(pr, _)| *pr == r).map(|&(_, c)| c).collect::<Vec<_>>()))
            .filter(|(_, c)| !c.is_empty())
            .collect())
    }
}

fn derive(file: &std::path::Path, span: &Span, common: &Common) -> Result<(), Fail> {
    let [expr] = &common.exprs[..] else {
        return Err(Fail(3, "give one expression: --expr \"battery_v * current_a\"".into()));
    };
    let store = store(common);
    let o = store.open_part(file, common.descriptor.as_deref(), common.part.as_deref())?;
    let rec = o.recording();
    let tree = tlmbench_expr::parse(expr).map_err(|e| Fail(3, format!("{} (at {})", e.msg, e.at)))?;
    let env = tlmbench_store::RecEnv::new(rec);
    let (t0, t1) = span_of(&o, span)?;
    let (from, until) = (rec.lower_bound(t0), rec.lower_bound(t1));
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut failed = None;
    let mut header = false;
    let number = tlmbench_expr::run(&tree, &env, tlmbench_expr::CHUNK, &mut |start, s| {
        if failed.is_some() {
            return;
        }
        let mut write = || -> std::io::Result<()> {
            if !header {
                writeln!(out, "time_ns,value")?;
                header = true;
            }
            for (k, v) in s.v.iter().enumerate() {
                let i = start + k;
                if i >= from && i < until {
                    let v = if v.is_nan() { String::new() } else { v.to_string() };
                    writeln!(out, "{},{v}", rec.time_at(i))?;
                }
            }
            Ok(())
        };
        failed = write().err();
    })
    .map_err(|e| Fail(3, format!("{} (at {})", e.msg, e.at)))?;
    if let Some(e) = failed {
        return Err(Fail(2, e.to_string()));
    }
    if let Some(x) = number {
        writeln!(out, "{x}").map_err(|e| Fail(2, e.to_string()))?;
    }
    out.flush().map_err(|e| Fail(2, e.to_string()))
}
