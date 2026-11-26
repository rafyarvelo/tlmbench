//! The shared-configuration folder, every kind of file in it and every way one can be wrong (coverage, R6c.3,
//! Nov 26): files that don't read, the folder against the cache, writing, pulling, the mirror of the app's settings,
//! and the folders read besides the cache. Each asserts what a person sees: the problem listed, the row's state, the
//! bytes in the folder.

use std::fs;
use std::path::Path;

use tlmbench_decode::profile::{self, ImportProfile, PrefixRule};
use tlmbench_store::config::{self, Category, Icon, Kind, Own, Recording, State, View, Widget};
use tlmbench_store::events::{self, Event};

fn put(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn user(id: &str, start: i64) -> Event {
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

fn why<'a>(problems: &'a [config::Problem], path: &str) -> &'a str {
    &problems.iter().find(|p| p.path == path).unwrap_or_else(|| panic!("no problem for {path}: {problems:?}")).why
}

/// One broken file of every kind is one problem each, with its path and why; the good ones beside them still read.
#[test]
fn a_file_of_every_kind_that_does_not_read_is_a_problem_with_its_path() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path();
    put(dir, "views/bad.tlmview.json", "{ not json");
    put(dir, "views/good.tlmview.json", r#"{"name":"Good","panes":[]}"#);
    put(dir, "views/unnamed.tlmview.json", r#"{"panes":[]}"#);
    put(dir, "events/bad.toml", "[[event]\n");
    put(dir, "events/future.toml", "schema = 9\nrecording = \"a.csv\"\nidentity = \"aa\"\n");
    put(dir, "checks/bad.toml", "[[check]\n");
    put(dir, "recipes/bad.toml", "schema = 1\n[[recipe]\n");
    put(dir, "profiles/bad.toml", "nonsense = \n");
    put(dir, "profiles/future.toml", "schema = 9\nheader = [\"a\"]\n[profile]\n");
    let pattern = ImportProfile { prefix: Some(PrefixRule::Pattern { pattern: "(".into() }), ..Default::default() };
    let cache = tempfile::tempdir().unwrap();
    profile::save(cache.path(), &["x".into(), "y".into()], &pattern).unwrap();
    let kept = config::cache_items(cache.path(), &Own::default());
    let item = kept.iter().find(|i| i.kind == Kind::Profiles).unwrap();
    let broken_profile = String::from_utf8(item.files[0].1.clone()).unwrap();
    put(dir, "profiles/pattern.toml", &broken_profile);
    put(dir, "decoders/bad.toml", "this is not a descriptor");
    put(dir, "timemaps/bad.toml", "method = \n");
    put(dir, "timemaps/future.toml", "schema = 4\nprimary = \"a\"\nfile = \"b\"\nmethod = \"sync\"\n");
    put(dir, "categories.toml", "schema = 1\n[[category]\n");
    put(dir, "widgets/nothing/readme.txt", "a folder that is not a widget");
    put(dir, "icons/dial.svg", "<svg/>");
    put(dir, "icons/ignored.png", "x");

    let (items, problems) = config::read_folder(dir);
    for p in [
        "views/bad.tlmview.json",
        "events/bad.toml",
        "events/future.toml",
        "checks/bad.toml",
        "recipes/bad.toml",
        "profiles/bad.toml",
        "profiles/future.toml",
        "profiles/pattern.toml",
        "decoders/bad.toml",
        "timemaps/bad.toml",
        "timemaps/future.toml",
        "categories.toml",
        "widgets/nothing/widget.toml",
    ] {
        assert!(!why(&problems, p).is_empty(), "{p}");
    }
    assert_eq!(problems.len(), 13, "{problems:?}");
    assert!(why(&problems, "events/future.toml").contains("event file schema 9"));
    assert!(why(&problems, "profiles/future.toml").contains("profile schema 9"));
    assert!(why(&problems, "profiles/pattern.toml").contains("the prefix pattern"));
    assert!(why(&problems, "timemaps/future.toml").contains("time map schema 4"));
    assert!(why(&problems, "widgets/nothing/widget.toml").contains("not found"));
    // the readable ones: a view is named by its "name" or "view"; an icon by its file; the .png is no icon
    let names = |k: Kind| items.iter().filter(|i| i.kind == k).map(|i| i.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(Kind::Views), ["Good", "view"]);
    assert_eq!(names(Kind::Icons), ["dial"]);
    assert_eq!(items.iter().find(|i| i.kind == Kind::Icons).unwrap().files[0].0, "icons/dial.svg");
    // validate reports the same files
    assert_eq!(config::validate(dir).len(), 13);
}

/// A folder whose every object is good reads each kind back to the same bytes it was shared as.
#[test]
fn a_decoder_a_time_map_a_check_set_and_icons_share_and_read_back() {
    use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
    let cache = tempfile::tempdir().unwrap();
    let d = Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        events: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: Some("Bench log".into()),
        endian: Endian::Little,
        header_size: 0,
        record_size: 4,
        time: TimeSpec::Rate { rate_hz: 100.0, t0_ns: 0 },
        fields: vec![Field {
            name: "a".into(),
            dtype: DType::F32,
            offset: 0,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
            category: None,
            bits: None,
            plugin: None,
            size: 0,
            params: None,
        }],
    };
    put(cache.path(), "decoders/bench.toml", &d.to_toml());
    put(
        cache.path(),
        "checks/limits.toml",
        "schema = 1\nname = \"Limits\"\n[[check]]\nname = \"rpm\"\nexpect = \"rpm < 5000\"\n",
    );
    let own = Own {
        icons: vec![Icon { name: "Dial 2".into(), svg: "<svg id=\"d\"/>".into() }],
        views: vec![View { file: "a.tlmview.json".into(), view: serde_json::json!({"name": "A", "panes": []}) }],
        ..Default::default()
    };
    let items = config::cache_items(cache.path(), &own);
    let kinds: Vec<Kind> = items.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, [Kind::Views, Kind::Checks, Kind::Decoders, Kind::Icons], "in kind order");
    assert_eq!(items[3].files[0].0, "icons/dial-2.svg");
    let out = tempfile::tempdir().unwrap();
    let written = config::share(out.path(), &items).unwrap();
    assert_eq!(written.len(), 4);
    let (back, problems) = config::read_folder(out.path());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(back.iter().find(|i| i.kind == Kind::Decoders).unwrap().name, "Bench log");
    assert_eq!(back.iter().find(|i| i.kind == Kind::Checks).unwrap().name, "Limits");
    assert_eq!(back.iter().find(|i| i.kind == Kind::Icons).unwrap().name, "dial-2");
    assert!(config::compare(&items, &back).iter().all(|r| r.state == State::Same || r.kind == Kind::Icons));
    // pulling takes the cache's own kinds and leaves the settings' kinds to the app
    let other = tempfile::tempdir().unwrap();
    let taken = config::pull(other.path(), &back, &Own::default()).unwrap();
    assert_eq!(taken, ["checks/limits.toml", "decoders/bench.toml"]);
    assert!(other.path().join("checks/limits.toml").exists() && other.path().join("decoders/bench.toml").exists());
    // a text that isn't a profile or an event file is skipped, not an error
    let junk = vec![
        config::Item { kind: Kind::Profiles, name: "x".into(), files: vec![("profiles/x.toml".into(), b"?".to_vec())] },
        config::Item { kind: Kind::Events, name: "y".into(), files: vec![("events/y.toml".into(), b"?".to_vec())] },
    ];
    let taken = config::pull(other.path(), &junk, &Own::default()).unwrap();
    assert_eq!(
        taken,
        ["profiles/x.toml"],
        "a profile that doesn't read counts as taken, an event file that doesn't doesn't"
    );
}

