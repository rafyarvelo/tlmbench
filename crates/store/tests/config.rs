//! Shared configuration: `.tlmbench/` folders read, compared, shared, pulled and exported.

use std::path::{Path, PathBuf};

use tlmbench_store::config::{self, Category, Kind, Own, Recording, State, TimeMapFile, Widget};
use tlmbench_store::events::{self, Event};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/recordings/.tlmbench")
}

fn user_event(id: &str, start: i64) -> Event {
    Event {
        id: id.into(),
        name: format!("event {id}"),
        start: start.to_string(),
        duration: "0".into(),
        category: "alarm".into(),
        source: "user".into(),
        ..Default::default()
    }
}

#[test]
fn the_example_folder_reads_clean_and_compares_equal_to_itself() {
    let (items, problems) = config::read_folder(&examples());
    assert!(problems.is_empty(), "{problems:?}");
    for k in [Kind::Views, Kind::Checks, Kind::Categories, Kind::Recipes, Kind::Widgets] {
        assert!(items.iter().any(|i| i.kind == k), "{k:?}");
    }
    assert!(config::compare(&items, &items).iter().all(|r| r.state == State::Same));
    assert!(config::validate(&examples()).is_empty());
}

#[test]
fn share_writes_once_and_rewrites_nothing_when_unchanged() {
    let cache = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().join(".tlmbench");
    let (ex, _) = config::read_folder(&examples());
    // the cache holds the example's recipes and check sets
    config::pull(cache.path(), &ex, &Own::default()).unwrap();
    let items = config::cache_items(cache.path(), &Own::default());
    assert!(items.iter().any(|i| i.kind == Kind::Recipes) && items.iter().any(|i| i.kind == Kind::Checks));
    let rows = config::compare(&items, &[]);
    assert!(rows.iter().all(|r| r.state == State::New));
    let first = config::share(&dir, &items).unwrap();
    assert!(!first.is_empty());
    assert!(config::share(&dir, &items).unwrap().is_empty(), "an unchanged object rewrites nothing");
    let (back, problems) = config::read_folder(&dir);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(config::compare(&items, &back).iter().all(|r| r.state == State::Same));
    // a changed recipe shows as changed, and only its file is written
    let mut changed = items.clone();
    let r = changed.iter_mut().find(|i| i.kind == Kind::Recipes).unwrap();
    let text = String::from_utf8(r.files[0].1.clone()).unwrap().replace("decimals = ", "decimals = 1");
    r.files[0].1 = text.into_bytes();
    let rows = config::compare(&changed, &back);
    assert_eq!(rows.iter().filter(|r| r.state == State::Changed).count(), 1);
    assert_eq!(config::share(&dir, &changed).unwrap().len(), 1);
}

#[test]
fn nothing_is_written_outside_the_dot_tlmbench_folder() {
    let folder = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("run.csv"), "t,a\n0,1\n").unwrap();
    let own = Own {
        categories: vec![Category {
            id: "alarm".into(),
            name: "Alarm".into(),
            color: "#f00".into(),
            show_on_charts: true,
            icon: None,
        }],
        ..Default::default()
    };
    config::share(&folder.path().join(".tlmbench"), &config::cache_items(cache.path(), &own)).unwrap();
    let names: Vec<_> = std::fs::read_dir(folder.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(folder.path().join(".tlmbench/categories.toml").exists());
}

#[test]
fn events_merge_by_id_and_find_their_recording_by_identity_then_name() {
    let cache = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().join(".tlmbench");
    let rec = Recording { name: "runs/a.csv".into(), identity: "aaaa".into(), ..Default::default() };
    let own = Own { recordings: vec![rec.clone()], ..Default::default() };
    events::save(cache.path(), "aaaa", user_event("e1", 100)).unwrap();
    events::save(cache.path(), "aaaa", user_event("e2", 200)).unwrap();
    config::share(&dir, &config::cache_items(cache.path(), &own)).unwrap();
    // a colleague adds e3 to the folder's file; yours then has e1, e2 and the folder e1, e2, e3
    let path = dir.join("events/runs-a-csv.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[[event]]\nid = \"e3\"\nname = \"theirs\"\nstart = 150\nduration = 0\n");
    std::fs::write(&path, text).unwrap();
    let (folder_items, _) = config::read_folder(&dir);
    let rows = config::compare(&config::cache_items(cache.path(), &own), &folder_items);
    let row = rows.iter().find(|r| r.kind == Kind::Events).unwrap();
    assert_eq!(row.state, State::Changed);
    assert_eq!(row.detail.as_deref(), Some("1 of theirs not yours"));
    // sharing again keeps e3
    config::share(&dir, &config::cache_items(cache.path(), &own)).unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains("e3"));
    // pulling into another cache puts them under the open recording's identity, matched by name
    let other = tempfile::tempdir().unwrap();
    let renamed = Own {
        recordings: vec![Recording { name: "runs/a.csv".into(), identity: "bbbb".into(), ..Default::default() }],
        ..Default::default()
    };
    let (folder_items, _) = config::read_folder(&dir);
    config::pull(other.path(), &folder_items, &renamed).unwrap();
    let got: Vec<String> = events::list(other.path(), "bbbb").unwrap().into_iter().map(|e| e.id).collect();
    assert_eq!(got, ["e1", "e3", "e2"]);
}

