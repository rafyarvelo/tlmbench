"""Drops files' pages from the OS file cache without root, so the next read comes from disk: evict.py <path>...

Folders are walked. macOS: msync(MS_INVALIDATE) on a shared map of each file. Linux: posix_fadvise(DONTNEED).
`just gate2` uses it on the recording and its cache entry before timing a reopen.
"""

import ctypes
import mmap
import os
import sys


def files(paths):
    for p in paths:
        if os.path.isdir(p):
            for root, _, names in os.walk(p):
                yield from (os.path.join(root, n) for n in names)
        else:
            yield p


def evict(path, libc):
    fd = os.open(path, os.O_RDONLY)
    try:
        n = os.fstat(fd).st_size
        if n == 0:
            return
        if hasattr(os, "posix_fadvise"):
            os.posix_fadvise(fd, 0, n, os.POSIX_FADV_DONTNEED)
            return
        addr = libc.mmap(None, n, mmap.PROT_READ, mmap.MAP_SHARED, fd, 0)
        if addr in (None, ctypes.c_void_p(-1).value):
            raise OSError(ctypes.get_errno(), f"mmap {path}")
        try:
            if libc.msync(addr, n, 0x2) != 0:  # MS_INVALIDATE
                raise OSError(ctypes.get_errno(), f"msync {path}")
        finally:
            libc.munmap(addr, n)
    finally:
        os.close(fd)


def main():
    libc = ctypes.CDLL(None, use_errno=True)
    libc.mmap.restype = ctypes.c_void_p
    libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_longlong]
    libc.msync.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int]
    libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    for f in files(sys.argv[1:]):
        evict(f, libc)


if __name__ == "__main__":
    main()
