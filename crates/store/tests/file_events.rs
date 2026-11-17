//! Events from the file (D-ui-12): a marked column makes one event at each change of its value.

use std::fs;

use tlmbench_store::{Store, events};

#[test]
fn a_marked_column_makes_an_event_at_each_change() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = data.path().join("run.csv");
    fs::write(&path, "t,mode,x\n0,0,1\n1,0,2\n2,1,3\n3,1,4\n4,2,5\n5,2,6\n6,1,7\n").unwrap();
    let store = Store::new(Some(cache.path()));
    let opened = store.open(&path, None).unwrap();
    let rec = opened.recording();
    let mode = rec.info().channels.iter().position(|c| c.name == "mode").unwrap();
    let got = events::from_channels(rec, &[mode]);
    let at: Vec<(String, String)> = got.iter().map(|e| (e.start.clone(), e.name.clone())).collect();
    assert_eq!(
        at,
        vec![("2000000000".into(), "1".into()), ("4000000000".into(), "2".into()), ("6000000000".into(), "1".into())],
        "the first sample is the starting state, each change after it an event"
    );
    assert!(got.iter().all(|e| e.source == "file" && e.channels == ["mode"] && e.duration == "0"));
    assert!(events::from_channels(rec, &[]).is_empty());
}