#[test]
fn widgets_categories_and_time_maps_round_trip() {
    let cache = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().join(".tlmbench");
    let w = Widget {
        id: "w1".into(),
        name: "Throttle lever".into(),
        icon: Some("<svg/>".into()),
        svg: Some("<svg>{{value}}</svg>".into()),
        angle: Some([-60.0, 60.0]),
        ..Default::default()
    };
    let code = Widget {
        id: "w2".into(),
        name: "Level".into(),
        code: Some("({ build(){}, update(){} })".into()),
        by: Some("j.kim".into()),
        ..Default::default()
    };
    let own = Own { widgets: vec![w.clone(), code.clone()], ..Default::default() };
    config::share(&dir, &config::cache_items(cache.path(), &own)).unwrap();
    assert!(
        dir.join("widgets/throttle-lever/widget.toml").exists() && dir.join("widgets/throttle-lever/icon.svg").exists()
    );
    let back = config::from_folder(&dir);
    let by_name = |n: &str| back.widgets.iter().find(|w| w.name == n).unwrap().clone();
    assert_eq!(by_name("Throttle lever").svg, w.svg);
    assert_eq!(by_name("Throttle lever").angle, w.angle);
    assert_eq!(by_name("Level").code, code.code);
    // a time map goes through the cache's folder as the folder's own file
    let t = TimeMapFile {
        schema: 1,
        primary: "a.csv".into(),
        file: "b.csv".into(),
        method: "sync".into(),
        knots: vec![[0, 10], [100, 111]],
        pairs: vec![[1.0, 1.5]],
        ..Default::default()
    };
    std::fs::create_dir_all(cache.path().join("timemaps")).unwrap();
    std::fs::write(cache.path().join("timemaps").join(t.file_name()), config::timemap_to_toml(&t)).unwrap();
    let items = config::cache_items(cache.path(), &Own::default());
    assert_eq!(items.iter().filter(|i| i.kind == Kind::Timemaps).count(), 1);
    let out = tempfile::tempdir().unwrap();
    config::export(cache.path(), &Own::default(), out.path()).unwrap();
    let text = std::fs::read_to_string(out.path().join("timemaps/a--b.toml")).unwrap();
    assert_eq!(config::timemap_from_toml(&text).unwrap(), t);
}

#[test]
fn profiles_share_as_toml_and_come_back() {
    use tlmbench_decode::profile::{self, ImportProfile, RateChoice};
    let cache = tempfile::tempdir().unwrap();
    let header: Vec<String> = ["time", "price", "volume"].iter().map(|s| s.to_string()).collect();
    let p = ImportProfile { rate: RateChoice::Resample { hz: 10.0 }, ..Default::default() };
    profile::save(cache.path(), &header, &p).unwrap();
    let items = config::cache_items(cache.path(), &Own::default());
    let item = items.iter().find(|i| i.kind == Kind::Profiles).unwrap();
    assert!(item.files[0].0.starts_with("profiles/time-price-volume-"), "{}", item.files[0].0);
    let other = tempfile::tempdir().unwrap();
    let folder = other.path().join(".tlmbench");
    config::share(&folder, &items).unwrap();
    let (back, problems) = config::read_folder(&folder);
    assert!(problems.is_empty(), "{problems:?}");
    let third = tempfile::tempdir().unwrap();
    config::pull(third.path(), &back, &Own::default()).unwrap();
    assert_eq!(profile::find(third.path(), &header).exact, Some(p));
}

#[test]
fn a_bad_file_is_a_problem_not_a_crash_and_validate_finds_bad_expressions() {
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().join(".tlmbench");
    std::fs::create_dir_all(dir.join("recipes")).unwrap();
    std::fs::write(dir.join("recipes/broken.toml"), "schema = 1\n[[recipe]\n").unwrap();
    std::fs::write(dir.join("recipes/bad-expr.toml"), "schema = 1\n[[recipe]]\nname = \"x\"\nexpr = \"a +\"\n")
        .unwrap();
    std::fs::write(dir.join("recipes/future.toml"), "schema = 9\n").unwrap();
    let problems = config::validate(&dir);
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(problems.iter().any(|p| p.why.contains("schema 9")));
}

