#!/usr/bin/env python3
"""Test data for the release test plan (docs/release/test-plan.md). Plain Python 3, no packages, any OS: the same
files every time, so a result on one machine means the same on another.

    python3 make.py [folder]          write everything (default: tlmbench-testdata in your home folder)
    python3 make.py big [folder]      also write big/big-100M.tlm, 100M samples (about 400 MB, a minute or two)
    python3 make.py grow FILE         append a row a second to FILE (a growing CSV, for Live)
    python3 make.py send [PORT]       stream bench records over TCP on PORT (default 5555) at 1 kHz, for Live;
                                      Ctrl+C stops it, and running it again starts its clock at 0 again

Writes (nothing next to your own data):
    flights/flight-1.csv    60 s at 100 Hz: altitude, speed, battery_v, temp (a good run: every check passes)
    flights/flight-2.csv    another run of the same channels, logged an hour later (for two files in one tab)
    flights/flight-bad.csv  battery drops under 12 V for 10 s and temp spikes (the checks fail here)
    flights/flight-gap.csv  a 30 s hole in the times (gap breaks, "connect gaps")
    browser/bench.tlm       a small tlmbench reference file (4 channels, 200,000 samples) for browser-only mode
    browser/codecs-*.parquet  the same small table in Parquet, snappy and Zstandard (written when pyarrow is installed)
    flights/.tlmbench/      shared config: a check set, a layout, two widgets (SVG and code), events for flight-1,
                            and decoders/bench.toml (the descriptor for bench.bin and for `send`)
    flights/bench.bin       60 s of 12-byte binary records at 1 kHz, read through that descriptor
    flights/modes.csv       a text column (mode: IDLE, TAXI, CLIMB, CRUISE, LAND) beside a number, as a labelled channel
    big/big-100M.tlm        (with `big`) one channel, 100M samples at 1 MHz: the slow build, hatching and coarse views
    grow.py                 python3 grow.py live.csv  appends a row per second, for the Live tests
"""
import math
import random
import shutil
import struct
import sys
import time
from pathlib import Path

HEADER = "time,altitude,speed,battery_v,temp\n"


def row(t, run):
    rnd = random.Random(int(t * 100) * 7 + run)
    alt = 120 + 80 * math.sin(t / 9.0 + run) + rnd.gauss(0, 0.4)
    speed = 18 + 6 * math.sin(t / 5.0) + rnd.gauss(0, 0.2)
    batt = 15.2 - t * 0.012 + rnd.gauss(0, 0.01)
    temp = 31 + t * 0.05 + rnd.gauss(0, 0.05)
    return alt, speed, batt, temp


def write_flight(path, run=0, start=0.0, seconds=60, hz=100, gap=None, bad=False):
    lines = [HEADER]
    n = int(seconds * hz)
    for i in range(n):
        t = i / hz
        if gap and gap[0] <= t < gap[1]:
            continue
        alt, speed, batt, temp = row(t, run)
        if bad and 20 <= t < 30:
            batt = 11.2 + 0.1 * math.sin(t)  # a 10 s low-battery stretch
        if bad and 45 <= t < 45.5:
            temp += 25  # a short spike
        lines.append(f"{start + t:.3f},{alt:.3f},{speed:.3f},{batt:.4f},{temp:.3f}\n")
    path.write_text("".join(lines))


CHECKS = """schema = 1
name = "Flight limits"
runs_on_any_file = true

[[check]]
name = "Battery above 12 V"
group = "Power"
expect = "battery_v > 12"
severity = "fail"
enabled = true

[[check]]
name = "Temperature under 50"
group = "Thermal"
expect = "temp < 50"
severity = "warn"
enabled = true
"""

LAYOUT = (
    '{"schema":1,"name":"Altitude and battery","panes":'
    '[{"channels":["altitude","speed"],"view":"chart"},{"channels":["battery_v"],"view":"chart"},{"channels":["temp"],"view":"table"}]}'
)

DIAL_SVG = (
    '<svg viewBox="0 0 120 96"><path d="M30 86h60" stroke="currentColor" stroke-width="2" opacity=".4"/>'
    '<g transform="rotate({{angle}} 60 86)"><path d="M60 86V30" stroke="{{color}}" stroke-width="6" stroke-linecap="round"/>'
    '<circle cx="60" cy="26" r="9" fill="{{color}}"/></g>'
    '<text x="60" y="14" text-anchor="middle" font-size="12" fill="currentColor">{{value}} {{unit}}</text></svg>'
)

BAR_JS = """// build(el, info) once, update(el, x) per frame
export default {
  build(el, info) { el.innerHTML = '<div style="padding:10px"><b class="t"></b><div class="s" style="font:11px system-ui;opacity:.7"></div></div>'; el.querySelector('.s').textContent = info.name; },
  update(el, x) { el.querySelector('.t').textContent = x.txt; },
};
"""

