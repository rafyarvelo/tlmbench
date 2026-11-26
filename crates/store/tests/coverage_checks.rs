//! The check engine's less-travelled paths (coverage, R6c.3, Nov 26): a set file that doesn't read, results as JSON,
//! roll-ups with nothing countable, a scope that reads labels and limits, rows that can't run or have nothing to look
//! at, live rows that fall back to a whole run, the library of sets in a folder, cached results, and the badges.

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;

use tlmbench_expr::{Env, Recipe, Series};
use tlmbench_store::checks::*;
use tlmbench_store::{ExprEnv, Store};

const HZ: f64 = 100.0;
const N: usize = 6000;
const T0: i64 = 1_000_000_000_000;

struct Rec {
    ch: HashMap<&'static str, Series>,
    n: Cell<usize>,
}

impl Env for Rec {
    fn series(&self, n: &str) -> Option<Series> {
        self.read(n, 0..self.n.get())
    }
    fn read(&self, n: &str, r: Range<usize>) -> Option<Series> {
        let r = r.start.min(self.n.get())..r.end.min(self.n.get());
        self.ch.get(n).map(|s| s.slice(r))
    }
    fn samples(&self) -> Option<usize> {
        Some(self.n.get())
    }
    fn hz(&self) -> Option<f64> {
        Some(HZ)
    }
    fn label(&self, c: &str, t: &str) -> Option<f64> {
        (c == "mode" && t == "Mission").then_some(2.0)
    }
    fn limits(&self, c: &str, alarm: bool) -> Option<(Option<f64>, Option<f64>)> {
        (c == "temp" && !alarm).then_some((None, Some(70.0)))
    }
}

/// `mode` is 2 from 10 s to 40 s; `armed` 1 from 5 s to 55 s; `temp` 60 with excursions of growing height and length
/// (so the worst value is in a later run); `blank` has no value at all; `wave` swings across zero; `gappy` is missing in
/// stretches.
fn rec() -> Rec {
    let f = |g: &dyn Fn(f64) -> f64| Series::rate((0..N).map(|i| g(i as f64 / HZ)).collect(), HZ);
    let mut ch = HashMap::new();
    ch.insert("rpm", f(&|t| t * 100.0));
    ch.insert("mode", f(&|t| if (10.0..40.0).contains(&t) { 2.0 } else { 0.0 }));
    ch.insert("armed", f(&|t| if (5.0..55.0).contains(&t) { 1.0 } else { 0.0 }));
    ch.insert(
        "temp",
        f(&|t| match t {
            t if (8.0..9.0).contains(&t) => 72.0,
            t if (20.0..22.0).contains(&t) => 75.0,
            t if (30.0..30.5).contains(&t) => 71.0,
            t if (45.0..48.0).contains(&t) => 80.0,
            _ => 60.0,
        }),
    );
    ch.insert("blank", f(&|_| f64::NAN));
    ch.insert("blink", Series::rate((0..N).map(|i| if i % 40 < 10 { f64::NAN } else { 50.0 }).collect(), HZ));
    ch.insert("wave", f(&|t| (t * 0.9).sin() * 10.0));
    ch.insert("gappy", f(&|t| if (t as usize) % 7 < 2 { f64::NAN } else { t }));
    Rec { ch, n: Cell::new(N) }
}

fn check(name: &str, expect: &str) -> Check {
    Check { name: name.into(), expect: expect.into(), enabled: true, group: "G".into(), ..Check::default() }
}

fn run(k: &Check) -> Outcome {
    run_check(&rec(), k, T0, &[])
}

fn set(name: &str, checks: Vec<Check>) -> CheckSet {
    CheckSet { schema: SCHEMA, name: name.into(), runs_on_any_file: false, info_groups: vec![], detail: None, checks }
}