/// The Share dialog's rows: what only the folder has, what only you have, and how events differ.
#[test]
fn compare_says_what_only_the_folder_has_and_how_event_files_differ() {
    let cache = tempfile::tempdir().unwrap();
    let theirs = tempfile::tempdir().unwrap();
    let dir = theirs.path().join(".tlmbench");
    let rec = Recording { name: "a.csv".into(), identity: "aaaa".into(), ..Default::default() };
    let own = Own { recordings: vec![rec.clone()], ..Default::default() };
    events::save(cache.path(), "aaaa", user("e1", 100)).unwrap();
    events::save(cache.path(), "aaaa", user("e2", 200)).unwrap();
    // the folder has e1 only, an icon you don't, and your cache has an icon the folder lacks
    let shared = tempfile::tempdir().unwrap();
    events::save(shared.path(), "aaaa", user("e1", 100)).unwrap();
    config::share(&dir, &config::cache_items(shared.path(), &own)).unwrap();
    put(&dir, "icons/theirs.svg", "<svg/>");
    let (folder, _) = config::read_folder(&dir);
    let mine = config::cache_items(
        cache.path(),
        &Own { icons: vec![Icon { name: "mine".into(), svg: "<svg/>".into() }], ..own.clone() },
    );
    let rows = config::compare(&mine, &folder);
    let row = |k: Kind, n: &str| rows.iter().find(|r| r.kind == k && r.name == n).unwrap().clone();
    assert_eq!(row(Kind::Events, "a.csv").state, State::Changed);
    assert_eq!(row(Kind::Events, "a.csv").detail.as_deref(), Some("1 of yours not shared"));
    assert_eq!(row(Kind::Icons, "mine").state, State::New);
    assert_eq!(row(Kind::Icons, "theirs").state, State::Folder);
    // the other way round: the folder has everything of yours and more: it is the same for you, "theirs" noted
    let rows = config::compare(&folder, &mine);
    assert_eq!(rows.iter().find(|r| r.kind == Kind::Events).unwrap().detail.as_deref(), Some("1 of theirs not yours"));
    // yours all there: same, with no detail
    let rows = config::compare(&config::cache_items(shared.path(), &own), &folder);
    let e = rows.iter().find(|r| r.kind == Kind::Events).unwrap();
    assert_eq!((e.state, e.detail.clone()), (State::Same, None));
    // sorted by kind then name
    let kinds: Vec<Kind> = config::compare(&mine, &folder).iter().map(|r| r.kind).collect();
    assert!(kinds.windows(2).all(|w| w[0] <= w[1]));
}

