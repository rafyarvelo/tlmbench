//! Native decoder plugins: shared libraries with the C interface in `include/tlmbench_plugin.h`.
//!
//! These are trusted code: they run in the app's process with the user's rights, so they are loaded only from the
//! user's own plugins folder and only when the person has turned that plugin on (see `install`). A plugin takes the
//! file's path and reads the file itself, which is where "native speed" comes from.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::{Library, Symbol};
use tlmbench_decode::descriptor::DType;
use tlmbench_decode::plugin::{Chunk, DecoderPlugin, DecoderSession, PluginChannel, PluginInfo, PluginSchema, Tier};

pub const ABI: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct Str {
    ptr: *const u8,
    len: usize,
}

impl Str {
    fn of(s: &str) -> Str {
        Str { ptr: s.as_ptr(), len: s.len() }
    }
    const NONE: Str = Str { ptr: std::ptr::null(), len: 0 };
    /// # Safety
    /// `ptr` and `len` describe memory that is valid for the length of the borrow.
    unsafe fn text(&self) -> String {
        if self.ptr.is_null() {
            return String::new();
        }
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(self.ptr, self.len) }).into_owned()
    }
}

#[repr(C)]
struct About {
    name: Str,
    version: Str,
    description: Str,
    files: Str,
}

#[repr(C)]
struct CChannel {
    name: Str,
    unit: Str,
    dtype: u8,
}

#[repr(C)]
struct CSchema {
    channels: *const CChannel,
    n_channels: usize,
    timed: u8,
}

#[repr(C)]
struct CChunk {
    rows: usize,
    time: *const i64,
    columns: *const *const c_void,
}

type AbiFn = unsafe extern "C" fn() -> u32;
type DescribeFn = unsafe extern "C" fn(*mut About);
type SniffFn = unsafe extern "C" fn(*const u8, usize, Str) -> i32;
type OpenFn = unsafe extern "C" fn(Str, *mut CSchema, *mut Str) -> *mut c_void;
type NextFn = unsafe extern "C" fn(*mut c_void, u32, *mut CChunk, *mut Str) -> i32;
type CloseFn = unsafe extern "C" fn(*mut c_void);

pub struct NativeDecoder {
    lib: Arc<Library>,
    info: PluginInfo,
}

impl std::fmt::Debug for NativeDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NativeDecoder({})", self.info.name)
    }
}

fn dtype(code: u8) -> Option<DType> {
    Some(match code {
        0 => DType::I8,
        1 => DType::I16,
        2 => DType::I32,
        3 => DType::I64,
        4 => DType::U8,
        5 => DType::U16,
        6 => DType::U32,
        7 => DType::U64,
        8 => DType::F32,
        9 => DType::F64,
        _ => return None,
    })
}

impl NativeDecoder {
    /// Loads the library and reads what it says about itself. Running its code starts here, so only call this for a
    /// plugin the person has turned on.
    pub fn load(path: &Path) -> Result<Self, String> {
        // SAFETY: loading a library runs its initialisers; the caller has decided to trust this one.
        let lib = unsafe { Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;
        let abi: Symbol<AbiFn> = unsafe { lib.get(b"tlm_abi\0") }
            .map_err(|_| format!("{}: not a tlmbench plugin (no tlm_abi)", path.display()))?;
        let v = unsafe { abi() };
        if v != ABI {
            return Err(format!("{}: built for plugin interface {v}, this app has {ABI}", path.display()));
        }
        let describe: Symbol<DescribeFn> =
            unsafe { lib.get(b"tlm_describe\0") }.map_err(|e| format!("{}: {e}", path.display()))?;
        let mut a = About { name: Str::NONE, version: Str::NONE, description: Str::NONE, files: Str::NONE };
        // SAFETY: the plugin fills `a` with strings it keeps alive.
        unsafe { describe(&mut a) };
        let (name, version, description, files) =
            unsafe { (a.name.text(), a.version.text(), a.description.text(), a.files.text()) };
        for sym in [&b"tlm_sniff\0"[..], b"tlm_open\0", b"tlm_next\0", b"tlm_close\0"] {
            unsafe { lib.get::<unsafe extern "C" fn()>(sym) }.map_err(|_| {
                format!("{}: missing {}", path.display(), String::from_utf8_lossy(&sym[..sym.len() - 1]))
            })?;
        }
        let info = PluginInfo {
            name: if name.is_empty() { stem(path) } else { name },
            version: if version.is_empty() { "0".into() } else { version },
            tier: Tier::Native,
            description: Some(description).filter(|d| !d.is_empty()),
            files: files.split(';').map(str::trim).filter(|f| !f.is_empty()).map(str::to_owned).collect(),
            path: PathBuf::from(path),
        };
        Ok(Self { lib: Arc::new(lib), info })
    }
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().trim_start_matches("lib").to_string()).unwrap_or_default()
}

