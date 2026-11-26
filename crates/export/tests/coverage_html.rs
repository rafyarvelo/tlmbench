//! The HTML export's smaller paths and the SVG sanitiser's odd inputs (coverage, R6c.3, Nov 26): detail spellings, a
//! viewer pack cut short, the viewer written to a folder, one-file exports over their limit, recordings without a time
//! column, and tags a browser would still read that a naive filter would not.

use std::fs;
use std::path::Path;

use serde_json::json;
use tlmbench_core::Recording;
use tlmbench_decode::OpenOptions;
use tlmbench_export::html::{self, Mode, Window};
use tlmbench_export::widgets::{export_widgets, sanitize_svg};

const S: i64 = 1_000_000_000;

fn open(path: &Path, cache: &Path) -> Box<dyn Recording> {
    tlmbench_decode::open(
        path,
        &OpenOptions {
            descriptor: None,
            part: None,
            profile: None,
            cache_dir: Some(cache.to_owned()),
            adjust: Vec::new(),
        },
    )
    .unwrap()
}

#[test]
fn detail_is_read_in_every_spelling_and_refused_with_why() {
    let w = |t: &str| Window::parse(t).unwrap().unwrap();
    for off in ["0", "off", "OFF", "None", " 0 "] {
        assert_eq!(Window::parse(off).unwrap(), None, "{off}");
    }
    // the factor in any order and with either times sign; empty parts are ignored
    assert_eq!(w("3×").factor, 3.0);
    assert_eq!((w("1.5X, 20s ,").factor, w("1.5X,20s").min_ns), (1.5, 20 * S));
    assert_eq!(w("0x").factor, 0.0);
    // units
    assert_eq!(w("500ms").min_ns, S / 2);
    assert_eq!(w("2h").min_ns, 7200 * S);
    assert_eq!(w("5m").min_ns, 300 * S);
    assert_eq!(w("10min").min_ns, 600 * S);
    assert_eq!(w("90").min_ns, 90 * S);
    assert_eq!(w(" 1.5 s ").min_ns, 1_500_000_000);
    // the cap is never under the minimum
    let c = w("100s,5s");
    assert_eq!((c.min_ns, c.cap_ns), (100 * S, 100 * S));
    assert_eq!(w("1s,2s").cap_ns, 2 * S);
    // refusals say what they didn't understand
    for bad in ["-1x", "x", "fastx", "-5s", "5 parsecs", "1s,2s,3s"] {
        assert!(Window::parse(bad).is_err(), "{bad}");
    }
    assert!(Window::parse("fastx").unwrap_err().contains("not a factor like 3x"));
    assert!(Window::parse("5 parsecs").unwrap_err().contains("not a duration"));
    assert!(Window::parse("1s,2s,3s").unwrap_err().contains("more than a factor"));
}

#[test]
fn a_viewer_pack_cut_short_gives_the_files_before_the_cut_and_a_viewer_is_written_to_a_folder() {
    let pack = |files: &[(&str, &[u8])]| -> Vec<u8> {
        let mut b = Vec::new();
        for (p, d) in files {
            b.extend((p.len() as u32).to_le_bytes());
            b.extend(p.as_bytes());
            b.extend((d.len() as u32).to_le_bytes());
            b.extend(*d);
        }
        b
    };
    let whole = pack(&[("index.html", b"<html>"), ("assets/app.js", b"1;")]);
    let got = html::unpack(&whole);
    assert_eq!(got, vec![("index.html".to_owned(), b"<html>".to_vec()), ("assets/app.js".to_owned(), b"1;".to_vec())]);
    assert!(html::unpack(&[]).is_empty());
    // cut in the middle of the second file's data, its length, its path, its path length, and the first's data
    for cut in [whole.len() - 1, whole.len() - 3, 4 + 10 + 4 + 6 + 4 + 5, 4 + 10 + 4 + 6 + 2, 4 + 10 + 4 + 3] {
        let got = html::unpack(&whole[..cut]);
        assert!(got.len() <= 1, "cut at {cut}: {got:?}");
        assert!(got.iter().all(|(p, _)| p == "index.html"));
    }
    assert_eq!(html::unpack(&whole).len(), 2);

    let tmp = tempfile::tempdir().unwrap();
    html::write_viewer(tmp.path(), &got).unwrap();
    assert_eq!(fs::read(tmp.path().join("assets/app.js")).unwrap(), b"1;");
    for bad in ["../x", "a/../b", "/abs", "a//b", ""] {
        let e = html::write_viewer(tmp.path(), &[(bad.to_owned(), vec![1])]).unwrap_err().to_string();
        assert!(e.contains("the viewer names"), "{bad}: {e}");
    }
    assert!(!tmp.path().join("x").exists());
    // a file where a folder is needed
    fs::write(tmp.path().join("plain"), "x").unwrap();
    assert!(html::write_viewer(tmp.path(), &[("plain/x.js".to_owned(), vec![1])]).is_err());
    // the viewers built into this program are a pack: whatever they hold is named by safe, relative paths
    for v in [html::embedded_viewer(), html::embedded_single_viewer()] {
        assert!(v.iter().all(|(p, _)| !p.starts_with('/') && !p.split('/').any(|s| s == ".." || s.is_empty())));
    }
}