/// A widget's folder is replaced as a whole when shared again, and a folder that can't take the write says so.
#[test]
fn sharing_a_widget_again_removes_files_it_no_longer_has_and_a_blocked_folder_is_an_error() {
    let cache = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let mut w = Widget {
        id: "w".into(),
        name: "Gauge".into(),
        svg: Some("<svg/>".into()),
        icon: Some("<svg id=\"i\"/>".into()),
        ..Default::default()
    };
    let share = |w: &Widget| {
        config::share(
            out.path(),
            &config::cache_items(cache.path(), &Own { widgets: vec![w.clone()], ..Default::default() }),
        )
    };
    share(&w).unwrap();
    assert!(out.path().join("widgets/gauge/icon.svg").exists());
    put(out.path(), "widgets/gauge/stale.txt", "left over");
    w.icon = None;
    let written = share(&w).unwrap();
    assert_eq!(written, ["widgets/gauge/widget.toml"], "only what changed is written");
    assert!(
        !out.path().join("widgets/gauge/icon.svg").exists() && !out.path().join("widgets/gauge/stale.txt").exists()
    );
    // a file where a folder is needed can't be written through
    let blocked = tempfile::tempdir().unwrap();
    fs::write(blocked.path().join("icons"), "a file").unwrap();
    let items = config::cache_items(
        cache.path(),
        &Own { icons: vec![Icon { name: "x".into(), svg: "<svg/>".into() }], ..Default::default() },
    );
    let e = config::share(blocked.path(), &items).unwrap_err().to_string();
    assert!(e.contains("icons"), "{e}");
}