GROW = '''#!/usr/bin/env python3
"""python3 grow.py live.csv: a CSV that grows by a row a second (Ctrl+C stops it). Open it from Live."""
import math, sys, time
path = sys.argv[1] if len(sys.argv) > 1 else "live.csv"
t0 = time.time()
with open(path, "w") as f:
    f.write("time,altitude,speed,battery_v,temp\\n")
    f.flush()
    i = 0
    while True:
        t = time.time() - t0
        f.write(f"{t:.3f},{120 + 80 * math.sin(t / 9):.3f},{18 + 6 * math.sin(t / 5):.3f},{15.2 - t * 0.012:.4f},{31 + t * 0.05:.3f}\\n")
        f.flush()
        i += 1
        time.sleep(1)
'''


BENCH_FIELDS = (
    '[[fields]]\nname = "t"\ntype = "u32"\noffset = 0\n'
    '[[fields]]\nname = "altitude"\ntype = "f32"\noffset = 4\nunit = "m"\n'
    '[[fields]]\nname = "battery"\ntype = "i16"\noffset = 8\nunit = "V"\nscale = 0.001\n'
    '[[fields]]\nname = "armed"\ntype = "u8"\noffset = 10\n'
)
# the descriptor of bench.bin and of `send`'s frames (a folder's .tlmbench/decoders/, found by the file name)
BENCH_DECODER = ('schema = 1\nname = "Bench records"\nfiles = ["*.bin"]\nrecord_size = 12\n'
                 '[time]\nsource = "field"\nfield = "t"\nunit = "us"\n' + BENCH_FIELDS)


def bench_record(t):
    return struct.pack("<IfhBx", int(t * 1e6) & 0xFFFFFFFF, 120 + 80 * math.sin(t / 9.0), int(12600 - 40 * (t % 60)), 1 if t > 5 else 0)


def write_bin(path, seconds=60, hz=1000):
    path.write_bytes(b"".join(bench_record(i / hz) for i in range(seconds * hz)))


def write_modes(path, seconds=120, hz=10):
    modes = ["IDLE"] * 15 + ["TAXI"] * 15 + ["CLIMB"] * 30 + ["CRUISE"] * 45 + ["LAND"] * 15
    lines = ["time,mode,throttle\n"]
    for i in range(seconds * hz):
        t = i / hz
        m = modes[min(int(t), len(modes) - 1)]
        lines.append(f"{t:.1f},{m},{ {'IDLE': 0, 'TAXI': 15, 'CLIMB': 90, 'CRUISE': 60, 'LAND': 20}[m] + math.sin(t):.2f}\n")
    path.write_text("".join(lines))


