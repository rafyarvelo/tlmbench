//! Smaller paths of the store's `Opened` (coverage, R6c.3, Nov 26): the sample nearest a time at the ends and in an
//! empty file, table columns of another file with nothing to read them, a cancelled filter, and a joined recording's
//! build, whose progress counts both parts.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use tlmbench_core::table::{Col, Filter};
use tlmbench_expr::Recipe;
use tlmbench_store::{Status, Store};

fn csv(dir: &Path, name: &str, rows: usize) -> std::path::PathBuf {
    let mut t = String::from("t,a\n");
    for i in 0..rows {
        t.push_str(&format!("{},{}\n", i, i * 10));
    }
    let p = dir.join(name);
    fs::write(&p, t).unwrap();
    p
}

#[test]
fn the_sample_nearest_a_time_at_the_ends_of_a_recording_and_in_an_empty_one() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::new(Some(&d.path().join("cache")));
    let o = store.open(&csv(d.path(), "a.csv", 5), None).unwrap();
    let (t0, t1) = (o.recording().info().t_first.unwrap(), o.recording().info().t_last.unwrap());
    let at = |t| o.values_at(&[0], t).map(|(i, t, v)| (i, t, v[0]));
    assert_eq!(at(t0 - 5_000_000_000).map(|x| x.0), Some(0), "before the first sample: the first");
    assert_eq!(at(t1 + 5_000_000_000).map(|x| x.0), Some(4), "after the last: the last");
    assert_eq!(at(t0 + 1_400_000_000).map(|x| x.0), Some(1), "between two: the nearer");
    assert_eq!(o.nearest(0, t0 + 3_600_000_000).map(|x| (x.0, x.2)), Some((4, 40.0)));
    let empty = store
        .open(
            &{
                let p = d.path().join("e.csv");
                fs::write(&p, "t,a\n").unwrap();
                p
            },
            None,
        )
        .unwrap();
    assert!(empty.values_at(&[0], 0).is_none());
}

#[test]
fn a_column_of_another_file_with_nothing_to_read_it_is_blank_and_a_cancelled_filter_gives_nothing() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::new(Some(&d.path().join("cache")));
    let o = store.open(&csv(d.path(), "a.csv", 20), None).unwrap();
    let all = (i64::MIN / 2, i64::MAX / 2);
    let go = |_: f64| true;
    let f = |t: &str| Filter::parse(t).unwrap().unwrap();
    // column 1 is past the file's own and nothing reads it: every row is blank, and a blank passes no filter
    let (_, rows) = o.select_rows(all, vec![(Col::Channel(1), f(">= 0"))], Vec::new(), &go).unwrap();
    assert!(rows.ids.is_empty());
    let (_, rows) = o.select_rows(all, vec![(Col::Channel(0), f(">= 100"))], Vec::new(), &go).unwrap();
    assert_eq!(rows.ids, (10..20).collect::<Vec<u32>>());
    // the held column is read when there is one
    let held = |c: usize, times: &[i64]| vec![c as f64; times.len()];
    let (_, rows) = o.select_rows_with(all, vec![(Col::Channel(1), f(">= 1"))], Vec::new(), Some(&held), &go).unwrap();
    assert_eq!(rows.ids.len(), 20);
    // cancelled: the mask is nothing, not an empty mask
    assert!(o.filter_mask(all, vec![(Col::Channel(0), f(">= 100"))], None, &|_| false).is_none());
    assert!(o.filter_mask(all, vec![(Col::Channel(0), f(">= 100"))], None, &go).is_some());
}

#[test]
fn building_a_joined_recording_builds_both_parts_and_counts_them_together() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::new(Some(&d.path().join("cache")));
    let base = Arc::new(store.open(&csv(d.path(), "a.csv", 3000), None).unwrap());
    let recipe = Recipe {
        name: "Twice".into(),
        expr: "a * 2".into(),
        unit: None,
        category: None,
        decimals: None,
        automatic: true,
    };
    let derived = store.open_derived(&base, &[recipe]).unwrap().opened.unwrap();
    let joined = store.join(base.clone(), Arc::new(derived)).unwrap();
    assert_eq!(joined.status(), Status::Missing);
    let mut seen = Vec::new();
    joined.build(&mut |done, total| seen.push((done, total))).unwrap();
    assert_eq!(joined.status(), Status::Built);
    assert_eq!(base.status(), Status::Built);
    assert!(!seen.is_empty() && seen.iter().all(|&(d, t)| t == 3000 && d <= t), "{seen:?}");
    assert!(seen.iter().any(|&(d, _)| d <= 1500), "the base counts first");
    assert!(seen.iter().any(|&(d, _)| d > 1500), "then the derived channels");
    // built already: nothing more is done
    let mut again = 0;
    joined.build(&mut |_, _| again += 1).unwrap();
    assert_eq!(again, 0);
}