/// What the app keeps, mirrored into the cache for the CLI.
#[test]
fn the_apps_settings_are_mirrored_once_and_read_back_and_nothing_means_defaults() {
    let cache = tempfile::tempdir().unwrap();
    assert_eq!(config::load_mirror(cache.path()), Own::default());
    let own = Own {
        categories: vec![Category {
            id: "a".into(),
            name: "A".into(),
            color: "#f00".into(),
            show_on_charts: true,
            icon: None,
        }],
        recordings: vec![Recording { name: "x.csv".into(), identity: "ii".into(), file: Some(2) }],
        ..Default::default()
    };
    config::save_mirror(cache.path(), &own).unwrap();
    let stamp = fs::metadata(cache.path().join("own.json")).unwrap().modified().unwrap();
    assert_eq!(config::load_mirror(cache.path()), own);
    std::thread::sleep(std::time::Duration::from_millis(20));
    config::save_mirror(cache.path(), &own).unwrap();
    assert_eq!(
        fs::metadata(cache.path().join("own.json")).unwrap().modified().unwrap(),
        stamp,
        "unchanged: not rewritten"
    );
    // a mirror that is damaged reads as nothing
    fs::write(cache.path().join("own.json"), "{").unwrap();
    assert_eq!(config::load_mirror(cache.path()), Own::default());
    // exporting writes the folder's files from the cache and own
    let out = tempfile::tempdir().unwrap();
    let written = config::export(cache.path(), &own, out.path()).unwrap();
    assert_eq!(written, ["categories.toml"]);
}

