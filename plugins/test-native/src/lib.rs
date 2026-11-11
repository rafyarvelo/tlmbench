//! A native decoder for tests: files that start with `NAT1` followed by little-endian f64 values, one channel
//! "v" with the row number as its time in milliseconds. Written against `tlmbench_plugin.h`.
#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;

#[repr(C)]
pub struct Str {
    ptr: *const u8,
    len: usize,
}
const fn s(t: &'static str) -> Str {
    Str { ptr: t.as_ptr(), len: t.len() }
}
unsafe impl Sync for Str {}

#[repr(C)]
pub struct About {
    name: Str,
    version: Str,
    description: Str,
    files: Str,
}
#[repr(C)]
pub struct Channel {
    name: Str,
    unit: Str,
    dtype: u8,
}
unsafe impl Sync for Channel {}
#[repr(C)]
pub struct Schema {
    channels: *const Channel,
    n_channels: usize,
    timed: u8,
}
#[repr(C)]
pub struct Chunk {
    rows: usize,
    time: *const i64,
    columns: *const *const c_void,
}

static CHANNELS: [Channel; 1] = [Channel { name: s("v"), unit: s("m"), dtype: 9 }];

struct Session {
    values: Vec<f64>,
    next: usize,
    time: Vec<i64>,
    col: *const c_void,
    cols: [*const c_void; 1],
}

#[unsafe(no_mangle)]
pub extern "C" fn tlm_abi() -> u32 {
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tlm_describe(out: *mut About) {
    unsafe {
        *out = About { name: s("native-test"), version: s("0.2.0"), description: s("A native test decoder"), files: s("*.nat") };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tlm_sniff(head: *const u8, len: usize, _name: Str) -> i32 {
    (unsafe { std::slice::from_raw_parts(head, len) }.starts_with(b"NAT1")) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tlm_open(path: Str, schema: *mut Schema, error: *mut Str) -> *mut c_void {
    let path = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(path.ptr, path.len) }).into_owned();
    match std::fs::read(&path) {
        Ok(b) if b.starts_with(b"NAT1") => {
            let values = b[4..].chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
            unsafe { *schema = Schema { channels: CHANNELS.as_ptr(), n_channels: 1, timed: 1 } };
            Box::into_raw(Box::new(Session { values, next: 0, time: Vec::new(), col: std::ptr::null(), cols: [std::ptr::null()] }))
                as *mut c_void
        }
        _ => {
            unsafe { *error = s("not a NAT1 file") };
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tlm_next(session: *mut c_void, max_rows: u32, out: *mut Chunk, _error: *mut Str) -> i32 {
    let st = unsafe { &mut *(session as *mut Session) };
    let n = (st.values.len() - st.next).min(max_rows as usize).min(3);
    if n == 0 {
        return 0;
    }
    st.time = (st.next..st.next + n).map(|i| i as i64 * 1_000_000).collect();
    st.col = st.values[st.next..].as_ptr() as *const c_void;
    st.cols = [st.col];
    st.next += n;
    unsafe { *out = Chunk { rows: n, time: st.time.as_ptr(), columns: st.cols.as_ptr() } };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tlm_close(session: *mut c_void) {
    drop(unsafe { Box::from_raw(session as *mut Session) });
}
