//! The viewer's core: `tlmbench-core` compiled to WASM, with the data an export holds (or a file opened in the page)
//! in memory. One entry point, [`call`]: a command name, JSON arguments and an optional binary payload in; a one-byte
//! kind (0 JSON, 1 binary, 2 an error message) and the reply out. The commands answer as the app's Tauri commands do
//! (`view`, `values_at`, `stats`, `rows`), so the UI's code runs unchanged over it.
//!
//! No wasm-bindgen: the ABI is `alloc`, `call` and `reply_len`, and `js/core.ts` is the whole glue.

mod data;
mod ops;

use std::cell::RefCell;

use serde_json::Value;

thread_local! {
    static STATE: RefCell<data::State> = RefCell::new(data::State::default());
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Memory for the page to write arguments into.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `p` and `n` are an earlier `alloc(n)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(p: *mut u8, n: usize) {
    drop(unsafe { Vec::from_raw_parts(p, 0, n.max(1)) });
}

/// The length of the last reply.
#[unsafe(no_mangle)]
pub extern "C" fn reply_len() -> usize {
    REPLY.with(|r| r.borrow().len())
}

/// Runs a command. Returns a pointer to the reply (`reply_len()` bytes, valid until the next call).
///
/// # Safety
/// Each pointer is an `alloc`ed buffer of the given length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call(cmd: *const u8, cmd_len: usize, args: *const u8, args_len: usize, bin: *const u8, bin_len: usize) -> *const u8 {
    let (cmd, args, bin) = unsafe {
        (
            std::slice::from_raw_parts(cmd, cmd_len),
            std::slice::from_raw_parts(args, args_len),
            std::slice::from_raw_parts(bin, bin_len),
        )
    };
    let out = match run(cmd, args, bin) {
        Ok(Reply::Json(v)) => [vec![0u8], serde_json::to_vec(&v).unwrap_or_default()].concat(),
        Ok(Reply::Bytes(b)) => [vec![1u8], b].concat(),
        Err(e) => [vec![2u8], e.into_bytes()].concat(),
    };
    REPLY.with(|r| {
        *r.borrow_mut() = out;
        r.borrow().as_ptr()
    })
}

pub enum Reply {
    Json(Value),
    Bytes(Vec<u8>),
}

fn run(cmd: &[u8], args: &[u8], bin: &[u8]) -> Result<Reply, String> {
    let cmd = std::str::from_utf8(cmd).map_err(|e| e.to_string())?;
    let args: Value = if args.is_empty() { Value::Null } else { serde_json::from_slice(args).map_err(|e| format!("{cmd}: {e}"))? };
    STATE.with(|s| ops::dispatch(&mut s.borrow_mut(), cmd, &args, bin))
}

/// The same entry for tests and native callers.
pub fn call_native(cmd: &str, args: &Value, bin: &[u8]) -> Result<Reply, String> {
    STATE.with(|s| ops::dispatch(&mut s.borrow_mut(), cmd, args, bin))
}