/// The folders read besides the cache, later ones winning: recipes, check sets, profiles; the overlay itself. One test,
/// since the overlay is the process's.
#[test]
fn shared_folders_fill_in_in_order_and_the_last_wins_a_name() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let recipe = |name: &str, expr: &str| format!("schema = 1\n[[recipe]]\nname = \"{name}\"\nexpr = \"{expr}\"\n");
    put(a.path(), "recipes/r.toml", &recipe("Power", "1"));
    put(a.path(), "recipes/s.toml", &recipe("Only a", "2"));
    put(b.path(), "recipes/r.toml", &recipe("Power", "3"));
    put(b.path(), "recipes/broken.toml", "[[");
    let dirs = vec![a.path().to_owned(), b.path().to_owned()];
    let got = config::shared_recipes(&dirs);
    assert_eq!(
        got.iter().map(|r| (r.name.as_str(), r.expr.as_str())).collect::<Vec<_>>(),
        [("Power", "3"), ("Only a", "2")]
    );

    let set = |name: &str| format!("schema = 1\nname = \"{name}\"\n[[check]]\nname = \"c\"\nexpect = \"x > 0\"\n");
    put(a.path(), "checks/one.toml", &set("One"));
    put(b.path(), "checks/one.toml", &set("One").replace("x > 0", "x > 5"));
    put(b.path(), "checks/bad.toml", "[[");
    let sets = config::shared_sets(&dirs);
    assert_eq!((sets.len(), sets[0].checks[0].expect.as_str()), (1, "x > 5"));

    // a profile by header: the later folder wins, and one that isn't the header's, or is of another schema, is skipped
    let header: Vec<String> = vec!["t".into(), "x".into()];
    let cache = tempfile::tempdir().unwrap();
    let p1 = ImportProfile { prefix: Some(PrefixRule::Custom { text: "one".into() }), ..Default::default() };
    let p2 = ImportProfile { prefix: Some(PrefixRule::Custom { text: "two".into() }), ..Default::default() };
    for (dir, p) in [(&a, &p1), (&b, &p2)] {
        profile::save(cache.path(), &header, p).unwrap();
        let items = config::cache_items(cache.path(), &Own::default());
        config::share(dir.path(), &items).unwrap();
    }
    put(b.path(), "profiles/other.toml", "schema = 9\nheader = [\"t\", \"x\"]\n[profile]\n");
    put(b.path(), "profiles/junk.toml", "x");
    assert_eq!(config::shared_profile(&dirs, &header), Some(p2.clone()));
    assert_eq!(config::shared_profile(&dirs[..1], &header), Some(p1));
    assert_eq!(config::shared_profile(&dirs, &["other".to_string()]), None);
    let text = fs::read_dir(b.path().join("profiles"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| fs::read_to_string(p).unwrap().contains("two"));
    assert_eq!(config::profile_from_toml(&fs::read_to_string(text.unwrap()).unwrap()).unwrap(), p2);
    assert!(config::profile_from_toml("schema = 9\nheader = []\n[profile]\n").unwrap_err().contains("schema 9"));
    assert!(config::profile_from_toml("x").is_err());
    let bad = ImportProfile { prefix: Some(PrefixRule::Pattern { pattern: "(".into() }), ..Default::default() };
    profile::save(cache.path(), &["q".into()], &bad).unwrap();
    let items = config::cache_items(cache.path(), &Own::default());
    let q = items.iter().find(|i| i.name == "q").unwrap();
    assert!(
        config::profile_from_toml(&String::from_utf8(q.files[0].1.clone()).unwrap()).unwrap_err().contains("prefix")
    );

    // shared events: matched by identity or name, an old schema skipped, the first folder's event of an id kept
    let ev = |id: &str, name: &str| format!("[[event]]\nid = \"{id}\"\nname = \"{name}\"\nstart = 5\n");
    put(
        a.path(),
        "events/a.toml",
        &format!("schema = 1\nrecording = \"a.csv\"\nidentity = \"aa\"\n{}", ev("e1", "first")),
    );
    put(
        b.path(),
        "events/a.toml",
        &format!("schema = 1\nrecording = \"a.csv\"\nidentity = \"zz\"\n{}{}", ev("e1", "second"), ev("e2", "other")),
    );
    put(
        b.path(),
        "events/old.toml",
        &format!("schema = 3\nrecording = \"a.csv\"\nidentity = \"aa\"\n{}", ev("e9", "old")),
    );
    put(b.path(), "events/broken.toml", "[[");
    let got = config::shared_events(&dirs, "aa", "a.csv");
    assert_eq!(
        got.iter().map(|e| (e.id.as_str(), e.name.as_str())).collect::<Vec<_>>(),
        [("e1", "first"), ("e2", "other")]
    );
    assert!(got.iter().all(|e| e.source == "shared" && e.start == "5"));
    assert!(config::shared_events(&dirs, "none", "").is_empty(), "an empty name matches nothing");

    config::set_overlay(dirs.clone());
    assert_eq!(config::overlay(), dirs);
    config::set_overlay(Vec::new());
    assert!(config::overlay().is_empty());
}

/// A recording renamed: offering the new name, and the update's errors.
#[test]
fn updating_renamed_recordings_reports_files_that_cannot_be_read_or_written() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path();
    let file = |name: &str, identity: &str| format!("schema = 1\nrecording = \"{name}\"\nidentity = \"{identity}\"\n");
    put(dir, "events/a.toml", &file("old/a.csv", "aa"));
    put(dir, "events/broken.toml", "[[");
    put(dir, "events/future.toml", &file("old/f.csv", "ff").replace("schema = 1", "schema = 2"));
    put(dir, "events/noid.toml", &file("old/n.csv", ""));
    let recs = [
        Recording { name: "new/a.csv".into(), identity: "aa".into(), file: None },
        Recording { name: "new/f.csv".into(), identity: "ff".into(), file: None },
        Recording { name: "new/n.csv".into(), identity: String::new(), file: None },
        Recording { name: String::new(), identity: "aa".into(), file: None },
    ];
    let found = config::renamed_recordings(dir, &recs);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].from.as_str(), found[0].to.as_str()), ("old/a.csv", "new/a.csv"));
    assert_eq!(config::update_renamed(dir, &recs).unwrap(), 1);
    // a read-only file can't be written: the error names it and nothing else changed
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        put(dir, "events/b.toml", &file("old/b.csv", "bb"));
        let b = [Recording { name: "new/b.csv".into(), identity: "bb".into(), file: None }];
        let path = dir.join("events/b.toml");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        if unsafe { libc::geteuid() } != 0 {
            let e = config::update_renamed(dir, &b).unwrap_err();
            assert!(e.contains("can't write") && e.contains("b.toml"), "{e}");
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }
}

