//! `tlmbench check`: runs check sets (and the rules in them) on recordings, the same engine and cache as the app's
//! folder run, and writes JUnit, CSV, JSON and HTML reports.
//!
//! Exit codes (the highest that applies): 0 everything passed; 1 a check failed (a warning too with `--fail-on warn`);
//! 2 a file couldn't be read or a check couldn't run, a channel is missing (`--allow-cant-run` leaves these out);
//! 3 a configuration error: a set that isn't found, a mistake in an expression, a bad argument.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use tlmbench_core::datetime::{format_duration, format_time};
use tlmbench_store::checks::{self, Check, CheckSet, Kind, Outcome, Sets, Verdict};

use super::{Common, Fail, recipe_library, store};

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    Junit,
    Csv,
    Json,
    Html,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailOn {
    Fail,
    Warn,
}

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Recordings, or folders of them (the files in the folder, in name order)
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// The set to run, by name (repeat). Looked for in --sets, else the nearest `.tlmbench/checks` above the first
    /// path, else the app's own sets
    #[arg(long = "set")]
    sets: Vec<String>,
    /// A folder of set files (TOML), or one (repeat)
    #[arg(long = "sets-from")]
    sets_from: Vec<PathBuf>,
    /// Also run the sets each CSV file's import profile names, so CI and the app agree
    #[arg(long)]
    sets_from_profile: bool,
    /// Reports to write into --out
    #[arg(long, value_enum, value_delimiter = ',')]
    report: Vec<Report>,
    /// Where the reports go (created if missing)
    #[arg(long)]
    out: Option<PathBuf>,
    /// Files checked at once
    #[arg(long, default_value_t = 1)]
    jobs: usize,
    /// `warn` counts a warning as a failure
    #[arg(long, value_enum, default_value_t = FailOn::Fail)]
    fail_on: FailOn,
    /// A check that can't run (a channel is missing) doesn't fail the build
    #[arg(long)]
    allow_cant_run: bool,
    #[command(flatten)]
    common: Common,
}

/// One file's results.
struct FileRes {
    path: PathBuf,
    error: Option<String>,
    sets: Vec<(CheckSet, checks::SetResult)>,
    t_first: i64,
}

fn discover(paths: &[PathBuf], common: &Common) -> Result<Vec<PathBuf>, Fail> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p)
                .map_err(|e| Fail(2, format!("{}: {e}", p.display())))?
                .flatten()
                .map(|e| e.path())
                .filter(|f| f.is_file())
                .filter(|f| {
                    let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let ext = f.extension().and_then(|e| e.to_str()).map(str::to_lowercase);
                    !name.starts_with('.')
                        && match ext.as_deref() {
                            Some("csv" | "tsv" | "txt" | "tlm" | "parquet") => true,
                            Some("bin" | "dat") => common.descriptor.is_some(),
                            _ => false,
                        }
                })
                .collect();
            v.sort();
            out.extend(v);
        } else if p.is_file() {
            out.push(p.clone());
        } else {
            return Err(Fail(2, format!("{}: no such file or folder", p.display())));
        }
    }
    Ok(out)
}

fn load_sets(args: &Args, first: &Path) -> Result<Vec<CheckSet>, Fail> {
    let mut found: Vec<CheckSet> = Vec::new();
    fn add(found: &mut Vec<CheckSet>, sets: Sets) {
        for s in sets.list() {
            if !found.iter().any(|f| f.name == s.name) {
                found.push(s);
            }
        }
    }
    for p in &args.sets_from {
        if p.is_dir() {
            let s = Sets::in_dir(p);
            if let Some((f, why)) = s.problems().into_iter().next() {
                return Err(Fail(3, format!("{}: {why}", f.display())));
            }
            add(&mut found, s);
        } else {
            let text = std::fs::read_to_string(p).map_err(|e| Fail(3, format!("{}: {e}", p.display())))?;
            let set = checks::from_toml(&text).map_err(|e| Fail(3, format!("{}: {e}", p.display())))?;
            if !found.iter().any(|f| f.name == set.name) {
                found.push(set);
            }
        }
    }
    let abs = std::fs::canonicalize(first).unwrap_or_else(|_| first.to_owned());
    if let Some(dir) = abs.ancestors().skip(1).map(|d| d.join(".tlmbench").join("checks")).find(|d| d.is_dir()) {
        add(&mut found, Sets::in_dir(&dir));
    }
    add(&mut found, Sets::new(&tlmbench_decode::cache::cache_root(args.common.cache.as_deref())));
    Ok(found)
}

