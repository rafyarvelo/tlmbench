//! What the OS says about the machine: free disk space (for the cache cap) and physical memory (for the chunk
//! cache budget).

use std::path::Path;

/// Bytes free for this user on the volume holding `path`, if the OS says.
pub fn free_disk(path: &Path) -> Option<u64> {
    imp::free_disk(path)
}

/// The share (0 to 1) of up to 64 pages spread over `bytes` (part of a memory map) that are in memory; None where
/// the OS can't say.
pub fn resident_share(bytes: &[u8]) -> Option<f64> {
    imp::resident_share(bytes)
}

/// Physical memory in bytes, if the OS says.
pub fn total_ram() -> Option<u64> {
    imp::total_ram()
}

#[cfg(unix)]
mod imp {
    use std::ffi::CString;

    pub fn resident_share(bytes: &[u8]) -> Option<f64> {
        // SAFETY: sysconf has no preconditions.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as usize;
        let base = bytes.as_ptr() as usize / page * page;
        let pages = (bytes.as_ptr() as usize + bytes.len()).div_ceil(page).saturating_sub(base / page);
        if pages == 0 {
            return Some(1.0);
        }
        let (picks, mut resident) = (pages.min(64), 0);
        for k in 0..picks {
            let addr = base + k * pages / picks * page;
            let mut v = 0 as libc::c_char;
            // SAFETY: `addr` is a page inside the mapping `bytes` belongs to; mincore writes one byte for one page.
            if unsafe { libc::mincore(addr as *mut libc::c_void, page, (&raw mut v).cast()) } != 0 {
                return None;
            }
            resident += (v & 1) as usize;
        }
        Some(resident as f64 / picks as f64)
    }
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    pub fn free_disk(path: &Path) -> Option<u64> {
        let c = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut st = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
        // SAFETY: `c` is a valid C string and `st` is writable for one statvfs.
        let ok = unsafe { libc::statvfs(c.as_ptr(), st.as_mut_ptr()) } == 0;
        // SAFETY: statvfs succeeded, so it filled `st`.
        #[allow(clippy::unnecessary_cast)] // the field types differ between platforms
        ok.then(|| unsafe { st.assume_init() }).map(|st| st.f_bavail as u64 * st.f_frsize as u64)
    }

    #[cfg(target_os = "macos")]
    pub fn total_ram() -> Option<u64> {
        let mut bytes = 0u64;
        let mut len = size_of::<u64>();
        let name = c"hw.memsize";
        // SAFETY: hw.memsize is a u64 and `bytes` holds one.
        let ok =
            unsafe { libc::sysctlbyname(name.as_ptr(), (&raw mut bytes).cast(), &mut len, std::ptr::null_mut(), 0) }
                == 0;
        ok.then_some(bytes)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn total_ram() -> Option<u64> {
        // SAFETY: sysconf has no preconditions.
        let (pages, size) = unsafe { (libc::sysconf(libc::_SC_PHYS_PAGES), libc::sysconf(libc::_SC_PAGESIZE)) };
        (pages > 0 && size > 0).then(|| pages as u64 * size as u64)
    }
}

#[cfg(windows)]
mod imp {
    pub fn resident_share(_: &[u8]) -> Option<f64> {
        None
    }

    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    pub fn free_disk(path: &Path) -> Option<u64> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut free = 0u64;
        // SAFETY: `wide` is NUL-terminated and `free` is a writable u64; the other outputs may be null.
        let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
        (ok != 0).then_some(free)
    }

    pub fn total_ram() -> Option<u64> {
        // SAFETY: MEMORYSTATUSEX is plain data; dwLength is set as the call requires.
        let mut st: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        st.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: `st` is valid and sized.
        let ok = unsafe { GlobalMemoryStatusEx(&mut st) };
        (ok != 0).then_some(st.ullTotalPhys)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_os_answers() {
        assert!(super::free_disk(&std::env::temp_dir()).is_some_and(|b| b > 0));
        assert!(super::total_ram().is_some_and(|b| b > 1 << 28));
    }
}