impl DecoderPlugin for NativeDecoder {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn sniff(&self, head: &[u8], name: &str) -> bool {
        let Ok(f) = (unsafe { self.lib.get::<SniffFn>(b"tlm_sniff\0") }) else { return false };
        // SAFETY: the pointers are valid for the call.
        unsafe { f(head.as_ptr(), head.len(), Str::of(name)) != 0 }
    }

    fn open(&self, path: &Path) -> Result<Box<dyn DecoderSession>, String> {
        let text = path.to_string_lossy();
        let open: Symbol<OpenFn> = unsafe { self.lib.get(b"tlm_open\0") }.map_err(|e| e.to_string())?;
        let mut schema = CSchema { channels: std::ptr::null(), n_channels: 0, timed: 0 };
        let mut err = Str::NONE;
        // SAFETY: `text` outlives the call; the plugin fills `schema` and `err`.
        let handle = unsafe { open(Str::of(&text), &mut schema, &mut err) };
        if handle.is_null() {
            let why = unsafe { err.text() };
            return Err(if why.is_empty() { "the plugin could not open the file".into() } else { why });
        }
        let channels = (0..schema.n_channels)
            .map(|k| {
                // SAFETY: the plugin promised `n_channels` valid entries until `tlm_close`.
                let c = unsafe { &*schema.channels.add(k) };
                let dt = dtype(c.dtype).ok_or_else(|| format!("channel {k} has the unknown type {}", c.dtype))?;
                Ok(PluginChannel {
                    name: unsafe { c.name.text() },
                    unit: Some(unsafe { c.unit.text() }).filter(|u| !u.is_empty()),
                    dtype: dt,
                })
            })
            .collect::<Result<Vec<_>, String>>();
        let session = Session {
            lib: self.lib.clone(),
            handle,
            schema: PluginSchema { channels: channels.as_ref().cloned().unwrap_or_default(), timed: schema.timed != 0 },
        };
        // a bad schema drops the session here, which closes it
        channels?;
        Ok(Box::new(session))
    }
}

struct Session {
    lib: Arc<Library>,
    handle: *mut c_void,
    schema: PluginSchema,
}

// SAFETY: the interface says a session is used by one thread at a time, and it is only ever used by the thread
// that builds a cache entry.
unsafe impl Send for Session {}

impl DecoderSession for Session {
    fn schema(&self) -> &PluginSchema {
        &self.schema
    }

    fn next(&mut self, max_rows: u32) -> Result<Option<Chunk>, String> {
        let next: Symbol<NextFn> = unsafe { self.lib.get(b"tlm_next\0") }.map_err(|e| e.to_string())?;
        let mut c = CChunk { rows: 0, time: std::ptr::null(), columns: std::ptr::null() };
        let mut err = Str::NONE;
        // SAFETY: the handle is ours until `drop`; the plugin fills `c` with buffers valid until the next call.
        let r = unsafe { next(self.handle, max_rows, &mut c, &mut err) };
        match r {
            0 => Ok(None),
            r if r < 0 => {
                let why = unsafe { err.text() };
                Err(if why.is_empty() { "the plugin failed".into() } else { why })
            }
            _ => {
                let rows = c.rows;
                let time = if self.schema.timed && !c.time.is_null() {
                    unsafe { std::slice::from_raw_parts(c.time, rows) }.to_vec()
                } else {
                    Vec::new()
                };
                let columns = self
                    .schema
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(k, ch)| {
                        let p = unsafe { *c.columns.add(k) } as *const u8;
                        unsafe { std::slice::from_raw_parts(p, rows * ch.dtype.size() as usize) }.to_vec()
                    })
                    .collect();
                Ok(Some(Chunk { rows, time, columns }))
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(close) = unsafe { self.lib.get::<CloseFn>(b"tlm_close\0") } {
            // SAFETY: the handle came from `tlm_open` and is closed once.
            unsafe { close(self.handle) };
        }
    }
}
