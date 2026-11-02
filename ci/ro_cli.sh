#!/usr/bin/env bash
# Every CLI command that reads recordings, against each CSV under <folder>, with a throwaway cache. ro_check.py runs
# this and fails if anything under the folder changed. Each roadmap phase adds the commands it brings.
set -euo pipefail
dir="$1"; bin="${TLMBENCH_BIN:-target/debug/tlmbench}"
c="$(mktemp -d)"; trap 'rm -rf "$c"' EXIT
find "$dir" -name '*.csv' -print0 | while IFS= read -r -d '' f; do
  "$bin" info --cache "$c" "$f" > /dev/null
  "$bin" cache build --quiet --cache "$c" "$f" > /dev/null
  "$bin" info --json --cache "$c" "$f" > /dev/null
  "$bin" query view --cache "$c" --width 500 "$f" > /dev/null
  "$bin" query values --cache "$c" --at 0 "$f" > /dev/null
  "$bin" query stats --cache "$c" "$f" > /dev/null
  "$bin" record "file://$f" --out "$c/rec" --idle-end 0.2 --json > /dev/null
done
# record: a session folder goes only where --out says (nothing is sent here, so it records nothing)
printf 'schema = 1\nrecord_size = 8\nsequence = "seq"\n[time]\nsource = "rate"\nrate_hz = 1000.0\n[[fields]]\nname = "seq"\ntype = "u32"\noffset = 0\n[[fields]]\nname = "v"\ntype = "f32"\noffset = 4\n' > "$c/live.toml"
"$bin" record udp://127.0.0.1:0 --descriptor "$c/live.toml" --out "$c/rec" --seconds 0.3 --json > /dev/null
"$bin" cache list --cache "$c" > /dev/null
"$bin" cache clear --cache "$c" > /dev/null