/// Where each kind lives, and the settings' kinds read from a folder on disk (icons through its listing).
#[test]
fn every_kind_has_its_place_and_a_folder_gives_its_icons_widgets_and_categories() {
    let paths: Vec<&str> = Kind::ALL.iter().map(|k| k.path()).collect();
    assert_eq!(
        paths,
        [
            "views",
            "events",
            "checks",
            "categories.toml",
            "recipes",
            "profiles",
            "decoders",
            "widgets",
            "icons",
            "timemaps"
        ]
    );
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "icons/dial.svg", "<svg id=\"dial\"/>");
    put(d.path(), "icons/readme.txt", "no");
    put(
        d.path(),
        "widgets/lever/widget.toml",
        "schema = 1\nname = \"Lever\"\nkind = \"svg\"\nentry = \"widget.svg\"\n",
    );
    put(d.path(), "widgets/lever/widget.svg", "<svg/>");
    put(d.path(), "widgets/empty/other.txt", "x");
    put(d.path(), "categories.toml", "schema = 1\n[[category]]\nid = \"a\"\nname = \"A\"\ncolor = \"#fff\"\n");
    let got = config::from_folder(d.path());
    assert_eq!(got.icons, vec![Icon { name: "dial".into(), svg: "<svg id=\"dial\"/>".into() }]);
    assert_eq!(got.widgets.iter().map(|w| (w.id.as_str(), w.name.as_str())).collect::<Vec<_>>(), [("lever", "Lever")]);
    assert_eq!((got.categories.len(), got.categories[0].show_on_charts), (1, true));
}

/// Events at the same moment keep a stable order (by id) in the file; two of them in a shared file merge by id.
#[test]
fn events_at_one_moment_are_written_in_id_order_and_merge_over_a_damaged_file() {
    let cache = tempfile::tempdir().unwrap();
    let rec = Recording { name: "a.csv".into(), identity: "aaaa".into(), ..Default::default() };
    let own = Own { recordings: vec![rec], ..Default::default() };
    for id in ["b", "c", "a"] {
        events::save(cache.path(), "aaaa", user(id, 50)).unwrap();
    }
    let out = tempfile::tempdir().unwrap();
    config::share(out.path(), &config::cache_items(cache.path(), &own)).unwrap();
    let text = fs::read_to_string(out.path().join("events/a-csv.toml")).unwrap();
    let at = |id: &str| text.find(&format!("id = \"{id}\"")).unwrap();
    assert!(at("a") < at("b") && at("b") < at("c"), "{text}");
    // read back in the same order
    let (items, _) = config::read_folder(out.path());
    assert_eq!(String::from_utf8(items[0].files[0].1.clone()).unwrap(), text);
    // a folder file that has been damaged is replaced by yours rather than merged
    fs::write(out.path().join("events/a-csv.toml"), "[[").unwrap();
    let written = config::share(out.path(), &config::cache_items(cache.path(), &own)).unwrap();
    assert_eq!(written, ["events/a-csv.toml"]);
    assert_eq!(fs::read_to_string(out.path().join("events/a-csv.toml")).unwrap(), text);
}

/// A file with different recording identity but the same events is the same to the Share dialog.
#[test]
fn event_files_that_differ_only_in_identity_are_the_same_rows() {
    let cache = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    events::save(cache.path(), "aaaa", user("e1", 5)).unwrap();
    let own = |identity: &str| Own {
        recordings: vec![Recording { name: "a.csv".into(), identity: identity.into(), ..Default::default() }],
        ..Default::default()
    };
    config::share(out.path(), &config::cache_items(cache.path(), &own("aaaa"))).unwrap();
    let (folder, _) = config::read_folder(out.path());
    let cache2 = tempfile::tempdir().unwrap();
    events::save(cache2.path(), "zzzz", user("e1", 5)).unwrap();
    let rows = config::compare(&config::cache_items(cache2.path(), &own("zzzz")), &folder);
    assert_eq!((rows[0].state, rows[0].detail.clone()), (State::Same, None), "{rows:?}");
}