/// The names of the sets a CSV file's saved (or given) import profile attaches.
fn profile_sets(path: &Path, common: &Common) -> Vec<String> {
    if let Some(p) = &common.profile {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        return serde_json::from_str::<tlmbench_decode::profile::ImportProfile>(&text)
            .map(|p| p.check_sets)
            .unwrap_or_default();
    }
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv") || e.eq_ignore_ascii_case("tsv")) {
        return Vec::new();
    }
    let Ok(i) = tlmbench_decode::csv::inspect_file(path) else { return Vec::new() };
    let header: Vec<String> = i.columns.iter().map(|c| c.source.clone()).collect();
    let root = tlmbench_decode::cache::cache_root(common.cache.as_deref());
    tlmbench_decode::profile::find(&root, &header).exact.map(|p| p.check_sets).unwrap_or_default()
}

pub fn run(args: Args) -> Result<(), Fail> {
    if args.sets.is_empty() && !args.sets_from_profile {
        return Err(Fail(3, "name a set with --set \"Flight acceptance\" (or use --sets-from-profile)".into()));
    }
    if !args.report.is_empty() && args.out.is_none() {
        return Err(Fail(3, "--report needs --out".into()));
    }
    let files = discover(&args.paths, &args.common)?;
    if files.is_empty() {
        return Err(Fail(2, "no recordings found".into()));
    }
    let known = load_sets(&args, &files[0])?;
    let lookup = |name: &str| known.iter().find(|s| s.name == name).cloned();
    for n in &args.sets {
        if lookup(n).is_none() {
            let have: Vec<&str> = known.iter().map(|s| s.name.as_str()).collect();
            return Err(Fail(3, format!("no check set called {n:?} (found: {})", have.join(", "))));
        }
    }
    let store = store(&args.common);
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<FileRes>>> = files.iter().map(|_| Mutex::new(None)).collect();
    let config_error = Mutex::new(None::<String>);
    std::thread::scope(|sc| {
        for _ in 0..args.jobs.clamp(1, files.len()) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(path) = files.get(i) else { break };
                    let mut res = FileRes { path: path.clone(), error: None, sets: Vec::new(), t_first: 0 };
                    let mut names = args.sets.clone();
                    if args.sets_from_profile {
                        for n in profile_sets(path, &args.common) {
                            if !names.contains(&n) {
                                names.push(n);
                            }
                        }
                    }
                    match store.open_part(path, args.common.descriptor.as_deref(), args.common.part.as_deref()) {
                        Err(e) => res.error = Some(e.to_string()),
                        Ok(base) => {
                            res.t_first = base.recording().info().t_first.unwrap_or(0);
                            match recipe_library(path, &args.common, true, true) {
                                Err(Fail(_, m)) => res.error = Some(m),
                                Ok((recipes, _)) => {
                                    for n in &names {
                                        let Some(set) = lookup(n) else {
                                            config_error
                                                .lock()
                                                .unwrap()
                                                .get_or_insert(format!("no check set called {n:?}"));
                                            continue;
                                        };
                                        match store.run_checks(&base, &set, &recipes) {
                                            Ok(r) => res.sets.push((set, r)),
                                            Err(e) => res.error = Some(e.to_string()),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    *slots[i].lock().unwrap() = Some(res);
                }
            });
        }
    });
    if let Some(m) = config_error.into_inner().unwrap() {
        return Err(Fail(3, m));
    }
    let results: Vec<FileRes> =
        slots.into_iter().map(|s| s.into_inner().unwrap().expect("every file was done")).collect();

    print_summary(&results);
    if let Some(out) = &args.out {
        std::fs::create_dir_all(out).map_err(|e| Fail(2, format!("{}: {e}", out.display())))?;
        for r in &args.report {
            let (name, text) = match r {
                Report::Junit => ("junit.xml", junit(&results, args.fail_on)),
                Report::Csv => ("results.csv", csv(&results)),
                Report::Json => ("results.json", json(&results)),
                Report::Html => ("report.html", html(&results)),
            };
            let p = out.join(name);
            std::fs::write(&p, text).map_err(|e| Fail(2, format!("{}: {e}", p.display())))?;
        }
    }

    let mut code = 0u8;
    for f in &results {
        if f.error.is_some() {
            code = code.max(2);
        }
        for (_, r) in &f.sets {
            for o in &r.outcomes {
                code = code.max(match o.verdict {
                    Verdict::Fail => 1,
                    Verdict::Warn if args.fail_on == FailOn::Warn => 1,
                    Verdict::Cant if !args.allow_cant_run => 2,
                    Verdict::Err => 3,
                    _ => 0,
                });
            }
        }
    }
    if code == 0 { Ok(()) } else { Err(Fail(code, String::new())) }
}

// ---------------------------------------------------------------------------------------------------------------
// Words

fn word(v: Verdict) -> &'static str {
    match v {
        Verdict::Fail => "Fail",
        Verdict::Warn => "Warn",
        Verdict::Err => "Error",
        Verdict::Cant => "Can't run",
        Verdict::Pass => "Pass",
        Verdict::Na => "Not applicable",
        Verdict::Mark => "Events",
        Verdict::Off => "Off",
    }
}

fn num(x: f64) -> String {
    if x.abs() >= 1000.0 || x == x.trunc() {
        format!("{x:.0}")
    } else {
        format!("{x:.3}").trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// "Fail · 3× · 4.2 s · worst 5310 at +01:12:40", "max 16.2 (limit 20)", or why it didn't run.
fn describe(o: &Outcome) -> String {
    let mut parts = vec![word(o.verdict).to_owned()];
    match o.verdict {
        Verdict::Fail | Verdict::Warn | Verdict::Mark if o.count > 0 => {
            parts.push(format!("{}×", o.count));
            parts.push(format_duration((o.secs * 1e9) as i64));
            if let (Some(w), Some(t)) = (o.worst, o.worst_t) {
                parts.push(format!("worst {} at {}", num(w), format_time(t)));
            }
            if let Some(r) = o.runs.first() {
                parts.push(format!("first at {}", format_time(r.start)));
            }
        }
        _ => {
            if let (Some(v), Some(f)) = (o.value, &o.function) {
                let lim = o.limit.map(|l| format!(" (limit {})", num(l))).unwrap_or_default();
                parts.push(format!("{f} {}{lim}", num(v)));
            }
            if !o.message.is_empty() {
                parts.push(o.message.clone());
            }
        }
    }
    parts.join(" · ")
}

fn rows<'a>(f: &'a FileRes) -> impl Iterator<Item = (&'a CheckSet, &'a Check, &'a Outcome)> {
    f.sets.iter().flat_map(|(s, r)| s.checks.iter().zip(&r.outcomes).map(move |(k, o)| (s, k, o)))
}

fn print_summary(results: &[FileRes]) {
    for f in results {
        println!("{}", f.path.display());
        if let Some(e) = &f.error {
            println!("  can't be checked: {e}");
        }
        for (s, r) in &f.sets {
            println!("  {} · {}", s.name, word(r.rollup.verdict));
            for g in &r.rollup.groups {
                let what = if g.verdict == Verdict::Mark {
                    format!("{} events", g.events)
                } else {
                    word(g.verdict).to_owned()
                };
                println!("    {}{} · {what}", g.name, if g.info { " (info only)" } else { "" });
                for (k, o) in s.checks.iter().zip(&r.outcomes).filter(|(k, _)| k.group == g.name) {
                    if !matches!(o.verdict, Verdict::Pass | Verdict::Na | Verdict::Off) {
                        println!("      {} · {}", k.name, describe(o));
                    }
                }
            }
        }
    }
    let (mut fail, mut warn, mut cant) = (0, 0, 0);
    for f in results {
        for (_, _, o) in rows(f) {
            match o.verdict {
                Verdict::Fail => fail += 1,
                Verdict::Warn => warn += 1,
                Verdict::Cant | Verdict::Err => cant += 1,
                _ => {}
            }
        }
    }
    println!("{} files · {fail} failed · {warn} warnings · {cant} couldn't run", results.len());
}

// ---------------------------------------------------------------------------------------------------------------
// Reports

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn junit(results: &[FileRes], fail_on: FailOn) -> String {
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>\n");
    for f in results {
        let cases: Vec<_> = rows(f).collect();
        let failures = cases
            .iter()
            .filter(|(_, _, o)| o.verdict == Verdict::Fail || (o.verdict == Verdict::Warn && fail_on == FailOn::Warn))
            .count();
        let errors = cases.iter().filter(|(_, _, o)| matches!(o.verdict, Verdict::Cant | Verdict::Err)).count()
            + f.error.is_some() as usize;
        x += &format!(
            "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{failures}\" errors=\"{errors}\">\n",
            esc(&f.path.display().to_string()),
            cases.len() + f.error.is_some() as usize
        );
        if let Some(e) = &f.error {
            x += &format!("    <testcase name=\"open\"><error message=\"{}\"/></testcase>\n", esc(e));
        }
        for (s, k, o) in cases {
            let d = esc(&describe(o));
            x += &format!("    <testcase classname=\"{}.{}\" name=\"{}\"", esc(&s.name), esc(&k.group), esc(&k.name));
            x += &match o.verdict {
                Verdict::Fail => format!("><failure message=\"{d}\"/></testcase>\n"),
                Verdict::Warn if fail_on == FailOn::Warn => {
                    format!("><failure type=\"warning\" message=\"{d}\"/></testcase>\n")
                }
                Verdict::Warn => format!("><system-out>{d}</system-out></testcase>\n"),
                Verdict::Cant | Verdict::Err => format!("><error message=\"{d}\"/></testcase>\n"),
                Verdict::Na | Verdict::Off => format!("><skipped message=\"{d}\"/></testcase>\n"),
                Verdict::Mark => format!("><system-out>{d}</system-out></testcase>\n"),
                Verdict::Pass => "/>\n".to_owned(),
            };
        }
        x += "  </testsuite>\n";
    }
    x + "</testsuites>\n"
}

fn field(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_owned() }
}

fn csv(results: &[FileRes]) -> String {
    let mut x = String::from(
        "file,set,group,check,kind,severity,verdict,count,seconds,worst,worst_time,value,limit,first,message\n",
    );
    for f in results {
        let file = f.path.display().to_string();
        if let Some(e) = &f.error {
            x += &format!("{},,,,,,error,,,,,,,,{}\n", field(&file), field(e));
        }
        for (s, k, o) in rows(f) {
            let opt = |v: Option<f64>| v.map(num).unwrap_or_default();
            x += &[
                field(&file),
                field(&s.name),
                field(&k.group),
                field(&k.name),
                if k.kind == Kind::Rule { "rule" } else { "check" }.into(),
                format!("{:?}", k.severity).to_lowercase(),
                format!("{:?}", o.verdict).to_lowercase(),
                o.count.to_string(),
                format!("{:.3}", o.secs),
                opt(o.worst),
                o.worst_t.map(format_time).unwrap_or_default(),
                opt(o.value),
                opt(o.limit),
                o.runs.first().map(|r| format_time(r.start)).unwrap_or_default(),
                field(&o.message),
            ]
            .join(",");
            x.push('\n');
        }
    }
    x
}

fn json(results: &[FileRes]) -> String {
    let v: Vec<serde_json::Value> = results
        .iter()
        .map(|f| {
            serde_json::json!({
                "file": f.path.display().to_string(),
                "error": f.error,
                "sets": f.sets.iter().map(|(s, r)| serde_json::json!({
                    "set": s.name,
                    "verdict": r.rollup.verdict,
                    "rollup": r.rollup,
                    "checks": s.checks.iter().zip(&r.outcomes).map(|(k, o)| serde_json::json!({
                        "group": k.group, "kind": k.kind, "severity": k.severity, "result": o,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&v).expect("results serialize") + "\n"
}

fn html(results: &[FileRes]) -> String {
    let class = |v: Verdict| format!("{v:?}").to_lowercase();
    let mut x = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>Check report</title><style>\
body{font:13px/1.4 system-ui,sans-serif;margin:24px;color:#1b1f24;background:#fff}h1{font-size:15px}h2{font-size:13px;margin:24px 0 8px}\
table{border-collapse:collapse}td,th{border:1px solid #d0d7de;padding:4px 8px;text-align:left;vertical-align:top}\
.fail{background:#ffd7d5}.warn{background:#fff1c2}.pass{background:#d3f5dc}.err,.cant{background:#e6e8eb}.na,.off{color:#6e7781}.mark{background:#dbeafe}\
@media (prefers-color-scheme:dark){body{background:#0d1117;color:#e6edf3}td,th{border-color:#30363d}.fail{background:#5b1d1d}.warn{background:#4d3b00}.pass{background:#12351f}.err,.cant{background:#262c34}.mark{background:#10294d}}\
</style></head><body><h1>Check report</h1>",
    );
    // the grid: files × groups, one per set
    let mut sets: Vec<&str> = Vec::new();
    for f in results {
        for (s, _) in &f.sets {
            if !sets.contains(&s.name.as_str()) {
                sets.push(&s.name);
            }
        }
    }
    for name in sets {
        let mut groups: Vec<&str> = Vec::new();
        for f in results {
            for (s, r) in &f.sets {
                if s.name == name {
                    for g in &r.rollup.groups {
                        if !groups.contains(&g.name.as_str()) {
                            groups.push(&g.name);
                        }
                    }
                }
            }
        }
        x += &format!("<h2>{}</h2><table><tr><th>File</th><th>Result</th>", esc(name));
        for g in &groups {
            x += &format!("<th>{}</th>", esc(g));
        }
        x += "</tr>";
        for f in results {
            let Some((_, r)) = f.sets.iter().find(|(s, _)| s.name == name) else { continue };
            x += &format!(
                "<tr><td>{}</td><td class=\"{}\">{}</td>",
                esc(&f.path.display().to_string()),
                class(r.rollup.verdict),
                word(r.rollup.verdict)
            );
            for g in &groups {
                match r.rollup.groups.iter().find(|x| x.name == *g) {
                    Some(g) => x += &format!("<td class=\"{}\">{}</td>", class(g.verdict), word(g.verdict)),
                    None => x += "<td></td>",
                }
            }
            x += "</tr>";
        }
        x += "</table>";
    }
    x += "<h2>Checks that didn't pass</h2><table><tr><th>File</th><th>Set</th><th>Group</th><th>Check</th><th>Result</th></tr>";
    for f in results {
        if let Some(e) = &f.error {
            x += &format!(
                "<tr><td>{}</td><td colspan=\"3\"></td><td class=\"err\">{}</td></tr>",
                esc(&f.path.display().to_string()),
                esc(e)
            );
        }
        for (s, k, o) in rows(f) {
            if !matches!(o.verdict, Verdict::Pass | Verdict::Na | Verdict::Off) {
                x += &format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td></tr>",
                    esc(&f.path.display().to_string()),
                    esc(&s.name),
                    esc(&k.group),
                    esc(&k.name),
                    class(o.verdict),
                    esc(&describe(o))
                );
            }
        }
    }
    x + "</table></body></html>\n"
}