#[test]
fn a_set_file_that_does_not_read_says_why_and_results_survive_json() {
    assert!(from_toml("[[check]\n").is_err());
    let e = from_toml("schema = 9\nname = \"x\"\n").unwrap_err();
    assert!(e.contains("check set schema 9"), "{e}");
    // a ranked order of verdicts, with the three that don't count at 0
    let ranks: Vec<u8> = [Verdict::Fail, Verdict::Warn, Verdict::Err, Verdict::Cant, Verdict::Pass, Verdict::Na]
        .iter()
        .map(|v| v.rank())
        .collect();
    assert_eq!(ranks, [5, 4, 3, 2, 1, 0]);
    assert_eq!((Verdict::Mark.rank(), Verdict::Off.rank()), (0, 0));
    // times are strings on the wire; one that isn't a number is refused
    let o = run(&check("t", "temp < 70"));
    assert!(o.worst_t.is_some());
    let text = serde_json::to_string(&o).unwrap();
    assert_eq!(serde_json::from_str::<Outcome>(&text).unwrap(), o);
    assert!(text.contains(&format!("\"worst_t\":\"{}\"", o.worst_t.unwrap())), "{text}");
    let bad = text.replace(&format!("\"{}\"", o.worst_t.unwrap()), "\"soon\"");
    assert!(serde_json::from_str::<Outcome>(&bad).is_err());
    let span = serde_json::to_string(&o.runs[0]).unwrap();
    assert!(serde_json::from_str::<Span>(&span.replace(&o.runs[0].start.to_string(), "x")).is_err());
    // an outcome without a worst time leaves the key out
    assert!(!serde_json::to_string(&run(&check("t", "temp < 90"))).unwrap().contains("worst_t"));
}

#[test]
fn a_roll_up_of_off_rows_or_only_rules_or_nothing_says_so() {
    let mut off = check("off", "temp < 70");
    off.enabled = false;
    let mut rule = check("rule", "temp > 70");
    rule.kind = Kind::Rule;
    rule.group = "Rules".into();
    let rolled = |checks: Vec<Check>| {
        let s = set("S", checks);
        run_set(&rec(), &s, T0, &[]).rollup
    };
    let r = rolled(vec![off.clone()]);
    assert_eq!((r.verdict, r.groups[0].verdict, r.applies), (Verdict::Na, Verdict::Off, false));
    let r = rolled(vec![rule.clone()]);
    assert_eq!(r.verdict, Verdict::Mark);
    assert!(r.events > 0 && r.groups[0].events == r.events, "{r:?}");
    let r = rolled(vec![]);
    assert_eq!((r.verdict, r.groups.len(), r.applies), (Verdict::Na, 0, false));
    // an off row beside a rule in one group: the rule's marks are the group's verdict
    let mut off_g = off;
    off_g.group = "Rules".into();
    let r = rolled(vec![off_g, rule]);
    assert_eq!((r.groups[0].verdict, r.verdict), (Verdict::Mark, Verdict::Mark));
}

#[test]
fn a_scope_reads_labels_and_limits_through_the_mask() {
    let mut label = check("in mission", "mode == 'Mission'");
    label.when = Some("armed > 0".into());
    // mode is Mission only from 10 s to 40 s, the scope is 5 s to 55 s: the check fails outside the mission
    let o = run(&label);
    assert_eq!(o.verdict, Verdict::Fail);
    assert!((o.secs - 20.0).abs() < 0.05, "{}", o.secs);
    let mut limit = check("under warn", "past_warn(temp) == 0");
    limit.when = Some("armed > 0".into());
    let o = run(&limit);
    assert_eq!(o.verdict, Verdict::Fail, "temp is past its warn limit in the scope");
    assert_eq!(o.count, 4);
}