#[test]
fn a_one_file_export_that_does_not_fit_is_refused_and_leaves_no_folder_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let mut csv = String::from("t,a\n");
    for i in 0..2000 {
        csv.push_str(&format!("{},{}\n", i as f64 / 100.0, (i * 7 % 13) as f64));
    }
    let src = tmp.path().join("r.csv");
    fs::write(&src, csv).unwrap();
    let rec = open(&src, &tmp.path().join("cache"));
    let file = || html::File { name: "r".into(), rec: rec.as_ref(), channels: vec![0], raw: vec![], windows: vec![] };
    let viewer = vec![
        ("index.html".to_owned(), b"<body><!--TLM_FILES--></body>".to_vec()),
        ("core.wasm".to_owned(), vec![0u8; 3000]),
    ];
    assert_eq!(html::single_overhead(&viewer), 29 + 4000);
    assert_eq!(html::single_room(&viewer, 10_000), (10_000 - 4029) / 4 * 3);
    assert_eq!(html::single_room(&viewer, 10), 0, "a limit under the viewer leaves no room");
    let out = tmp.path().join("deep/er/one.html");
    // a limit the page can't meet: the error says the size and the limit, nothing is written
    let before: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap().flatten().map(|e| e.file_name()).collect();
    let e = html::export_single(&out, &[file()], json!({}), &html::Options::default(), 5000, &viewer, &mut |_, _| {})
        .unwrap_err()
        .to_string();
    assert!(e.contains("MB but the limit is 0 MB"), "{e}");
    assert!(!out.exists());
    let after: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(after.iter().all(|n| !n.to_string_lossy().starts_with("tlmbench-single-") || before.contains(n)));
    // one that fits is written, in folders made for it, and says its size
    let (report, bytes) =
        html::export_single(&out, &[file()], json!({}), &html::Options::default(), 1 << 20, &viewer, &mut |_, _| {})
            .unwrap();
    assert_eq!(fs::metadata(&out).unwrap().len(), bytes);
    assert_eq!(report.mode, Mode::Full);
    // a viewer with no page, and an export of nothing
    let e = html::single_file(tmp.path(), &viewer[1..]).unwrap_err().to_string();
    assert!(e.contains("has no page"), "{e}");
    let e = html::export(&tmp.path().join("none"), &[], json!({}), &html::Options::default()).unwrap_err();
    assert_eq!(e.to_string(), "nothing to export");
    // the manifest's folder can't be made under a file
    let blocker = tmp.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    assert!(html::export(&blocker.join("out"), &[file()], json!({}), &html::Options::default()).is_err());
}

#[test]
fn a_span_and_a_recording_without_time_are_exported_as_they_are() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("n.csv");
    fs::write(&plain, "label,v\nx,1\ny,2\nz,3\nw,4\n").unwrap();
    let rec = open(&plain, &tmp.path().join("cache"));
    assert!(matches!(rec.info().time, tlmbench_core::TimeSource::Index));
    let file = || html::File { name: "n".into(), rec: rec.as_ref(), channels: vec![0], raw: vec![], windows: vec![] };
    let out = tmp.path().join("site");
    html::export(&out, &[file()], json!({}), &html::Options::default()).unwrap();
    let m: serde_json::Value = serde_json::from_slice(&fs::read(out.join("export.json")).unwrap()).unwrap();
    assert_eq!(m["files"][0]["samples"], 4);
    assert_eq!(m["files"][0]["time"]["kind"], "rate", "numbered rows are a rate of one a second");
    // a span: from and until are times, until is exclusive; both are clamped to the recording
    let t = rec.read_times(0, 4);
    let span = |from: Option<i64>, until: Option<i64>| {
        let dir = tmp.path().join(format!("s{}-{}", from.is_some(), until.is_some()));
        let r = html::export(&dir, &[file()], json!({}), &html::Options { from, until, ..Default::default() }).unwrap();
        r.files[0].samples
    };
    assert_eq!(span(Some(t[1]), Some(t[3])), 2);
    assert_eq!(span(Some(t[2]), None), 2);
    assert_eq!(span(None, Some(t[1])), 1);
    assert_eq!(span(Some(t[3] + 10 * S), None), 0, "from after the end: nothing");
    assert_eq!(span(Some(t[3]), Some(t[0])), 0, "until before from: nothing");
}

#[test]
fn provenance_hashes_the_inputs_and_reports_the_one_it_cannot_read() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.bin");
    fs::write(&a, vec![7u8; 3_000_000]).unwrap();
    let cfg = json!({"b": 2, "a": 1});
    let p = html::provenance(&["tlmbench".into(), "export".into()], &cfg, &[&a]).unwrap();
    assert_eq!(p["inputs"][0]["bytes"], 3_000_000);
    assert_eq!(p["inputs"][0]["blake3"], blake3::hash(&vec![7u8; 3_000_000]).to_hex().to_string());
    assert_eq!(p["command"], json!(["tlmbench", "export"]));
    // the same configuration, written another way round, hashes the same
    let q = html::provenance(&[], &json!({"a": 1, "b": 2}), &[&a]).unwrap();
    assert_eq!(p["config_hash"], q["config_hash"]);
    let e = html::provenance(&[], &cfg, &[&tmp.path().join("missing.bin")]).unwrap_err();
    assert!(e.to_string().contains("missing.bin"));
}