#[test]
fn the_nearest_folder_is_found_from_a_file_or_config_wins() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".tlmbench")).unwrap();
    std::fs::create_dir_all(root.path().join("runs/day1")).unwrap();
    let file = root.path().join("runs/day1/a.csv");
    std::fs::write(&file, "t\n1\n").unwrap();
    let want = std::fs::canonicalize(root.path().join(".tlmbench")).unwrap();
    assert_eq!(config::nearest(&file), Some(want.clone()));
    assert_eq!(config::dirs_for(&file, &[]), vec![want]);
    let other = PathBuf::from("/somewhere/else");
    assert_eq!(config::dirs_for(&file, std::slice::from_ref(&other)), vec![other]);
}

#[test]
fn the_folders_recipes_sets_events_and_profiles_fill_in_behind_yours() {
    use tlmbench_store::Library;
    let folder = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let dir = folder.path().join(".tlmbench");
    let (ex, _) = config::read_folder(&examples());
    // the folder shares the example's recipes and sets, and one recording's events
    config::share(&dir, &ex).unwrap();
    let rec = Recording { name: "a.csv".into(), identity: "aaaa".into(), ..Default::default() };
    let theirs = tempfile::tempdir().unwrap();
    events::save(theirs.path(), "aaaa", user_event("e1", 5)).unwrap();
    let own = Own { recordings: vec![rec], ..Default::default() };
    config::share(&dir, &config::cache_items(theirs.path(), &own)).unwrap();
    config::set_overlay(vec![dir.clone()]);
    let names: Vec<String> = Library::new(cache.path()).list().into_iter().map(|r| r.name).collect();
    assert!(names.contains(&"Power".to_string()), "{names:?}");
    // a recipe of your own with the same name wins
    let mine = tlmbench_expr::Recipe {
        name: "Power".into(),
        expr: "1".into(),
        unit: None,
        category: None,
        decimals: None,
        automatic: true,
    };
    Library::new(cache.path()).save(&mine, None).unwrap();
    assert_eq!(Library::new(cache.path()).get("Power").unwrap().expr, "1");
    assert!(!tlmbench_store::checks::Sets::new(cache.path()).list().is_empty());
    assert!(
        tlmbench_store::checks::Sets::in_dir(&cache.path().join("none")).list().is_empty(),
        "a plain folder doesn't overlay"
    );
    let shared = config::shared_events(&[dir.clone()], "aaaa", "");
    assert_eq!((shared.len(), shared[0].source.as_str()), (1, "shared"));
    assert_eq!(config::shared_events(&[dir], "zzzz", "a.csv").len(), 1, "found by name when the identity changed");
    config::set_overlay(vec![]);
    assert!(Library::new(cache.path()).list().iter().all(|r| r.expr == "1"));
}

#[test]
fn a_layout_matches_its_shared_view_and_keeps_the_view_files_and_range() {
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().join(".tlmbench");
    let shared = serde_json::json!({"schema":1,"name":"Flight","by":"a.chen","files":[{"path":"a.csv","slot":1}],"view":[0.0,5.0],"panes":[{"channels":["x"]}]});
    let layout = serde_json::json!({"schema":1,"name":"Flight","panes":[{"channels":["x"]}]});
    let view = |v: &serde_json::Value| config::View { file: "flight.tlmview.json".into(), view: v.clone() };
    let own = |v: &serde_json::Value| Own { views: vec![view(v)], ..Default::default() };
    let cache = tempfile::tempdir().unwrap();
    config::share(&dir, &config::cache_items(cache.path(), &own(&shared))).unwrap();
    let (theirs, _) = config::read_folder(&dir);
    let rows = config::compare(&config::cache_items(cache.path(), &own(&layout)), &theirs);
    assert_eq!(rows[0].state, State::Same, "{rows:?}");
    // changing the layout changes the shared file, and the view's files and range stay
    let moved = serde_json::json!({"schema":1,"name":"Flight","panes":[{"channels":["x","y"]}]});
    let rows = config::compare(&config::cache_items(cache.path(), &own(&moved)), &theirs);
    assert_eq!(rows[0].state, State::Changed);
    config::share(&dir, &config::cache_items(cache.path(), &own(&moved))).unwrap();
    let text = std::fs::read_to_string(dir.join("views/flight.tlmview.json")).unwrap();
    assert!(text.contains("\"files\"") && text.contains("a.chen") && text.contains("\"y\""), "{text}");
}