#[test]
fn rows_that_cannot_run_or_have_nothing_to_look_at_say_why() {
    let msg = |k: &Check| {
        let o = run(k);
        (o.verdict, o.message)
    };
    assert_eq!(msg(&check("e", "  ")), (Verdict::Err, "nothing to expect yet".into()));
    let mut rule = check("r", "");
    rule.kind = Kind::Rule;
    assert_eq!(msg(&rule), (Verdict::Err, "no condition yet".into()));
    // a When that doesn't parse, or names a channel that isn't there, says it is the When
    let mut k = check("w", "temp < 70");
    k.when = Some("armed >".into());
    let (v, m) = msg(&k);
    assert!(v == Verdict::Err && m.starts_with("When: "), "{m}");
    k.when = Some("nothere > 0".into());
    let (v, m) = msg(&k);
    assert!(v == Verdict::Cant && m.starts_with("When: "), "{m}");
    // a When that is one number: false means never, true means everywhere
    k.when = Some("max(idle_nothing) > 0".into());
    k.when = Some("max(blank) > 0".into());
    assert_eq!(msg(&k).0, Verdict::Na);
    k.when = Some("max(rpm) > 0".into());
    assert_eq!(msg(&k).0, Verdict::Fail);
    k.when = Some("max(rpm) < 0".into());
    assert_eq!(msg(&k), (Verdict::Na, "When is never true".into()));
    // a When that is never true over time
    k.when = Some("rpm > 1e9".into());
    assert_eq!(msg(&k), (Verdict::Na, "its When never held".into()));
    // nothing but missing values: no samples
    assert_eq!(msg(&check("b", "blank < 5")), (Verdict::Na, "no samples".into()));
    let mut b = check("b", "blank < 5");
    b.when = Some("armed > 0".into());
    assert_eq!(msg(&b), (Verdict::Na, "its When never held".into()));
    assert_eq!(msg(&check("s", "max(blank) < 5")), (Verdict::Na, "no samples".into()));
    let mut s = check("s", "max(blank) < 5");
    s.when = Some("armed > 0".into());
    assert_eq!(msg(&s), (Verdict::Na, "no samples in scope".into()));
    // a rule given one number
    let mut r = check("r", "max(rpm) > 5");
    r.kind = Kind::Rule;
    assert_eq!(run(&r).verdict, Verdict::Err);
    // a summary reports its number, its function and its limit
    let o = run(&check("m", "max(temp) < 70"));
    assert_eq!(
        (o.verdict, o.value, o.function.as_deref(), o.limit),
        (Verdict::Fail, Some(80.0), Some("max"), Some(70.0))
    );
    // a comparison of two things has no limit to report
    let o = run(&check("m", "max(temp) < max(rpm)"));
    assert_eq!((o.verdict, o.limit), (Verdict::Pass, None));
}

/// The first sample time of a recording and the worst value across several violations: the furthest, from a later run.
#[test]
fn the_worst_value_is_the_furthest_across_runs_and_for_checks_that_hold_above() {
    let o = run(&check("t", "temp < 70"));
    assert_eq!((o.count, o.worst), (4, Some(80.0)));
    assert!(((o.worst_t.unwrap() - T0) as f64 / 1e9 - 45.0).abs() < 0.02);
    let o = run(&check("w", "wave > -9"));
    assert!(o.worst.unwrap() < -9.0);
    // missing breaks a check that says so, and only inside its scope
    let mut g = check("g", "gappy > 0");
    g.missing = Missing::Fail;
    g.when = Some("armed > 0".into());
    let o = run(&g);
    assert_eq!(o.verdict, Verdict::Fail);
    assert!(o.count > 3, "{}", o.count);
}

