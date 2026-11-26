//! Files that can't be read, written or removed, in the places the store keeps them (coverage, R6c.3, Nov 26): the
//! recipe library, a preview that reads value labels and limits, and the events and trigger files.

use std::fs;

use tlmbench_expr::Recipe;
use tlmbench_store::{Error, Library, Store};

fn recipe(name: &str, expr: &str) -> Recipe {
    Recipe { name: name.into(), expr: expr.into(), unit: None, category: None, decimals: None, automatic: true }
}

#[test]
fn a_recipe_library_that_cannot_be_written_or_read_says_so() {
    let d = tempfile::tempdir().unwrap();
    // under a plain file: the folder can't be made
    let file = d.path().join("plain");
    fs::write(&file, "x").unwrap();
    let e = Library::new(&file.join("cache")).save(&recipe("A", "1"), None).unwrap_err();
    assert!(matches!(e, Error::Io { .. }) && e.to_string().contains("plain"), "{e}");
    // a file that isn't text is a problem with why, and no recipe
    let root = d.path().join("root");
    let lib = Library::new(&root);
    lib.save(&recipe("Kept", "1"), None).unwrap();
    fs::write(root.join("recipes/binary.toml"), [0xffu8, 0xfe, 0x00]).unwrap();
    assert_eq!(lib.problems().len(), 1);
    assert!(lib.problems()[0].1.contains("UTF-8"), "{:?}", lib.problems());
    assert_eq!(lib.list().len(), 1);
    // a folder where the temporary file goes
    fs::create_dir_all(root.join("recipes/b.tmp")).unwrap();
    let e = lib.save(&recipe("B", "2"), None).unwrap_err().to_string();
    assert!(e.contains("b.tmp"), "{e}");
    // importing reports the save that failed, by its message
    let text = tlmbench_expr::recipe::to_toml(&[recipe("B", "2")]);
    assert!(lib.import(&text).unwrap_err().contains("b.tmp"));
}

#[cfg(unix)]
#[test]
fn a_read_only_recipe_folder_stops_a_delete_and_a_rename() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let lib = Library::new(d.path());
    let dir = d.path().join("recipes");
    lib.save(&recipe("Old", "1"), None).unwrap();
    // a file of two recipes, and one of one
    fs::write(dir.join("pair.toml"), tlmbench_expr::recipe::to_toml(&[recipe("One", "a"), recipe("Two", "b")]))
        .unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    fs::set_permissions(dir.join("pair.toml"), fs::Permissions::from_mode(0o444)).unwrap();
    let removed = lib.delete("Old");
    let rewritten = lib.delete("One");
    let renamed = lib.save(&recipe("New", "1"), Some("Old"));
    fs::set_permissions(dir.join("pair.toml"), fs::Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(removed.unwrap_err().to_string().contains("old.toml"));
    assert!(rewritten.unwrap_err().to_string().contains("pair.toml"));
    assert!(renamed.is_err());
    assert!(lib.get("Old").is_some() && lib.get("One").is_some(), "nothing was lost");
}

/// The Derive dialog's preview sees a text channel's labels, and says what is wrong with a limit nobody set.
#[test]
fn a_preview_reads_value_labels_and_reports_missing_limits() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("s.csv");
    fs::write(&src, "t,state,x\n0,idle,1\n1,run,2\n2,run,3\n3,idle,4\n").unwrap();
    let store = Store::new(Some(&d.path().join("cache")));
    let o = store.open(&src, None).unwrap();
    let p = tlmbench_store::check(o.recording(), "state == 'run'").unwrap();
    assert_eq!((p.samples, p.values, p.min, p.max), (4, 4, Some(0.0), Some(1.0)));
    assert_eq!(p.mean, Some(0.5));
    let e = tlmbench_store::check(o.recording(), "past_warn(x)").unwrap_err();
    assert_eq!(e.msg, "x has no warn limits");
    let e = tlmbench_store::check(o.recording(), "state == 'nope'").unwrap_err();
    assert!(e.msg.contains("no value called 'nope'"), "{}", e.msg);
}
