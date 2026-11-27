# Gates and measurements

A gate is a scripted run with a pass condition, so a performance or correctness claim in the design is judged by a number. They run on the Mac, in the release build, with the window visible (a hidden window or a sleeping display pauses animation frames and the run is reported invalid). Results are written with the commit they judged; the release candidate reruns every one on its own build.

## Large test recordings

Recordings are generated, never stored or committed: the generator is deterministic from a seed. They go to
`$TLMBENCH_DATA` (default `~/tlmbench-data`); point it at an external drive for the 10 GB and 100 GB sets. 10 GB serves
the first-paint target and the CLI's flat-memory claim for `convert` and `check`; 100 GB serves `convert`'s. Neither suits
CI runners (about 14 GB free), so they run on the Mac (`just gen-gate2`, below). Mounted read-only (an `hdiutil` image),
they also extend `just ro-test`.

## The Phase 1 gate

`just gen-gate` writes the gate recording (100M samples, one channel, as `.tlm` ~400 MB and CSV ~2.4 GB) to
`$TLMBENCH_DATA/gate`. `just gate` builds the release app and runs a scripted 30 s sweep over it (zoom from the whole
file to 1 s and back, pans), then prints a JSON report and exits 0 on pass. Keep the window visible: a hidden window
or sleeping display pauses animation frames, and the run is reported as invalid.

