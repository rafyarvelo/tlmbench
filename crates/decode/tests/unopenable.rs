//! A file that can't be opened is an error that names the file (or what is wrong with it), whichever way it is
//! opened: a descriptor naming a file that isn't there, a reference file or a Parquet file that is damaged, a cache
//! folder that can't be made (coverage, R6c.3).

use std::path::Path;

use tlmbench_decode::descriptor::Descriptor;
use tlmbench_decode::reference;
use tlmbench_decode::{OpenOptions, open_all};

const FIXED: &str = "schema = 1\nrecord_size = 4\nfields = [{ name = \"a\", type = \"f32\", offset = 0 }]\n";
const PACKETS: &str = "schema = 1\nrecord_size = 4\nfields = [{ name = \"a\", type = \"f32\", offset = 0 }]\n\n\
                       [packets]\nsync = \"EB90\"\nheader_size = 4\nlength = { offset = 2, type = \"u16\" }\n";

fn err(file: &Path, opts: &OpenOptions) -> String {
    match open_all(file, opts) {
        Ok(_) => panic!("{} opened", file.display()),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_file_that_cant_be_opened_is_an_error_not_a_recording() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let opts = OpenOptions { cache_dir: Some(d.join("cache")), ..Default::default() };

    // a descriptor for a file that isn't there, laid out as fixed-width records and as packets
    for (name, toml) in [("fixed", FIXED), ("packets", PACKETS)] {
        let desc = d.join(format!("{name}.toml"));
        std::fs::write(&desc, toml).unwrap();
        let missing = d.join(format!("missing-{name}.bin"));
        let e = err(&missing, &OpenOptions { descriptor: Some(desc), ..opts.clone() });
        assert!(e.contains(&format!("missing-{name}.bin")), "{name}: {e}");
    }

    // a reference file whose embedded descriptor isn't a descriptor
    let mut header = Vec::new();
    reference::write_header(&mut header, &Descriptor::parse(FIXED).unwrap()).unwrap();
    header[24] = b'?';
    let file = d.join("bad.tlmref");
    std::fs::write(&file, &header).unwrap();
    assert!(err(&file, &opts).contains("bad.tlmref"));

    // a Parquet file that is only its magic and noise
    let file = d.join("noise.parquet");
    std::fs::write(&file, [b"PAR1".as_slice(), &[0x5a; 300], b"PAR1"].concat()).unwrap();
    let e = err(&file, &opts);
    assert!(e.contains("noise.parquet") && e.contains("Parquet"), "{e}");

    // a CSV file whose cache folder can't be made: the cache's place is a file
    let file = d.join("a.csv");
    std::fs::write(&file, "t,v\n0,1\n1,2\n").unwrap();
    let blocked = d.join("blocked");
    std::fs::write(&blocked, "a file, not a folder").unwrap();
    let e = err(&file, &OpenOptions { cache_dir: Some(blocked.clone()), ..Default::default() });
    assert!(e.contains("blocked"), "{e}");
}
