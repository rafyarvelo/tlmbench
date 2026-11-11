//! WASM plugins on wasmtime: the sandbox and the two worlds (see `wit/tlmbench.wit`).
//!
//! The sandbox: a plugin is a component with no host access beyond what is handed to it. Its WASI context has no
//! preopened folders, no network, no environment or arguments and fixed clocks, so a decoder reads the file only
//! through `source.read` (a read-only handle the host opened) and an estimator only sees the series it asked for.
//! Each call has a CPU budget (an epoch deadline, ticked by one background thread) and each instance a memory cap, so
//! a loop or a huge allocation ends in an error and not in a hung app.

use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, ResourceLimiter, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// What one call may use. Decoders get a generous budget per chunk, estimators the design's 2 s and 256 MB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub cpu: Duration,
    pub memory: usize,
}

impl Limits {
    pub const DECODER: Limits = Limits { cpu: Duration::from_secs(30), memory: 1 << 30 };
    pub const ESTIMATOR: Limits = Limits { cpu: Duration::from_secs(2), memory: 256 << 20 };
}

/// One tick of the epoch clock: the resolution of a CPU limit.
const TICK: Duration = Duration::from_millis(25);

pub(crate) fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut c = Config::new();
        c.epoch_interruption(true);
        let engine = Engine::new(&c).expect("wasmtime starts");
        let weak = engine.weak();
        std::thread::Builder::new()
            .name("plugin-epoch".into())
            .spawn(move || {
                while let Some(e) = weak.upgrade() {
                    e.increment_epoch();
                    drop(e);
                    std::thread::sleep(TICK);
                }
            })
            .expect("the epoch thread starts");
        engine
    })
}

/// A plugin's instance state: its sandboxed WASI context and what the host lends it.
pub(crate) struct Host {
    wasi: WasiCtx,
    table: ResourceTable,
    max_memory: usize,
    /// The file a decoder reads: the only thing it can reach.
    pub(crate) file: Option<(File, u64)>,
}

impl WasiView for Host {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl ResourceLimiter for Host {
    fn memory_growing(&mut self, _current: usize, desired: usize, _max: Option<usize>) -> wasmtime::Result<bool> {
        Ok(desired <= self.max_memory)
    }
    fn table_growing(&mut self, _current: usize, desired: usize, _max: Option<usize>) -> wasmtime::Result<bool> {
        Ok(desired <= 10_000_000)
    }
}

struct FixedWall;
impl wasmtime_wasi::HostWallClock for FixedWall {
    fn resolution(&self) -> Duration {
        Duration::from_secs(1)
    }
    fn now(&self) -> Duration {
        Duration::ZERO
    }
}
struct FixedMono;
impl wasmtime_wasi::HostMonotonicClock for FixedMono {
    fn resolution(&self) -> u64 {
        1_000_000_000
    }
    fn now(&self) -> u64 {
        0
    }
}

fn host(limits: Limits, file: Option<File>) -> Host {
    // Nothing is inherited or preopened; the clocks stand still and the "insecure" random numbers are seeded, so a
    // plugin's result depends on its inputs alone.
    let wasi = WasiCtxBuilder::new().wall_clock(FixedWall).monotonic_clock(FixedMono).insecure_random_seed(0).build();
    let file = file.and_then(|f| f.metadata().ok().map(|m| (f, m.len())));
    Host { wasi, table: ResourceTable::new(), max_memory: limits.memory, file }
}

pub(crate) fn linker() -> wasmtime::Result<Linker<Host>> {
    let mut l = Linker::new(engine());
    wasmtime_wasi::p2::add_to_linker_sync(&mut l)?;
    Ok(l)
}

/// A new store with the CPU budget set; call [`arm`] before each call into the plugin.
pub(crate) fn store(limits: Limits, file: Option<File>) -> Store<Host> {
    let mut s = Store::new(engine(), host(limits, file));
    s.limiter(|h| h);
    s.epoch_deadline_trap();
    s
}

/// Starts the CPU budget of the next call.
pub(crate) fn arm(s: &mut Store<Host>, limits: Limits) {
    s.set_epoch_deadline((limits.cpu.as_millis() / TICK.as_millis()).max(1) as u64);
}

/// A trap's message for a person: the budget and memory cap are named, a panic in the plugin is not a crash.
pub(crate) fn describe_error(e: wasmtime::Error, what: &str) -> String {
    let text = format!("{e:#}");
    if text.contains("epoch deadline") || text.contains("interrupt") {
        format!("{what} ran past its time limit")
    } else if text.contains("memory") && text.contains("grow") {
        format!("{what} needed more memory than it may use")
    } else {
        format!("{what} failed: {}", text.lines().next().unwrap_or("unknown error"))
    }
}

/// Reads up to `len` bytes at `offset`.
pub(crate) fn read_at(file: &File, offset: u64, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let mut got = 0;
    while got < len {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(file, &mut buf[got..], offset + got as u64);
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(file, &mut buf[got..], offset + got as u64);
        match n {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    buf.truncate(got);
    buf
}

pub(crate) fn load(path: &Path) -> Result<Component, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Component::new(engine(), bytes).map_err(|e| {
        format!("{}: not a plugin component ({})", path.display(), format!("{e:#}").lines().next().unwrap_or(""))
    })
}

/// Which world a component implements, from its exports.
pub(crate) fn kind_of(c: &Component) -> Option<crate::Kind> {
    let ty = c.component_type();
    let names: Vec<String> = ty.exports(engine()).map(|(n, _)| n.to_string()).collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    if has("sniff") && has("next") {
        Some(crate::Kind::Decoder)
    } else if has("estimate") {
        Some(crate::Kind::Estimator)
    } else {
        None
    }
}