#[test]
fn svg_with_broken_or_unusual_markup_is_cut_where_it_is_unsafe_and_kept_where_it_is_plain() {
    let same = |s: &str| assert_eq!(sanitize_svg(s), s, "{s}");
    let is = |s: &str, want: &str| assert_eq!(sanitize_svg(s), want, "{s}");
    // plain things come out as they went in, including a processing instruction and newlines in a tag
    same("<svg><g/></svg>");
    same("<?xml version=\"1.0\"?><svg/>");
    is("<svg\n viewBox=\"0 0 1 1\"><g/></svg>", "<svg viewBox=\"0 0 1 1\"><g/></svg>");
    same("text before <svg/> and after");
    // a tag that never closes, a comment that never closes: the rest is cut
    is("<svg><rect fill=\"a>", "<svg>");
    is("<svg><!-- never closed", "<svg>");
    is("<svg><!DOCTYPE x><g/>", "<svg><g/>");
    // dropped elements: with content, without a close, self-closing, stray closers, any case, with a prefix
    is("<svg><script>alert(1)", "<svg>");
    is("<svg><script src=\"x\"/><g/></svg>", "<svg><g/></svg>");
    is("<svg></script></svg>", "<svg></svg>");
    is("<svg><SCRIPT>x</SCRIPT><g/></svg>", "<svg><g/></svg>");
    is("<svg><svg:script>x</svg:script><g/></svg>", "<svg><g/></svg>");
    is("<svg><style>a{}</style><g/></svg>", "<svg><g/></svg>");
    // attributes: unquoted, single-quoted, a quote inside, none at all
    is("<rect width=5 height='3' hidden/>", "<rect width=\"5\" height=\"3\" hidden/>");
    is("<g title='say \"hi\"'/>", "<g title=\"say &quot;hi&quot;\"/>");
    is("<g data-x = \"1\" />", "<g data-x=\"1\"/>");
    // references in a value: known ones are decoded to find a scheme, unknown ones and long ones are left alone
    is("<g fill=\"&#106;avascript:x\"/>", "<g/>");
    is("<g fill=\"java&Tab;script:x\"/>", "<g/>");
    is("<g fill=\"javascript&colon;x\"/>", "<g/>");
    same("<g fill=\"javascript&lpar;x\" stroke=\"red\"/>"); // no colon: not a script URL
    same("<g title=\"a &amp; b &bogus; &#xZZ; &#1114112; &aaaaaaaaaaaaaaaaaaaa; &\"/>");
    // event handlers and outside links go, whatever the case; fragment links stay
    is("<a ONCLICK=\"x()\" HREF=\"http://e.com\" xlink:href=\"#ok\"/>", "<a xlink:href=\"#ok\"/>");
    is("<rect style=\"fill:url('#g')\"/>", "<rect style=\"fill:url('#g')\"/>");
    is("<rect style=\"fill:url('http://e.com/x')\"/>", "<rect/>");
    is("<rect style=\"@import 'x'\"/>", "<rect/>");
    // animations: those that write a link, handler or style go; the rest stay
    same("<set to=\"1\"/>");
    same("<animate attributeName=\"opacity\" values=\"0;1\"/>");
    same("<animate attributeName=\"x:y\"/>");
    is("<g><animate attributeName=href to=\"x\"/></g>", "<g></g>");
    is("<g><animate attributeName='style' to=\"x\"/></g>", "<g></g>");
    is("<g><animate attributeName=\"xlink:href\" to=\"x\"/></g>", "<g></g>");
    is("<g><animate attributeName=\"onbegin\">x</animate><rect/></g>", "<g><rect/></g>");
    same("<g><animate attributeName=\"opacity\">x</animate></g>");
    is("<g><animate attributeName=\"href\">x", "<g>");
    // a closing tag of a kept element is kept
    same("<g></g>");
}

#[test]
fn widgets_with_both_a_drawing_and_code_travel_as_cards_and_names_default_to_ids() {
    let out = export_widgets(&[
        json!({"id": "a", "svg": "<svg/>", "code": "x", "icon": "<svg onload=\"x()\"/>"}),
        json!({"id": "b", "name": "B", "svg": "<svg/>", "by": "kim"}),
        json!({"id": 7, "name": "not a string id"}),
        json!({"id": "c<>&\"", "name": "c<>&\""}),
    ]);
    assert_eq!(out.len(), 3);
    assert_eq!((out[0]["name"].as_str(), out[0]["card"].as_bool()), (Some("a"), Some(true)));
    assert_eq!(out[0]["icon"], "<svg/>", "an icon is sanitised too");
    assert_eq!(out[1]["by"], "kim");
    assert!(out[1]["icon"].is_null());
    assert!(!out[2]["svg"].as_str().unwrap().contains("c<>"), "a name can't break out of the card");
}