Pass: at most 1% of frames miss a vsync and no interval exceeds 3 refresh periods; the core reduces views of up to
1M samples in p95 < 5 ms (timed in Rust; the webview's clocks round to 1 ms). Whole-file views are reported only.

Last run (MacBook Air M1, 8 GB, 59 Hz, 2264 px): pass. 1799 frames, 3 missed; reduce p95 1.4 ms for views of up to
1M samples, whole-file p50 21 ms (657 ms on a cold first read); round trip p50 4 ms.

## The Phase 2 gate

`just gen-gate2` writes the Gate 2 recording (a 10 GB `.tlm`, 20 channels × 125M f32 samples) to
`$TLMBENCH_DATA/gate2` in about 30 s. It needs about 12 GB free: the file and its 1.1 GB pyramid. `just gate2` builds the
pyramid with the CLI into a fresh cache, drops the file's and the cache's pages from memory (`ci/evict.py`, no root),
and times a reopen in the release app; then it clears the cache and times a first-ever open. It prints two JSON reports,
exits 0 when the reopen passes, and deletes the 10 GB file afterwards (`just gate2 <file> 1` keeps it). Keep the window
visible.

Pass: the reopen's first paint of the whole view (all 20 channels, coarse counts) within 1 s of asking to open.
Reported: when the exact view replaced it, the 30 s sweep of Phase 1 over all 20 channels, and for the first-ever open,
when axes and progress showed and how long the build took.

Last run (MacBook Air M1, 8 GB, 59 Hz, 1736 px, 30 GB free): pass. First paint 70 ms (coarse), exact 1.3 s; the
20-channel sweep missed 1 of 1,800 frames, longest interval 40 ms, zoomed 20-trace replies p95 4.4 ms in the core (15.7 ms
before the chunk cache and view batches). First-ever open: axes and progress in 62 ms, build 9.9 s (1,014 MB/s; the disk
reads the file cold in 9.5 s; the run before took 12.5 s). Leave 20 GB free:
on a nearly full disk writes slow to 74 MB/s and the build takes 20 s.

## The Phase 3 gate

`just gate3 core` judges the live path without the app (`ci/gate3.py`): `tlmbench-gen --send` streams records (a u64
sequence number and an f32 per channel) at 4M samples/s to `tlmbench record` on localhost, for 60 s in each of two
shapes, 4 channels at 1 MHz and 32 at 125 kHz, over TCP and then UDP. Each recording is then checked record by record
against the sender (`tlmbench-gen --verify`) and deleted. Then it tails a growing CSV (`tlmbench-gen --append`, lines
cut mid-way, truncated once half-way) at 1M and 4M samples/s with `tlmbench record file://…`. It needs about 2 GB
free while a run lasts and takes about 7 minutes; `just gate3 core 5` runs 5 s each, for a quick check.

Pass (TCP runs): every record recorded and identical, none dropped by the ring buffer, no sequence number missing or
repeated, and the ring never more than half full. A run where the sender fell more than 0.1 s behind its schedule is
invalid, not failed. Reported: UDP runs (localhost UDP can lose datagrams in the kernel, which shows as missing
sequence numbers) and CSV tails (whether the tail kept up, rows identical, and the rows a truncation took before they
were read). `just gate3 app` runs the same TCP shapes into the release app (keep its window visible): a live tab follows each
stream with a 10 s window until it ends. Pass: at most 1% of frames missed while following, nothing dropped or
missing, the ring at most half full, and the recording identical to what was sent (checked after); the longest
frame is reported. Last run: 0 of 3,539 and 3 of 3,541 frames missed (longest 24 and 36 ms), every record kept.

Last run (MacBook Air M1, 8 GB, Nov 2, 2026): pass. Every TCP and UDP run recorded all its records bit for bit, none
dropped or missing, the ring buffer at most 0.7% of its 229 MB; the recorder took 3.7% of a core for TCP 4 × 1 MHz
(22.9 MB/s written), 6.5% for TCP 32 × 125 kHz (16.2 MB/s), 45% and 22% for UDP (one datagram a read). The CSV tails
kept up at both rates with every recorded row identical, at 11% and 17% of a core; the truncation took 5,221 rows
(about 20 ms) at 1M samples/s and none at 4M. A finished CSV is tailed at about 7.5M rows/s.

## Zoomed views and the chunk cache

`cargo bench -p tlmbench-store --bench chunks` times 1M-sample views (1736 px, a raw reduction) of all 20 channels of
an interleaved f32 `.tlm` (4M samples a channel, written to a temp folder): read in place (no chunk cache, as before
the cache), decoding their chunks, and with the chunks decoded. Disk-cold numbers come from the same views of a 2 GB
file (`just gen --size 2G --channels 20 --format tlm`) with its pages dropped by `ci/evict.py` before each view.

Last run (MacBook Air M1, 8 GB):

| 1M samples × 20 channels | File in the page cache | File pages evicted |
| --- | --- | --- |
| Read in place (before) | 20.1 ms | 82–88 ms |
| Decoding the chunks | 10.5 ms | 44–52 ms |
| Chunks decoded | 3.5 ms | — |

## The formats gate

`just gate4 [matrix|packets|app]` (`ci/gate4.py`): one synthetic recording is written as `.tlm`, CSV, a big-endian scaled
fixed-width file with its own descriptor and packet files (uncompressed, zstd, lz4, gzip), and converted to Parquet, Arrow
IPC, HDF5 and MAT. Every format the core reads must open with the same per-channel count, extremes, mean and standard
deviation, and the same view bit for bit in the binary formats. `packets` adds a compressed packet file of about 10 GB: its
first index build is reported, and a reopen with the index and pyramid cached must draw the whole view in under 1 s (it
needs about 12 GB free). With `app`, every format is opened in the release app too.

## The v1 gate (`just gate5`)

`ci/gate5.py` writes a 2 h flight (20 channels at 100 Hz) and two short ones, runs a check set over the folder, exports
the flight with every channel and its checks, requires it to fit 250 MB in Full mode, serves the folder with a plain
static file server and opens it in headless Chromium (`ci/gate5_open.mjs`), requiring the first chart view in under 2 s and
no page errors. CI doesn't run it (it needs the viewer built and Chromium).

## The release candidate's pass

On the final commit's release build, in this order: `just testdata big` (the 100M-sample file in
`tools/testdata/make.py`), `just gen-gate` and `just gate`; `just gen-gate2` and `just gate2`; `just gate3 core` and
`just gate3 app`; `just gate4`; `just gate5`; then the soak, an hour of a live tab at 4M samples per second. Check
`df -h /` first: the big files need 10 GB and more, and each recipe deletes what it wrote.

