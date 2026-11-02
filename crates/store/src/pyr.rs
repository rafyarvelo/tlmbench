//! Pyramid and gap files: their layout, loading, and the resumable build.

use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use tlmbench_core::Recording;
use tlmbench_core::gaps::{GapIndex, Jump};
use tlmbench_core::pyramid::{MIN_LEVEL, Pyramid, Summary, coarser, level_lengths, span};

use crate::{Error, FOLLOW_EVERY, Opened, Result};

/// The layout version: the folder under `pyr/`. A layout change bumps it, and old folders age out of the cache.
pub(crate) const VERSION: &str = "v1";
const FORMAT: u32 = 1;
/// A build works in blocks of 4^10 samples: levels 4 to 10 of a block are written as soon as it is read.
const BLOCK_LEVEL: u32 = 10;
const SUMMARY: usize = size_of::<Summary>();

/// The cache key: the source's fingerprint and how it is decoded.
pub(crate) fn key(fingerprint: &str, layout_id: &[u8]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(fingerprint.as_bytes());
    h.update(&[0]);
    h.update(layout_id);
    h.finalize().to_hex()[..32].to_owned()
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    fingerprint: String,
    samples: usize,
    channels: usize,
    /// Summaries per level, level 4 first.
    levels: Vec<usize>,
    median_ns: i64,
    jumps: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Progress {
    format: u32,
    fingerprint: String,
    samples: usize,
    channels: usize,
    blocks_done: usize,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// Writes JSON through a temporary file and a rename, so a reader never sees half of it.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value).expect("serializes")).map_err(Error::io(&tmp))?;
    fs::rename(&tmp, path).map_err(Error::io(path))
}

/// The fingerprint a pyramid folder (built or building) was made from.
pub(crate) fn fingerprint_of(dir: &Path) -> Option<String> {
    read_json::<Manifest>(&dir.join("manifest.json"))
        .map(|m| m.fingerprint)
        .or_else(|| read_json::<Progress>(&dir.join("progress.json")).map(|p| p.fingerprint))
}

/// A recording's pyramid files, mapped.
#[derive(Debug)]
pub(crate) struct Loaded {
    /// One per channel: every level back to back.
    maps: Vec<Mmap>,
    /// The files mapped.
    paths: Vec<PathBuf>,
    /// Summaries per level, and where each level starts (in summaries).
    lens: Vec<usize>,
    offsets: Vec<usize>,
    /// Levels usable now (all once complete; up to level 10 while building).
    usable: usize,
    /// Samples `0..done` are summarized.
    pub done: usize,
    pub complete: bool,
    pub gaps: GapIndex,
}

impl Loaded {
    /// Channel `channel`'s pyramid, if any of it is built.
    pub fn pyramid(&self, channel: usize) -> Option<Pyramid<'_>> {
        if self.done == 0 {
            return None;
        }
        let all: &[Summary] = bytemuck::cast_slice(&self.maps[channel]);
        let levels = (0..self.usable).map(|j| &all[self.offsets[j]..self.offsets[j] + self.lens[j]]).collect();
        Some(Pyramid::new(self.done, levels))
    }

    /// Reads channel `channel`'s level-4 summaries of samples `a..b` with large sequential reads. An exact view of
    /// much of a cold file touches nearly every page of level 4 (its pixel edges), and page faults one by one are
    /// several times slower than reading the range through.
    pub fn warm(&self, channel: usize, a: usize, b: usize) {
        let w = span(MIN_LEVEL);
        let (s0, s1) = (a / w, b.div_ceil(w).min(self.lens[0]));
        let Ok(f) = File::open(&self.paths[channel]) else { return };
        let mut buf = vec![0u8; 8 << 20];
        let (mut at, end) = (((self.offsets[0] + s0) * SUMMARY) as u64, ((self.offsets[0] + s1) * SUMMARY) as u64);
        while at < end {
            let n = buf.len().min((end - at) as usize);
            if read_at(&f, &mut buf[..n], at).is_err() {
                return;
            }
            at += n as u64;
        }
    }

    /// Whether most of channel `channel`'s level 4 for samples `a..b` is out of memory (None: can't tell).
    pub fn cold(&self, channel: usize, a: usize, b: usize) -> Option<bool> {
        let w = span(MIN_LEVEL);
        let (s0, s1) = (a / w, b.div_ceil(w).min(self.lens[0]));
        if s0 >= s1 {
            return None;
        }
        let bytes = &self.maps[channel][(self.offsets[0] + s0) * SUMMARY..(self.offsets[0] + s1) * SUMMARY];
        crate::sys::resident_share(bytes).map(|r| r < 0.5)
    }

    fn paths(dir: &Path, channels: usize) -> Vec<PathBuf> {
        (0..channels).map(|c| dir.join(format!("c{c}.pyr"))).collect()
    }

    fn map(dir: &Path, channels: usize, total: usize) -> Result<Vec<Mmap>> {
        (0..channels)
            .map(|c| {
                let p = dir.join(format!("c{c}.pyr"));
                let f = File::open(&p).map_err(Error::io(&p))?;
                let len = f.metadata().map_err(Error::io(&p))?.len();
                if len != (total * SUMMARY) as u64 {
                    return Err(Error::Damaged { path: p, why: format!("{len} bytes, expected {}", total * SUMMARY) });
                }
                // SAFETY: cache files are only written by a build holding the key's lock, in regions past what
                // readers use (`done`), before the manifest marks them complete.
                unsafe { Mmap::map(&f) }.map_err(Error::io(&p))
            })
            .collect()
    }

    /// A finished pyramid in `dir`, if there is one that matches `rec`.
    pub fn open_built(dir: &Path, rec: &dyn Recording) -> Result<Option<Loaded>> {
        let Some(m) = read_json::<Manifest>(&dir.join("manifest.json")) else { return Ok(None) };
        let info = rec.info();
        let lens = level_lengths(info.samples);
        if m.format != FORMAT || m.samples != info.samples || m.channels != info.channels.len() || m.levels != lens {
            return Err(Error::Damaged {
                path: dir.to_owned(),
                why: "the manifest doesn't match the recording".into(),
            });
        }
        let offsets = offsets(&lens);
        let maps = Self::map(dir, m.channels, offsets[lens.len()])?;
        let gp = dir.join("gaps.bin");
        let bytes = fs::read(&gp).map_err(Error::io(&gp))?;
        if bytes.len() != m.jumps * size_of::<Jump>() {
            return Err(Error::Damaged { path: gp, why: "wrong size".into() });
        }
        let jumps: Vec<Jump> =
            bytes.as_chunks::<{ size_of::<Jump>() }>().0.iter().map(|b| bytemuck::pod_read_unaligned(b)).collect();
        let usable = lens.len();
        Ok(Some(Loaded {
            maps,
            paths: Self::paths(dir, m.channels),
            lens,
            offsets,
            usable,
            done: m.samples,
            complete: true,
            gaps: GapIndex { median_ns: m.median_ns, jumps },
        }))
    }

    /// A build under way in `dir` (this process's or another's), at its last recorded progress.
    fn open_building(dir: &Path, rec: &dyn Recording, p: &Progress) -> Result<Loaded> {
        let lens = level_lengths(rec.info().samples);
        let offsets = offsets(&lens);
        let maps = Self::map(dir, p.channels, offsets[lens.len()])?;
        let usable = lens.len().min((BLOCK_LEVEL - MIN_LEVEL) as usize + 1);
        let done = (p.blocks_done * span(BLOCK_LEVEL)).min(p.samples);
        let paths = Self::paths(dir, p.channels);
        Ok(Loaded { maps, paths, lens, offsets, usable, done, complete: false, gaps: GapIndex::default() })
    }
}