def write_big(path, samples=100_000_000, hz=1_000_000):
    """One f32 channel at a fixed rate: a block of a million samples written again and again (fast in plain Python)."""
    desc = (f'schema = 1\nname = "Big"\nrecord_size = 4\n[time]\nsource = "rate"\nrate_hz = {hz}\n'
            '[[fields]]\nname = "signal"\ntype = "f32"\noffset = 0\nunit = "V"\n').encode()
    header_len = -(-(24 + len(desc)) // 64) * 64
    head = b"TLMBENCH" + struct.pack("<HHIII", 1, 0, header_len, len(desc), 0) + desc
    block_n = 1_000_000
    block = struct.pack(f"<{block_n}f", *(math.sin(i / 5000.0) + (3.0 if i % 250_000 == 7 else 0.0) for i in range(block_n)))
    with open(path, "wb") as f:
        f.write(head.ljust(header_len, b"\0"))
        for _ in range(samples // block_n):
            f.write(block)


def send(port=5555, hz=1000):
    """A TCP server: whoever connects gets bench records at `hz`, time from 0 (a device that just started)."""
    import socket
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", port))
    srv.listen(1)
    print(f"Sending on tcp://127.0.0.1:{port}: in TelemetryBench, Connect › TCP, address 127.0.0.1:{port}, with the "
          "descriptor flights/.tlmbench/decoders/bench.toml. Ctrl+C stops.", flush=True)
    try:
        while True:
            conn, _ = srv.accept()
            print("connected", flush=True)
            t0, i = time.time(), 0
            try:
                while True:
                    due = int((time.time() - t0) * hz)
                    if due > i:
                        conn.sendall(b"".join(bench_record(k / hz) for k in range(i, due)))
                        i = due
                    time.sleep(0.01)
            except OSError:
                print("disconnected", flush=True)
    except KeyboardInterrupt:
        pass


def write_tlm(path, samples=200_000, hz=1000):
    """A tlmbench reference file: 4 channels (a u32 time in us, f32 altitude, i16 battery in mV, u8 armed), 12-byte records."""
    desc = (
        'schema = 1\nname = "Bench"\nrecord_size = 12\n[time]\nsource = "field"\nfield = "t"\nunit = "us"\n'
        '[[fields]]\nname = "t"\ntype = "u32"\noffset = 0\n'
        '[[fields]]\nname = "altitude"\ntype = "f32"\noffset = 4\nunit = "m"\n'
        '[[fields]]\nname = "battery"\ntype = "i16"\noffset = 8\nunit = "V"\nscale = 0.001\n'
        '[[fields]]\nname = "armed"\ntype = "u8"\noffset = 10\n'
    ).encode()
    header_len = -(-(24 + len(desc)) // 64) * 64
    head = b"TLMBENCH" + struct.pack("<HHIII", 1, 0, header_len, len(desc), 0) + desc
    with open(path, "wb") as f:
        f.write(head.ljust(header_len, b"\0"))
        for i in range(samples):
            t = i / hz
            f.write(struct.pack("<IfhBx", int(t * 1e6), 120 + 80 * math.sin(t / 9.0), int(12600 - 40 * t), 1 if t > 5 else 0))


def write_parquet(folder):
    """The fixtures' small table in two codecs, where pyarrow is there."""
    try:
        sys.path.insert(0, str(Path(__file__).parent))
        import parquet_codecs
        import pyarrow.parquet as pq
    except ImportError:
        print("pyarrow isn't installed: no Parquet files in browser/")
        return
    for codec in ("snappy", "zstd"):
        pq.write_table(parquet_codecs.table(), folder / f"codecs-{codec}.parquet", compression=codec, row_group_size=1200)


def make(root):
    browser = root / "browser"
    browser.mkdir(parents=True, exist_ok=True)
    write_tlm(browser / "bench.tlm")
    write_parquet(browser)
    flights = root / "flights"
    cfg = flights / ".tlmbench"
    for d in (cfg / "checks", cfg / "views", cfg / "events", cfg / "decoders", cfg / "widgets" / "dial", cfg / "widgets" / "bar-code"):
        d.mkdir(parents=True, exist_ok=True)
    write_flight(flights / "flight-1.csv", run=0)
    write_flight(flights / "flight-2.csv", run=1, start=3600.0)
    write_flight(flights / "flight-bad.csv", run=2, bad=True)
    write_flight(flights / "flight-gap.csv", run=3, gap=(25.0, 55.0), seconds=90)
    write_bin(flights / "bench.bin")
    write_modes(flights / "modes.csv")
    (cfg / "decoders" / "bench.toml").write_text(BENCH_DECODER)
    (cfg / "checks" / "flight-limits.toml").write_text(CHECKS)
    (cfg / "views" / "altitude-and-battery.tlmview.json").write_text(LAYOUT)
    (cfg / "events" / "flight-1.toml").write_text(
        'schema = 1\nrecording = "flight-1.csv"\nidentity = ""\n\n'
        '[[event]]\nid = "ev-takeoff"\nname = "Takeoff"\nstart = 5000000000\nduration = 0\nchannels = ["altitude"]\n\n'
        '[[event]]\nid = "ev-turn"\nname = "Hard turn"\nstart = 30000000000\nduration = 8000000000\nchannels = ["speed", "altitude"]\n'
    )
    (cfg / "widgets" / "dial" / "widget.toml").write_text('schema = 1\nname = "Dial"\nkind = "svg"\nentry = "widget.svg"\nangle = [-60, 60]\n')
    (cfg / "widgets" / "dial" / "widget.svg").write_text(DIAL_SVG)
    (cfg / "widgets" / "bar-code" / "widget.toml").write_text('schema = 1\nname = "Value (code)"\nkind = "code"\nentry = "widget.js"\nby = "you"\n')
    (cfg / "widgets" / "bar-code" / "widget.js").write_text(BAR_JS)
    grow = root / "grow.py"
    grow.write_text(GROW)
    grow.chmod(0o755)


def big(root):
    out = root / "big"
    out.mkdir(parents=True, exist_ok=True)
    print("writing big/big-100M.tlm (about 400 MB)...")
    write_big(out / "big-100M.tlm")


def main():
    a = sys.argv[1:]
    if a[:1] == ["grow"]:
        path = a[1] if len(a) > 1 else "live.csv"
        with open(path, "w") as f:
            f.write(HEADER)
            f.flush()
            t0 = time.time()
            while True:
                t = time.time() - t0
                alt, speed, batt, temp = row(t, 0)
                f.write(f"{t:.3f},{alt:.3f},{speed:.3f},{batt:.4f},{temp:.3f}\n")
                f.flush()
                time.sleep(1)
    if a[:1] == ["send"]:
        return send(int(a[1]) if len(a) > 1 else 5555)
    do_big = a[:1] == ["big"]
    rest = a[1:] if do_big else a
    root = Path(rest[0]).expanduser() if rest else Path.home() / "tlmbench-testdata"
    if root.exists() and any(root.iterdir()) and not (root / "flights").exists():
        sys.exit(f"{root} exists and isn't a test data folder: pick another")
    if (root / "flights").exists():
        shutil.rmtree(root / "flights")
    root.mkdir(parents=True, exist_ok=True)
    make(root)
    if do_big:
        big(root)
    print(f"Wrote {root}\n")
    print(__doc__.split("Writes", 1)[1])


if __name__ == "__main__":
    main()
