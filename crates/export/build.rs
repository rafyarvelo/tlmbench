//! Embeds the viewer (`app/viewer/viewer.pack`, made by `just viewer`) so `tlmbench export html` and the app's Export need
//! no files beside them, and the one-file viewer (`single.pack`). Without them (a plain `cargo build`), the export writes
//! data only and says so.

use std::path::Path;

fn main() {
    for (name, out_name) in [("viewer.pack", "viewer.pack"), ("single.pack", "single.pack")] {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/viewer").join(name);
        println!("cargo:rerun-if-changed={}", pack.display());
        let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join(out_name);
        let bytes = std::fs::read(&pack).unwrap_or_default();
        std::fs::write(out, bytes).unwrap();
    }
}