/// Live rows that fall back to a whole run, or can't run: each matches what a whole run says at every step.
#[test]
fn live_rows_of_every_shape_match_a_whole_run_at_each_step() {
    let mut off = check("off", "temp < 70");
    off.enabled = false;
    let mut when_bad = check("when bad", "temp < 70");
    when_bad.when = Some("nothere > 0".into());
    let mut rule_multi = check("rule", "temp > 70");
    rule_multi.kind = Kind::Rule;
    let mut scoped = check("scoped", "temp < 70");
    scoped.when = Some("armed > 0".into());
    let mut scoped_summary = check("scoped summary", "max(temp) < 90");
    scoped_summary.when = Some("armed > 0".into());
    let mut high = check("lower worst", "wave > -9");
    high.grace = 0.05;
    let mut gaps = check("gaps", "gappy > 0");
    gaps.missing = Missing::Fail;
    let mut when_num = check("when number", "temp < 70");
    when_num.when = Some("max(rpm) > 100".into());
    let mut when_fails = check("when fails", "temp < 70");
    when_fails.when = Some("mode == 'Bogus'".into());
    let mut no_data_scoped = check("no data scoped", "blank < 1");
    no_data_scoped.when = Some("armed > 0".into());
    let mut gaps_scoped = check("gaps scoped", "gappy > 0");
    gaps_scoped.missing = Missing::Fail;
    gaps_scoped.when = Some("armed > 0".into());
    let mut scoped_window = check("scoped window", "rolling_mean(gappy, 50) < 30");
    scoped_window.when = Some("rolling_mean(gappy, 20) > 5".into());
    let rows = vec![
        off,
        check("empty", ""),
        check("no such label", "mode == 'Bogus'"),
        check("no limits", "past_warn(rpm) == 0"),
        when_fails,
        check("unknown", "nothere < 1"),
        check("broken", "temp <"),
        when_bad,
        check("running total", "cumsum(rpm) < 1e9"),
        check("no data", "blank < 1"),
        no_data_scoped,
        check("multi", "temp < 70"),
        rule_multi,
        scoped,
        scoped_summary,
        high,
        gaps,
        when_num,
        check("wave", "wave < 9"),
        check("long window", "rolling_mean(wave, 5000) < 9"),
        // a missing value between violations holds the worst value of the stretch back until the run goes on
        check("tail up", "rpm < blink"),
        gaps_scoped,
        check("tail down", "temp > gappy"),
        check("gappy window", "rolling_mean(gappy, 50) < 30"),
        scoped_window,
    ];
    let all = set("Live", rows);
    let g = rec();
    let mut state = LiveSet::default();
    let mut n = 0;
    let mut steps: Vec<usize> = (1..40).collect();
    steps.extend((40..N).step_by(173));
    steps.push(N);
    for step in steps {
        n = step;
        g.n.set(n);
        let live = run_set_live(&g, &all, T0, &[], &mut state, "");
        let whole = run_set(&g, &all, T0, &[]);
        for (a, b) in live.outcomes.iter().zip(&whole.outcomes) {
            assert_eq!(a, b, "{} at {n} samples", a.name);
        }
        assert_eq!(live.rollup, whole.rollup, "at {n}");
    }
    assert_eq!(n, N);
    // the verdicts at the end are the ones the rows' wording says
    let last = run_set_live(&g, &all, T0, &[], &mut state, "");
    let v = |name: &str| last.outcomes.iter().find(|o| o.name == name).unwrap().verdict;
    assert_eq!(
        [v("off"), v("empty"), v("unknown"), v("broken"), v("when bad"), v("no data"), v("multi"), v("rule")],
        [
            Verdict::Off,
            Verdict::Err,
            Verdict::Cant,
            Verdict::Err,
            Verdict::Cant,
            Verdict::Na,
            Verdict::Fail,
            Verdict::Mark
        ]
    );
    // what the live rows say when the expression fails while it runs
    let msg = |name: &str| last.outcomes.iter().find(|o| o.name == name).unwrap().message.clone();
    assert_eq!(msg("no such label"), "mode has no value called 'Bogus'");
    assert_eq!(msg("no limits"), "rpm has no warn limits");
    assert_eq!(msg("when fails"), "When: mode has no value called 'Bogus'");
    assert_eq!([v("no such label"), v("no limits"), v("when fails")], [Verdict::Err; 3]);
}

/// A file of recipes standing for channels: a row that names one is found by the cache key.
fn csv(dir: &std::path::Path) -> std::path::PathBuf {
    let mut t = String::from("t,rpm,temp\n");
    for i in 0..300 {
        t.push_str(&format!("{:.2},{},{}\n", i as f64 / 100.0, i * 10, if (100..150).contains(&i) { 75 } else { 60 }));
    }
    let p = dir.join("run.csv");
    std::fs::write(&p, t).unwrap();
    p
}

