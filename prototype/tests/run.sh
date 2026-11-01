#!/bin/bash
# The prototype's test suite: every t_*.js here, 3 at a time (timing tests alone, last). Prints each test's result and any failed
# checks, then a summary; exits 1 if anything failed.   Usage: ./run.sh [t_name ...]
cd "$(dirname "$0")"
export NODE_PATH="$(pwd)/node_modules:$(npm root -g)"
# GNU timeout: `timeout` on Linux, `gtimeout` from Homebrew coreutils on a Mac
TIMEOUT=$(command -v timeout || command -v gtimeout) || { echo "run.sh needs GNU timeout (brew install coreutils)"; exit 2; }
export TIMEOUT
tests=${@:-$(ls t_*.js | sed 's/\.js$//')}
mkdir -p out
run1() { t=$1; s=$(date +%s); "$TIMEOUT" 240 node $t.js > out/$t.txt 2>&1; c=$?; d=$(( $(date +%s) - s ));
  if [ $c -eq 0 ]; then echo "PASS $t (${d}s) $(tail -1 out/$t.txt | sed 's/.*: //')"; else echo "FAIL $t (${d}s)"; grep -E "^  FAIL" out/$t.txt | sed 's/^/     /'; fi; }
export -f run1
# timing tests (they measure how fast things appear or draw) run alone at the end, so other tests' load can't skew them
SERIAL="t_loading t_perf_gauges"
pat=$(printf '%s\n' $SERIAL); par=$(printf '%s\n' $tests | grep -vxF "$pat"); ser=$(printf '%s\n' $tests | grep -xF "$pat")
{ printf '%s\n' $par | grep . | xargs -P 3 -I{} bash -c 'run1 {}'; for t in $ser; do run1 $t; done; } | tee out/summary.txt
n=$(grep -c "^PASS" out/summary.txt); f=$(grep -c "^FAIL" out/summary.txt)
echo "== $n passed, $f failed"
[ $f -eq 0 ]