/// Views: a layout that is not an object compares as itself; a new layout's own files and author beat the old view's;
/// sharing over a file that isn't JSON writes the new one.
#[test]
fn view_files_that_are_not_objects_or_not_json_are_handled() {
    let d = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    put(d.path(), "views/list.tlmview.json", "[1, 2]");
    let (folder, problems) = config::read_folder(d.path());
    assert!(problems.is_empty());
    let own = |v: serde_json::Value| Own {
        views: vec![View { file: "list.tlmview.json".into(), view: v }],
        ..Default::default()
    };
    let rows = config::compare(&config::cache_items(cache.path(), &own(serde_json::json!([1, 2]))), &folder);
    assert_eq!(rows[0].state, State::Same);
    // the new layout has its own author: the old view's is not carried over
    put(d.path(), "views/f.tlmview.json", r#"{"name":"F","by":"old","files":["x"],"panes":[]}"#);
    let mine = Own {
        views: vec![View {
            file: "f.tlmview.json".into(),
            view: serde_json::json!({"name":"F","by":"new","panes":[1]}),
        }],
        ..Default::default()
    };
    config::share(d.path(), &config::cache_items(cache.path(), &mine)).unwrap();
    let text = fs::read_to_string(d.path().join("views/f.tlmview.json")).unwrap();
    assert!(text.contains("\"new\"") && !text.contains("\"old\"") && text.contains("\"files\""), "{text}");
    // over a file that isn't JSON
    put(d.path(), "views/g.tlmview.json", "{{{");
    let mine = Own {
        views: vec![View { file: "g.tlmview.json".into(), view: serde_json::json!({"name":"G","by":"me"}) }],
        ..Default::default()
    };
    config::share(d.path(), &config::cache_items(cache.path(), &mine)).unwrap();
    assert!(fs::read_to_string(d.path().join("views/g.tlmview.json")).unwrap().contains("\"me\""));
}

/// Files that can't be read as text are skipped by the folders read besides the cache, and listed as problems by the
/// folder reader; `nearest` copes with a path that doesn't exist; `writable` with one that can't be a C string.
#[test]
fn unreadable_text_is_skipped_or_listed_and_odd_paths_are_answered() {
    let d = tempfile::tempdir().unwrap();
    let not_text = [0xffu8, 0xfe, 0x00, 0x80];
    for rel in ["recipes/x.toml", "checks/x.toml", "profiles/x.toml", "events/x.toml", "timemaps/x.toml"] {
        let p = d.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, not_text).unwrap();
    }
    let dirs = vec![d.path().to_owned()];
    assert!(config::shared_recipes(&dirs).is_empty() && config::shared_sets(&dirs).is_empty());
    assert!(config::shared_profile(&dirs, &["a".to_string()]).is_none());
    assert!(config::shared_events(&dirs, "a", "a").is_empty());
    assert!(
        config::renamed_recordings(d.path(), &[Recording { name: "n".into(), identity: "i".into(), file: None }])
            .is_empty()
    );
    let (_, problems) = config::read_folder(d.path());
    assert_eq!(problems.len(), 5, "{problems:?}");
    assert!(why(&problems, "recipes/x.toml").contains("UTF-8"));

    // a path that isn't there: its nearest folder is found through the parents as given
    std::fs::create_dir_all(d.path().join("proj/.tlmbench")).unwrap();
    let missing = d.path().join("proj/not/here/a.csv");
    let found = config::nearest(&missing).unwrap();
    assert!(found.ends_with("proj/.tlmbench"), "{found:?}");
    assert!(config::nearest(Path::new("/definitely/not/anywhere/a.csv")).is_none());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        assert!(!config::writable(Path::new(std::ffi::OsStr::from_bytes(b"x\0/y"))));
    }
}