#[test]
fn the_library_of_sets_keeps_files_by_name_and_lists_the_ones_that_do_not_read() {
    let d = tempfile::tempdir().unwrap();
    let sets = Sets::in_dir(&d.path().join("sets"));
    assert!(sets.list().is_empty() && sets.problems().is_empty());
    assert!(sets.save(&set(" ", vec![]), None).is_err(), "a set needs a name");
    sets.save(&set("Flight!", vec![check("a", "rpm < 1")]), None).unwrap();
    // a name that slugs the same gets its own file
    sets.save(&set("flight?", vec![check("b", "rpm < 2")]), None).unwrap();
    let names: Vec<String> = sets.list().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["Flight!", "flight?"]);
    assert!(d.path().join("sets/flight.toml").exists() && d.path().join("sets/flight-2.toml").exists());
    // saving again keeps the file; a rename replaces the old name's file
    sets.save(&set("Flight!", vec![check("a", "rpm < 3")]), Some("Flight!")).unwrap();
    assert_eq!(sets.get("Flight!").unwrap().checks[0].expect, "rpm < 3");
    sets.save(&set("Renamed", vec![]), Some("Flight!")).unwrap();
    assert!(sets.get("Flight!").is_none() && sets.get("Renamed").is_some());
    // a name with no letters has a file of its own too
    sets.save(&set("???", vec![]), None).unwrap();
    assert!(d.path().join("sets/set.toml").exists());
    // a file that doesn't read is a problem, and not a set
    std::fs::write(d.path().join("sets/broken.toml"), "[[").unwrap();
    std::fs::write(d.path().join("sets/ignored.txt"), "x").unwrap();
    let problems = sets.problems();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].0.ends_with("broken.toml"));
    sets.delete("Renamed").unwrap();
    sets.delete("never was").unwrap();
    assert!(sets.get("Renamed").is_none());
    assert_eq!(sets.list().len(), 2);
}

#[test]
fn results_are_cached_by_what_they_depend_on_and_live_runs_are_not() {
    let d = tempfile::tempdir().unwrap();
    let data = csv(d.path());
    let store = Store::new(Some(&d.path().join("cache")));
    let o = store.open(&data, None).unwrap();
    let mut named_in_when = check("when names it", "temp < 70");
    named_in_when.when = Some("Hot > 100".into());
    let mut uses_limits = check("limits", "past_warn(temp) == 0");
    uses_limits.group = "A".into();
    let s = set("S", vec![check("temp", "temp < 70"), uses_limits, check("named", "Hot < 1"), named_in_when]);
    let hot = |expr: &str| Recipe {
        name: "Hot".into(),
        expr: expr.into(),
        unit: None,
        category: None,
        decimals: None,
        automatic: true,
    };
    let first = store.run_checks(&o, &s, &[hot("temp - 60")]).unwrap();
    assert_eq!(
        first.outcomes.iter().map(|o| o.verdict).collect::<Vec<_>>(),
        [Verdict::Fail, Verdict::Err, Verdict::Fail, Verdict::Na],
        "no warn limit is set yet"
    );
    let results = || std::fs::read_dir(d.path().join("cache/checks-results")).unwrap().count();
    assert_eq!(results(), 1);
    assert_eq!(store.run_checks(&o, &s, &[hot("temp - 60")]).unwrap(), first, "read back");
    assert_eq!(results(), 1);
    // a recipe a row names changes the key; one it doesn't, doesn't
    store.run_checks(&o, &s, &[hot("temp - 70")]).unwrap();
    assert_eq!(results(), 2);
    let other = Recipe { name: "Unused".into(), ..hot("1") };
    store.run_checks(&o, &s, &[hot("temp - 70"), other]).unwrap();
    assert_eq!(results(), 2);
    // limits are part of the key for a row that reads them (a recipe with a quote or past_ counts too)
    store.set_expr_env(ExprEnv {
        limits: [(("temp".to_string(), false), (None, Some(65.0)))].into(),
        ..Default::default()
    });
    let with_limit = store.run_checks(&o, &s, &[hot("temp - 70")]).unwrap();
    assert_eq!(results(), 3);
    assert_eq!(with_limit.outcomes[1].verdict, Verdict::Fail, "temp is past the new warn limit");
    // the editor's preview and a live run read the same environment but keep nothing
    let one = store.check_one(&o, &check("p", "past_warn(temp) == 0"), &[]);
    assert_eq!(one.verdict, Verdict::Fail);
    let mut state = LiveSet::default();
    let live = store.run_checks_live(&o, &s, &[hot("temp - 70")], &mut state);
    assert_eq!(live.outcomes, with_limit.outcomes);
    assert_eq!(results(), 3);
}