/// Where each level starts, in summaries; the last entry is the total.
fn offsets(lens: &[usize]) -> Vec<usize> {
    let mut out = vec![0];
    for l in lens {
        out.push(out.last().unwrap() + l);
    }
    out
}

#[cfg(unix)]
fn read_at(f: &File, buf: &mut [u8], at: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(f, buf, at)
}

#[cfg(windows)]
fn read_at(f: &File, buf: &mut [u8], at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    let mut done = 0;
    while done < buf.len() {
        match f.seek_read(&mut buf[done..], at + done as u64) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn write_at(f: &File, buf: &[u8], at: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(f, buf, at)
}

#[cfg(windows)]
fn write_at(f: &File, mut buf: &[u8], mut at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match f.seek_write(buf, at) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => {
                buf = &buf[n..];
                at += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Builds `op`'s pyramid (see the crate docs). Returns false when cancelled before the end.
pub(crate) fn build(op: &Opened, progress: &mut dyn FnMut(u64, u64)) -> Result<bool> {
    let rec = op.rec.as_ref();
    let (n, channels) = (rec.info().samples, rec.info().channels.len());
    if n == 0 || channels == 0 {
        return Ok(true);
    }
    let final_dir = op.dir.join(&op.key);
    // built in place: complete once manifest.json is written, last (a folder of mapped files can't be renamed on
    // Windows, so there is no rename into place)
    let building = final_dir.clone();
    let lock_path = op.dir.join(format!("{}.lock", op.key));
    let lock =
        File::options().create(true).truncate(false).write(true).open(&lock_path).map_err(Error::io(&lock_path))?;
    let set = |l: Loaded| *op.loaded.write().unwrap() = Some(l);

    // Another process may be building it: follow its progress until it finishes or this one can take over.
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) => {
                if let Ok(Some(l)) = Loaded::open_built(&final_dir, rec) {
                    set(l);
                    return Ok(true);
                }
                if let Some(p) = read_json::<Progress>(&building.join("progress.json"))
                    && let Ok(l) = Loaded::open_building(&building, rec, &p)
                {
                    progress(l.done as u64, n as u64);
                    set(l);
                }
                if op.cancel.load(Ordering::Relaxed) {
                    return Ok(false);
                }
                std::thread::sleep(FOLLOW_EVERY);
            }
            Err(TryLockError::Error(e)) => return Err(Error::io(&lock_path)(e)),
        }
    }

    match Loaded::open_built(&final_dir, rec) {
        Ok(Some(l)) => {
            set(l);
            return Ok(true);
        }
        Ok(None) => {}
        Err(_) => fs::remove_dir_all(&final_dir).map_err(Error::io(&final_dir))?, // damaged: rebuild it
    }

    let lens = level_lengths(n);
    let offs = offsets(&lens);
    let block = span(BLOCK_LEVEL);
    let per_block = lens.len().min((BLOCK_LEVEL - MIN_LEVEL) as usize + 1);
    let progress_path = building.join("progress.json");
    let resumable = read_json::<Progress>(&progress_path)
        .filter(|p| p.format == FORMAT && p.fingerprint == op.fingerprint && p.samples == n && p.channels == channels);
    let start = match resumable {
        Some(p) if Loaded::open_building(&building, rec, &p).is_ok() => p.blocks_done,
        _ => {
            let _ = fs::remove_dir_all(&building);
            fs::create_dir_all(&building).map_err(Error::io(&building))?;
            for c in 0..channels {
                let p = building.join(format!("c{c}.pyr"));
                File::create(&p).and_then(|f| f.set_len((offs[lens.len()] * SUMMARY) as u64)).map_err(Error::io(&p))?;
            }
            0
        }
    };
    let record = |blocks_done: usize| {
        write_json(
            &progress_path,
            &Progress { format: FORMAT, fingerprint: op.fingerprint.clone(), samples: n, channels, blocks_done },
        )
    };
    record(start)?;
    let files = (0..channels)
        .map(|c| {
            let p = building.join(format!("c{c}.pyr"));
            File::options().write(true).open(&p).map_err(Error::io(&p))
        })
        .collect::<Result<Vec<_>>>()?;
    let write_level = |c: usize, j: usize, first: usize, level: &[Summary]| {
        let at = ((offs[j] + first) * SUMMARY) as u64;
        write_at(&files[c], bytemuck::cast_slice(level), at).map_err(Error::io(&building))
    };
    let p0 = Progress { format: FORMAT, fingerprint: op.fingerprint.clone(), samples: n, channels, blocks_done: start };
    set(Loaded::open_building(&building, rec, &p0)?);
    progress((start * block).min(n) as u64, n as u64);

    for b in start..n.div_ceil(block) {
        if op.cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let (s, e) = (b * block, ((b + 1) * block).min(n));
        for (c, mut level) in rec.summarize_all(s, e).into_iter().enumerate() {
            for j in 0..per_block {
                if j > 0 {
                    level = coarser(&level);
                }
                write_level(c, j, s >> (2 * (MIN_LEVEL as usize + j)), &level)?;
            }
        }
        record(b + 1)?;
        if let Some(l) = op.loaded.write().unwrap().as_mut() {
            l.done = e;
        }
        progress(e as u64, n as u64);
    }

    // the levels above a block, from the last per-block level
    if lens.len() > per_block {
        for c in 0..channels {
            let mut level: Vec<Summary> = {
                let g = op.loaded.read().unwrap();
                let l = g.as_ref().expect("set above");
                let all: &[Summary] = bytemuck::cast_slice(&l.maps[c]);
                all[offs[per_block - 1]..offs[per_block - 1] + lens[per_block - 1]].to_vec()
            };
            for j in per_block..lens.len() {
                level = coarser(&level);
                write_level(c, j, 0, &level)?;
            }
        }
    }
    for f in &files {
        f.sync_data().map_err(Error::io(&building))?;
    }
    let gaps = rec.gap_index();
    let gp = building.join("gaps.bin");
    fs::write(&gp, bytemuck::cast_slice::<Jump, u8>(&gaps.jumps)).map_err(Error::io(&gp))?;
    write_json(
        &building.join("manifest.json"),
        &Manifest {
            format: FORMAT,
            fingerprint: op.fingerprint.clone(),
            samples: n,
            channels,
            levels: lens,
            median_ns: gaps.median_ns,
            jumps: gaps.jumps.len(),
        },
    )?;
    let _ = fs::remove_file(&progress_path);
    drop(files);
    let built = Loaded::open_built(&final_dir, rec)?.expect("just written");
    set(built);
    let _ = fs::remove_file(&lock_path);
    Ok(true)
}