/// A check whose expression doesn't parse is a problem of its file, with where.
#[test]
fn validate_finds_a_check_that_does_not_parse() {
    let d = tempfile::tempdir().unwrap();
    put(
        d.path(),
        "checks/c.toml",
        "schema = 1\nname = \"S\"\n[[check]]\nname = \"bad\"\nexpect = \"rpm <\"\n[[check]]\nname = \"ok\"\nexpect = \"rpm < 5\"\n",
    );
    let problems = config::validate(d.path());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "checks/c.toml");
    assert!(problems[0].why.starts_with("check \"bad\": "), "{}", problems[0].why);
}

/// A write that can't happen is an error naming the path, and pulling into a cache that can't be written fails the same.
#[test]
fn writes_that_fail_name_the_path() {
    let out = tempfile::tempdir().unwrap();
    let icon = |name: &str| {
        config::cache_items(
            out.path(),
            &Own { icons: vec![Icon { name: name.into(), svg: "<svg/>".into() }], ..Default::default() },
        )
    };
    // a folder where the temp file goes
    fs::create_dir_all(out.path().join("dest/icons/a.tmp")).unwrap();
    let e = config::share(&out.path().join("dest"), &icon("a")).unwrap_err().to_string();
    assert!(e.contains("a.tmp"), "{e}");
    // a non-empty folder where the file goes: the rename fails
    fs::create_dir_all(out.path().join("dest/icons/b.svg/inner")).unwrap();
    let e = config::share(&out.path().join("dest"), &icon("b")).unwrap_err().to_string();
    assert!(e.contains("b.svg"), "{e}");

    // pulling into a cache under a plain file
    let file = out.path().join("plain");
    fs::write(&file, "x").unwrap();
    let cache = file.join("cache");
    let recipe = "schema = 1\n[[recipe]]\nname = \"P\"\nexpr = \"1\"\n";
    let item = |kind: Kind, rel: &str, body: &str| config::Item {
        kind,
        name: "x".into(),
        files: vec![(rel.into(), body.as_bytes().to_vec())],
    };
    let e = config::pull(&cache, &[item(Kind::Recipes, "recipes/p.toml", recipe)], &Own::default()).unwrap_err();
    assert!(e.to_string().contains("recipes"), "{e}");
    let prof = tlmbench_decode::profile::ImportProfile::default();
    let src = tempfile::tempdir().unwrap();
    profile::save(src.path(), &["a".into()], &prof).unwrap();
    let pitem =
        config::cache_items(src.path(), &Own::default()).into_iter().find(|i| i.kind == Kind::Profiles).unwrap();
    assert!(config::pull(&cache, &[pitem], &Own::default()).is_err());
    let ev = "schema = 1\nrecording = \"a\"\nidentity = \"i\"\n[[event]]\nid = \"e\"\nname = \"n\"\nstart = 1\n";
    assert!(config::pull(&cache, &[item(Kind::Events, "events/a.toml", ev)], &Own::default()).is_err());
}

/// An icon that can't be read is left out of the folder's items; a widget folder whose stale file can't go is an error.
#[cfg(unix)]
#[test]
fn permissions_stop_reading_an_icon_and_removing_a_stale_widget_file() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return; // root reads and removes anything
    }
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "icons/open.svg", "<svg/>");
    put(d.path(), "icons/locked.svg", "<svg/>");
    let locked = d.path().join("icons/locked.svg");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let (items, _) = config::read_folder(d.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["open"]);

    let out = tempfile::tempdir().unwrap();
    let w = Widget { id: "w".into(), name: "Gauge".into(), svg: Some("<svg/>".into()), ..Default::default() };
    let items = config::cache_items(out.path(), &Own { widgets: vec![w], ..Default::default() });
    config::share(out.path(), &items).unwrap();
    let folder = out.path().join("widgets/gauge");
    put(out.path(), "widgets/gauge/stale.txt", "x");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let e = config::share(out.path(), &items);
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(e.unwrap_err().to_string().contains("stale.txt"));
}
