//! The check engine against a synthetic recording: scope, settle, grace, summaries, worst values, roll-up, events,
//! the library of sets and the example sets people share.

use std::collections::HashMap;
use std::ops::Range;

use tlmbench_expr::{Env, Recipe, Series};
use tlmbench_store::checks::*;

const HZ: f64 = 100.0;
const N: usize = 6000; // 60 s

struct Rec {
    ch: HashMap<&'static str, Series>,
}

impl Env for Rec {
    fn series(&self, n: &str) -> Option<Series> {
        self.ch.get(n).cloned()
    }
    fn read(&self, n: &str, r: Range<usize>) -> Option<Series> {
        self.ch.get(n).map(|s| s.slice(r))
    }
    fn samples(&self) -> Option<usize> {
        Some(N)
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

/// `rpm` ramps 0..6000 over the minute; `mode` is 2 from 10 s to 40 s, else 0; `armed` is 1 from 5 s to 55 s;
/// `temp` is 60 with a 3 s excursion to 75 at 20 s; `sat` has a hole of NaNs 30 s..31 s.
fn rec() -> Rec {
    let f = |g: &dyn Fn(f64) -> f64| Series::rate((0..N).map(|i| g(i as f64 / HZ)).collect(), HZ);
    let mut ch = HashMap::new();
    ch.insert("rpm", f(&|t| t * 100.0));
    ch.insert("mode", f(&|t| if (10.0..40.0).contains(&t) { 2.0 } else { 0.0 }));
    ch.insert("armed", f(&|t| if (5.0..55.0).contains(&t) { 1.0 } else { 0.0 }));
    ch.insert("temp", f(&|t| if (20.0..23.0).contains(&t) { 75.0 } else { 60.0 }));
    ch.insert("sat", f(&|t| if (30.0..31.0).contains(&t) { f64::NAN } else { 12.0 }));
    ch.insert("idle", f(&|_| 0.0));
    Rec { ch }
}

fn check(name: &str, expect: &str) -> Check {
    Check { name: name.into(), expect: expect.into(), enabled: true, group: "G".into(), ..Check::default() }
}

fn run(k: &Check) -> Outcome {
    run_check(&rec(), k, 1_000_000_000_000, &[])
}

fn secs(span: &Span) -> (f64, f64) {
    ((span.start - 1_000_000_000_000) as f64 / 1e9, (span.end - 1_000_000_000_000) as f64 / 1e9)
}

#[test]
fn a_sample_check_finds_its_violations() {
    let o = run(&check("rpm", "rpm < 5000"));
    assert_eq!(o.verdict, Verdict::Fail);
    assert_eq!((o.count, o.runs.len()), (1, 1));
    let (a, b) = secs(&o.runs[0]);
    assert!((a - 50.0).abs() < 1e-6 && (b - 60.0).abs() < 1e-6, "{a}..{b}");
    assert!((o.secs - 10.0).abs() < 1e-6);
    // the worst value is the furthest past the limit, and when
    assert_eq!(o.worst, Some(5990.0 + 9.0));
    assert!((secs(&Span { start: o.worst_t.unwrap(), end: 0 }).0 - 59.99).abs() < 1e-6);
    assert!(o.channels == ["rpm"]);
    // passing
    assert_eq!(run(&check("ok", "rpm < 7000")).verdict, Verdict::Pass);
    // a warning severity
    let mut w = check("w", "rpm < 5000");
    w.severity = Severity::Warn;
    assert_eq!(run(&w).verdict, Verdict::Warn);
}

#[test]
fn grace_ignores_short_violations_and_settle_the_start_of_the_scope() {
    // temp > 70 for 3 s at 20 s
    let mut k = check("temp", "temp < 70");
    assert_eq!(run(&k).count, 1);
    k.grace = 3.5;
    assert_eq!(run(&k).verdict, Verdict::Pass);
    k.grace = 2.9;
    assert_eq!(run(&k).verdict, Verdict::Fail);
    // the excursion starts 10 s into 'armed' (5 s..): a settle of 16 s skips the first 16 s of the scope
    let mut s = check("s", "temp < 70");
    s.when = Some("armed".into());
    assert_eq!(run(&s).verdict, Verdict::Fail);
    s.settle = 16.0; // scope starts at 21 s: the last 2 s of the excursion remain
    let o = run(&s);
    assert_eq!(o.verdict, Verdict::Fail);
    let (a, b) = secs(&o.runs[0]);
    assert!((a - 21.0).abs() < 0.02 && (b - 23.0).abs() < 1e-6, "{a}..{b}");
    s.settle = 18.0; // starts at 23 s: nothing left
    assert_eq!(run(&s).verdict, Verdict::Pass);
}

#[test]
fn violations_close_together_are_one() {
    // temp is high for 3 s; with rpm in a band the same signal is fine; two violations 0.1 s apart join
    let mut k = check("two", "!(rpm > 1000 && rpm < 1100) && !(rpm > 1110 && rpm < 1200)");
    k.grace = 0.0;
    // rpm 1000..1100 is 10 s..11 s, 1110..1200 is 11.1 s..12 s: the 0.1 s between them joins
    let o = run(&k);
    assert_eq!((o.verdict, o.count), (Verdict::Fail, 1));
}

#[test]
fn when_scopes_the_check() {
    let mut k = check("rpm in mission", "rpm < 2500");
    k.when = Some("mode == 'Mission'".into());
    let o = run(&k);
    // Mission is 10 s..40 s; rpm passes 2500 at 25 s
    assert_eq!(o.verdict, Verdict::Fail);
    let (a, b) = secs(&o.runs[0]);
    assert!((a - 25.0).abs() < 0.02 && (b - 40.0).abs() < 0.02, "{a}..{b}");
    assert!((o.scope_secs.unwrap() - 30.0).abs() < 0.02);
    // a When that never holds is Not applicable
    let mut n = check("n", "rpm < 1");
    n.when = Some("idle > 1".into());
    assert_eq!(run(&n).verdict, Verdict::Na);
    // a constant When is fine
    n.when = Some("1".into());
    assert_eq!(run(&n).verdict, Verdict::Fail);
}

#[test]
fn summary_checks_see_only_the_scope() {
    let mut k = check("max rpm", "max(rpm) < 3000");
    let o = run(&k);
    assert_eq!(
        (o.verdict, o.value, o.limit, o.function.as_deref()),
        (Verdict::Fail, Some(5999.0), Some(3000.0), Some("max"))
    );
    k.when = Some("mode == 'Mission'".into()); // 10 s..40 s: max 3999
    let o = run(&k);
    assert_eq!((o.verdict, o.value), (Verdict::Fail, Some(3999.0)));
    k.when = Some("armed == 0 && rpm < 400".into()); // 0 s..4 s
    let o = run(&k);
    assert_eq!(o.verdict, Verdict::Pass);
    assert!(o.value.unwrap() < 400.0);
    // the summary functions
    assert_eq!(run(&check("d", "duration(mode == 'Mission') > 29")).verdict, Verdict::Pass);
    assert_eq!(run(&check("d", "duration(mode == 'Mission') > 31")).verdict, Verdict::Fail);
    assert_eq!(run(&check("p", "pct(armed) >= 80")).verdict, Verdict::Pass);
    assert_eq!(run(&check("l", "last(armed) == 0")).verdict, Verdict::Pass);
    assert_eq!(run(&check("o", "occurrences(armed) == 1")).verdict, Verdict::Pass);
    assert_eq!(run(&check("m", "mean(idle) == 0")).verdict, Verdict::Pass);
    // a rule can't be one number
    let mut r = check("r", "max(rpm) > 1");
    r.kind = Kind::Rule;
    assert_eq!(run(&r).verdict, Verdict::Err);
}

#[test]
fn missing_values_are_no_data_not_failures() {
    // sat has a hole 30 s..31 s of NaN; "sat >= 10" holds everywhere else
    let o = run(&check("sat", "sat >= 10"));
    assert_eq!(o.verdict, Verdict::Pass);
    assert!((o.scope_secs.unwrap() - 59.0).abs() < 0.02);
}

#[test]
fn rules_mark_where_the_condition_holds() {
    let mut r = check("hot", "past_warn(temp)");
    r.kind = Kind::Rule;
    r.category = Some("alarm".into());
    let o = run(&r);
    assert_eq!((o.verdict, o.count), (Verdict::Mark, 1));
    let (a, b) = secs(&o.runs[0]);
    assert!((a - 20.0).abs() < 1e-6 && (b - 22.99).abs() < 1e-6, "{a}..{b}");
    // grace drops short runs
    r.grace = 5.0;
    let o = run(&r);
    assert_eq!((o.verdict, o.count), (Verdict::Mark, 0));
    // a one-sample run is an instant
    let mut i = check("i", "rpm == 1234");
    i.kind = Kind::Rule;
    let o = run(&i);
    assert_eq!(o.count, 1);
    assert_eq!(o.runs[0].start, o.runs[0].end);
}

#[test]
fn what_cannot_run_says_so() {
    let o = run(&check("x", "nothing_here < 3"));
    assert_eq!(o.verdict, Verdict::Cant);
    assert!(o.message.contains("nothing_here"), "{}", o.message);
    assert_eq!(run(&check("x", "rpm <")).verdict, Verdict::Err);
    assert_eq!(run(&check("x", "")).verdict, Verdict::Err);
    let mut w = check("x", "rpm < 1");
    w.when = Some("gone > 1".into());
    let o = run(&w);
    assert_eq!(o.verdict, Verdict::Cant);
    assert!(o.message.starts_with("When:"));
    let mut off = check("x", "rpm < 1");
    off.enabled = false;
    assert_eq!(run(&off).verdict, Verdict::Off);
}

#[test]
fn recipes_stand_in_for_channels() {
    let recipe = Recipe {
        name: "Half rpm".into(),
        expr: "rpm / 2".into(),
        unit: None,
        category: None,
        decimals: None,
        automatic: true,
    };
    let k = check("half", "\"Half rpm\" < 2000");
    let o = run_check(&rec(), &k, 0, &[recipe]);
    assert_eq!(o.verdict, Verdict::Fail);
    assert_eq!(run(&k).verdict, Verdict::Cant);
}

fn set(checks: Vec<Check>, info: &[&str]) -> CheckSet {
    CheckSet {
        schema: SCHEMA,
        name: "Test".into(),
        runs_on_any_file: true,
        info_groups: info.iter().map(|s| s.to_string()).collect(),
        checks,
    }
}

#[test]
fn groups_and_sets_take_their_worst_result() {
    let mut warn = check("warn", "rpm < 5000");
    warn.group = "A".into();
    warn.severity = Severity::Warn;
    let mut pass = check("pass", "rpm < 7000");
    pass.group = "A".into();
    let mut fail = check("fail", "rpm < 5000");
    fail.group = "B".into();
    let mut cant = check("cant", "zzz < 1");
    cant.group = "C".into();
    let mut na = check("na", "rpm < 1");
    na.group = "D".into();
    na.when = Some("idle > 1".into());
    let mut env_fail = check("info", "rpm < 5000");
    env_fail.group = "Env".into();

    let s = set(vec![warn.clone(), pass.clone(), cant.clone(), na.clone()], &[]);
    let r = run_set(&rec(), &s, 0, &[]);
    let g = |n: &str| r.rollup.groups.iter().find(|g| g.name == n).unwrap().verdict;
    assert_eq!((g("A"), g("C"), g("D")), (Verdict::Warn, Verdict::Cant, Verdict::Na));
    // Warn beats can't-run; not applicable doesn't count
    assert_eq!((r.rollup.verdict, r.rollup.warned, r.rollup.failed), (Verdict::Warn, 1, 0));
    assert!(r.rollup.applies);

    // Fail beats Warn
    let r = run_set(&rec(), &set(vec![warn, pass, fail.clone()], &[]), 0, &[]);
    assert_eq!((r.rollup.verdict, r.rollup.failed, r.rollup.warned), (Verdict::Fail, 1, 1));
    let a = r.rollup.groups.iter().find(|g| g.name == "A").unwrap();
    assert_eq!((a.total, a.passed, a.warned, a.failed), (2, 1, 1, 0));

    // info-only groups show their result but never fail the set
    let r = run_set(&rec(), &set(vec![env_fail, check("fine", "rpm < 7000")], &["Env"]), 0, &[]);
    assert_eq!(r.rollup.groups.iter().find(|g| g.name == "Env").unwrap().verdict, Verdict::Fail);
    assert_eq!(r.rollup.verdict, Verdict::Pass);

    // nothing could run: the set doesn't apply to the file
    let r = run_set(&rec(), &set(vec![cant], &[]), 0, &[]);
    assert!(!r.rollup.applies);
    assert_eq!(r.rollup.verdict, Verdict::Cant);

    // a group of rules shows events, not a verdict
    let mut rule = check("rule", "past_warn(temp)");
    rule.kind = Kind::Rule;
    rule.group = "R".into();
    let r = run_set(&rec(), &set(vec![rule, fail], &[]), 0, &[]);
    let g = r.rollup.groups.iter().find(|g| g.name == "R").unwrap();
    assert_eq!((g.verdict, g.events), (Verdict::Mark, 1));
    assert_eq!(r.rollup.events, 1);
    assert_eq!(r.rollup.verdict, Verdict::Fail);
}

#[test]
fn results_make_read_only_events() {
    let mut rule = check("Hot", "past_warn(temp)");
    rule.kind = Kind::Rule;
    rule.category = Some("alarm".into());
    let s = set(vec![check("RPM", "rpm < 5000"), rule], &[]);
    let r = run_set(&rec(), &s, 1_000_000_000_000, &[]);
    let ev = events_of(&s, &r);
    assert_eq!(ev.len(), 2);
    // sorted by start; the rule's is at 20 s
    assert_eq!((ev[0].name.as_str(), ev[0].source.as_str(), ev[0].category.as_str()), ("Hot", "rule", "alarm"));
    assert_eq!((ev[1].name.as_str(), ev[1].source.as_str(), ev[1].category.as_str()), ("RPM", "check", CHECK_CATEGORY));
    assert_eq!(ev[1].start, (1_000_000_000_000i64 + 50_000_000_000).to_string());
    assert_eq!(ev[1].channels, ["rpm"]);
    assert_eq!(events_of(&s, &r), ev, "ids are stable");
    // and the wire form keeps nanoseconds whole
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"1050000000000\""), "{json}");
    assert_eq!(serde_json::from_str::<SetResult>(&json).unwrap(), r);
}