### 1.0.0-rc.2 (Nov 26, 2026, `657160f`; tagged before the window gates, owner's call)

MacBook Air M1, 8 GB; the release build of `657160f`. The gates that open the app's window (`gate`, `gate2`, `gate3 app`,
`gate4 app`) and the soak are still to run: the screen locked during `just gate` and its window never drew (a locked
screen stops a window's frames even under `caffeinate -d`; turn off "Require password" for the run). A bug they find
starts rc.3.

| Gate | Result |
| --- | --- |
| `just gate3 core` | pass: 4 × 1 MHz and 32 × 125 kHz, TCP and UDP, every record identical, none dropped by the ring; CSV tails reported |
| `just gate4` | pass: no failures in the format and convert matrix; the 10 GB zstd packet file's cached reopen 0.041 s (first index build 75.9 s, 136 MB/s) |
| `just gate5` | pass: first chart view of the Full export 354 ms, no page errors |

### 1.0.0-rc.1 (Nov 24, 2026, `63360a1`)

MacBook Air M1, 8 GB, 59 Hz, a 1580 px window, 23 GB free; every gate on the release build of `63360a1`.

| Gate | Result |
| --- | --- |
| `just testdata big` | written (`big/big-100M.tlm`, 400 MB) |
| `just gate` | pass: 1 of 1,801 frames missed, longest 1.6 periods; zoomed reduce p95 0.73 ms, whole file p50 3.7 ms; round trip p50 3 ms. (On `a8d9652` the first of three runs failed by one 60 ms frame, 3.5 periods, right after the 2.8 GB of gate files were written; the next two passed.) |
| `just gate2` | pass: reopen first paint 154 ms (coarse), exact 1.3 s; 20-trace sweep 2 of 1,801 frames missed, longest 1.6 periods, zoomed replies p95 4.9 ms. First-ever open: axes and progress 99 ms, build 7.3 s (1,375 MB/s) |
| `just gate3 core` | pass: 4 × 1 MHz and 32 × 125 kHz, TCP and UDP, every record identical, none dropped or missing, ring at most 0.12%; recorder 2.4% and 5.5% of a core (TCP). CSV tails kept up at 1M and 4M samples/s |
| `just gate3 app` | pass: 0 of 3,539 frames missed in both shapes (longest 21 and 25 ms), every record verified |
| `just gate4` | pass: 53 format and convert checks; the 10 GB zstd packet file's cached reopen 0.048 s (first index build 80.5 s, 128 MB/s). On `a8d9652` it failed (53.5 s): with under 10 GB free, the pyramid's build evicted the packet index (fixed in `644df60`; the same low-disk case now reopens in 0.03 s) |
| `just gate4 app` | pass: all nine files read the same in the app |
| `just gate5` | pass: the 2 h flight exports Full at 118.8 MB in 0.3 s; first chart view 284 ms, no page errors |
| Soak | pass, an hour (Nov 24, external exFAT USB drive, `TMPDIR` pointed at it so the recording went there): a live tab on 32 × 125 kHz (4M samples/s) over TCP. 450M records sent, recorded and verified, none dropped or missing, the ring under half full; 178 of 215,893 frames missed (0.08%), longest 81 ms; core reduce p95 12.1 ms, round trip p95 15 ms. App memory 420 to 1,105 MB, mean 782 MB in the first quarter hour and 744 MB in the last (no growth); WebKit 139 to 208 MB, 154 and 169 MB. 57 GB written. A first run of 15 min on the internal disk: the same, 46 of 53,927 frames missed |

The hour needs about 60 GB free for its recording (a live tab writes everything it receives, about 16 MB/s at 4M
samples/s; 86 GB for 4 × 1 MHz): run it with `TMPDIR` on a drive that sustains 20 MB/s or more, e.g.
`TMPDIR=/Volumes/X python3 ci/gate3.py --bin target/release --app --shape 32 --seconds 3600`, with the app's memory
sampled each minute (`ps -axo rss=,comm=`).