#[test]
fn badges_name_the_channels_of_failing_checks_and_redaction_drops_rows_that_mention_a_name() {
    let mut warn = check("warn", "temp < 70");
    warn.severity = Severity::Warn;
    let mut rule = check("rule", "temp > 70");
    rule.kind = Kind::Rule;
    let s = set(
        "S",
        vec![
            check("rpm", "rpm < 5000"),
            warn,
            check("both", "rpm < 4000 && temp < 70"),
            rule,
            check("fine", "armed < 5"),
        ],
    );
    let res = run_set(&rec(), &s, T0, &[]);
    let m = failing_channels(&s, &res);
    assert_eq!(m.get("rpm"), Some(&Verdict::Fail));
    assert_eq!(m.get("temp"), Some(&Verdict::Fail), "a fail beats a warn on the same channel");
    assert!(!m.contains_key("armed"), "{m:?}");
    // a row whose text doesn't parse is left out if it so much as mentions a redacted name
    let mut broken = s.clone();
    broken.checks.push(check("broken", "rpm +"));
    broken.checks.push(check("when broken", "armed < 5"));
    broken.checks[6].when = Some("rpm >".into());
    let res = run_set(&rec(), &broken, T0, &[]);
    let (kept, kept_res, left) = redacted(&broken, &res, &["rpm".to_string()]);
    assert_eq!(left, 4);
    assert_eq!(kept.checks.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["warn", "rule", "fine"]);
    assert_eq!(kept_res.outcomes.len(), 3);
}

/// Files that can't be read, written or removed are said so, and the library stays as it was.
#[test]
fn a_library_that_cannot_be_written_says_so() {
    let d = tempfile::tempdir().unwrap();
    // a set folder under a plain file
    let file = d.path().join("plain");
    std::fs::write(&file, "x").unwrap();
    let e = Sets::in_dir(&file.join("sets")).save(&set("A", vec![]), None).unwrap_err().to_string();
    assert!(e.contains("plain"), "{e}");
    // a folder where the temporary file goes
    let sets = Sets::in_dir(&d.path().join("sets"));
    std::fs::create_dir_all(d.path().join("sets/a.tmp")).unwrap();
    let e = sets.save(&set("A", vec![]), None).unwrap_err().to_string();
    assert!(e.contains("a.tmp"), "{e}");
    // a file of the folder that isn't text is a problem, not a set
    std::fs::write(d.path().join("sets/binary.toml"), [0xffu8, 0xfe]).unwrap();
    assert_eq!(sets.problems().len(), 1);
    assert!(sets.list().is_empty());
}

#[cfg(unix)]
#[test]
fn a_read_only_library_and_results_folder_are_errors_not_silence() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let sets = Sets::in_dir(&d.path().join("sets"));
    sets.save(&set("Old", vec![]), None).unwrap();
    let dir = d.path().join("sets");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let deleted = sets.delete("Old");
    let renamed = sets.save(&set("New", vec![]), Some("Old"));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(deleted.unwrap_err().to_string().contains("old.toml"));
    assert!(renamed.is_err());
    assert!(sets.get("Old").is_some(), "nothing was lost");

    // results that can't be kept: a file where the folder goes, then a folder that can't be written
    let data = csv(d.path());
    let store = Store::new(Some(&d.path().join("cache")));
    let o = store.open(&data, None).unwrap();
    let s = set("S", vec![check("temp", "temp < 70")]);
    std::fs::write(d.path().join("cache/checks-results"), "x").unwrap();
    assert!(store.run_checks(&o, &s, &[]).unwrap_err().to_string().contains("checks-results"));
    std::fs::remove_file(d.path().join("cache/checks-results")).unwrap();
    std::fs::create_dir(d.path().join("cache/checks-results")).unwrap();
    std::fs::set_permissions(d.path().join("cache/checks-results"), std::fs::Permissions::from_mode(0o555)).unwrap();
    let e = store.run_checks(&o, &s, &[]);
    std::fs::set_permissions(d.path().join("cache/checks-results"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(e.is_err());
}
