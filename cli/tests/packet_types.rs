//! A packet file with several packet types: one recording per type in `info` and `convert`, `--type` picks one.

use std::path::Path;
use std::process::{Command, Output};

use tlmbench_decode::descriptor::Descriptor;
use tlmbench_decode::packets::encode_packet;

const DESC: &str = r#"
    schema = 1
    name = "flight"
    [packets]
    sync = "EB90"
    header_size = 8
    length = { offset = 2, type = "u16" }
    id = { offset = 4, type = "u8" }
    [[packet_types]]
    id = 1
    name = "gps"
    record_size = 12
    time = { source = "field", field = "t", unit = "ms" }
    fields = [ { name = "t", type = "u64", offset = 8 }, { name = "lat", type = "f32", offset = 16 } ]
    [[packet_types]]
    id = 2
    name = "imu"
    record_size = 4
    time = { source = "rate", rate_hz = 100.0 }
    fields = [ { name = "ax", type = "i16", offset = 8 }, { name = "ay", type = "i16", offset = 10 } ]
"#;

fn setup(dir: &Path) {
    let d = Descriptor::parse(DESC).unwrap();
    let p = d.packets.as_ref().unwrap();
    let mut data = Vec::new();
    for i in 0..30u64 {
        let mut header = vec![0u8; 8];
        header[4] = 1;
        let mut gps = (i * 100).to_le_bytes().to_vec();
        gps.extend((i as f32).to_le_bytes());
        data.extend(encode_packet(p, false, &header, &gps).unwrap());
        header[4] = 2;
        data.extend(encode_packet(p, false, &header, &[i as u8, 0, 0, 0]).unwrap());
    }
    std::fs::write(dir.join("flight.bin"), data).unwrap();
    std::fs::write(dir.join("flight.toml"), DESC).unwrap();
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench"))
        .args(args)
        .args([
            "--cache",
            dir.join("cache").to_str().unwrap(),
            "--descriptor",
            dir.join("flight.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

#[test]
fn info_and_convert_work_per_type() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    let bin = dir.join("flight.bin");
    let bin = bin.to_str().unwrap();

    let out = run(dir, &["info", "--json", bin]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: Vec<serde_json::Value> =
        String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!((v[0]["part"].as_str(), v[0]["samples"].as_u64()), (Some("gps"), Some(30)));
    assert_eq!((v[1]["part"].as_str(), v[1]["time"]["kind"].as_str()), (Some("imu"), Some("rate")));

    let text = String::from_utf8_lossy(&run(dir, &["info", "--type", "imu", bin]).stdout).into_owned();
    assert!(text.starts_with(&format!("{bin} · imu")), "{text}");

    // a command that reads one recording asks which
    let out = run(dir, &["query", "stats", bin]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("2 packet types (gps, imu)"));
    assert!(run(dir, &["query", "stats", "--type", "gps", bin]).status.success());

    let out = run(dir, &["convert", "--to", "csv", "--out", dir.join("out").to_str().unwrap(), bin]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut names: Vec<_> = std::fs::read_dir(dir.join("out"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["flight.gps.channels.csv", "flight.gps.csv", "flight.imu.channels.csv", "flight.imu.csv"]);
}
