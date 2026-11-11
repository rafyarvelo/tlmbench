//! Reading the plugins folders: what loads, what is turned off, what isn't allowed, and a native plugin once it is on.

use std::fs;
use std::path::Path;

use tlmbench_decode::{OpenOptions, open};
use tlmbench_plugin::{Found, Kind, Status, Tier, install};

mod common;

fn copy(from: &Path, dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::copy(from, dir.join(from.file_name().unwrap())).unwrap();
}

fn by_file<'a>(f: &'a [Found], name: &str) -> &'a Found {
    f.iter().find(|f| f.file == name).unwrap_or_else(|| panic!("{name} not listed in {f:?}"))
}

#[test]
fn folders_are_read_native_plugins_wait_to_be_turned_on_and_shared_ones_are_only_sandboxed() {
    let dec = common::build("template-decoder", "my_decoder.wasm");
    let est = common::build("template-estimator", "my_estimator.wasm");
    let nat = common::build_native("test-native", "test_native");
    let nat_name = nat.file_name().unwrap().to_string_lossy().into_owned();

    let dir = tempfile::tempdir().unwrap();
    let (own, shared) = (dir.path().join("own"), dir.path().join("shared/.tlmbench/plugins"));
    for p in [&dec, &est, &nat] {
        copy(p, &own);
    }
    fs::write(own.join("junk.wasm"), b"not a component").unwrap();
    fs::write(own.join("notes.txt"), b"ignored").unwrap();
    fs::write(own.join(".hidden.wasm"), b"ignored").unwrap();
    copy(&nat, &shared);
    copy(&est, &shared);

    let nat_file = dir.path().join("run.nat");
    let mut bytes = b"NAT1".to_vec();
    for v in [1.5f64, 2.5, 3.5, 4.5, 5.5] {
        bytes.extend(v.to_le_bytes());
    }
    fs::write(&nat_file, bytes).unwrap();
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };

    // nothing turned on: the native plugin is listed, not loaded, and its files stay unknown
    let got = install(Some(&own), std::slice::from_ref(&shared), &[]).found;
    let wasm_dec = by_file(&got, "my_decoder.wasm");
    assert_eq!((wasm_dec.kind, wasm_dec.tier, &wasm_dec.status), (Some(Kind::Decoder), Tier::Wasm, &Status::Loaded));
    assert_eq!((wasm_dec.name.as_str(), wasm_dec.version.as_deref()), ("demo", Some("0.1.0")));
    assert_eq!(by_file(&got, "my_estimator.wasm").kind, Some(Kind::Estimator));
    let off = by_file(&got, &nat_name);
    assert_eq!((off.tier, &off.status), (Tier::Native, &Status::Off));
    assert!(matches!(&by_file(&got, "junk.wasm").status, Status::Failed(w) if w.contains("not a plugin component")));
    assert!(got.iter().all(|f| f.file != "notes.txt" && f.file != ".hidden.wasm"));
    // the folder's native plugin is refused whatever is turned on; its sandboxed one is fine
    let shared_native = got.iter().find(|f| f.shared && f.tier == Tier::Native).unwrap();
    assert!(matches!(&shared_native.status, Status::Failed(w) if w.contains("only from your own")));
    assert!(got.iter().any(|f| f.shared && f.tier == Tier::Wasm && f.status == Status::Loaded));
    assert!(open(&nat_file, &opts).is_err());

    // turned on (by file name): it loads and decodes
    let got = install(Some(&own), std::slice::from_ref(&shared), std::slice::from_ref(&nat_name)).found;
    let on = got.iter().find(|f| f.file == nat_name && !f.shared).unwrap();
    assert_eq!((on.tier, &on.status, on.name.as_str()), (Tier::Native, &Status::Loaded, "native-test"));
    assert_eq!(on.version.as_deref(), Some("0.2.0"));
    let rec = open(&nat_file, &opts).unwrap();
    assert_eq!(rec.info().format, "native-test (plugin)");
    assert_eq!(rec.info().samples, 5);
    assert_eq!(rec.info().channels[0].unit.as_deref(), Some("m"));
    assert_eq!(rec.value_at(0, 4, &Default::default()), 5.5);
    assert_eq!(rec.info().t_last, Some(4_000_000));

    // and turned off again
    install(Some(&own), &[], &[]);
    assert!(open(&dir.path().join("other.nat"), &opts).is_err());
    install(None, &[], &[]);
}