#[test]
fn sets_are_toml_files_in_the_shared_format() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/recordings/.tlmbench/checks");
    let sets = Sets::in_dir(&dir);
    assert!(sets.problems().is_empty(), "{:?}", sets.problems());
    let all = sets.list();
    assert_eq!(
        all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["ESC bench limits", "Event rules", "Flight acceptance"]
    );
    let flight = sets.get("Flight acceptance").unwrap();
    assert_eq!(flight.checks.len(), 16);
    assert_eq!(flight.info_groups, ["Environment"]);
    assert!(flight.runs_on_any_file);
    let rules = sets.get("Event rules").unwrap();
    assert!(rules.checks.iter().all(|k| k.kind == Kind::Rule) && !rules.checks[3].enabled);
    // text round trip
    for s in &all {
        assert_eq!(&from_toml(&to_toml(s)).unwrap(), s);
    }
    // saving, renaming, deleting
    let tmp = tempfile::tempdir().unwrap();
    let lib = Sets::new(tmp.path());
    let mut s = set(vec![check("a", "rpm < 1")], &[]);
    lib.save(&s, None).unwrap();
    assert_eq!(lib.get("Test").unwrap(), s);
    let old = s.name.clone();
    s.name = "Renamed".into();
    lib.save(&s, Some(&old)).unwrap();
    assert_eq!(lib.list().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Renamed"]);
    lib.delete("Renamed").unwrap();
    assert!(lib.list().is_empty());
    // a new version when anything that changes a result changes
    let mut t = set(vec![check("a", "rpm < 1")], &[]);
    let v = t.version();
    t.checks[0].grace = 1.0;
    assert_ne!(t.version(), v);
}
