#!/bin/bash
# The app's UI tests: every t_*.js and t_*.mjs here against the built UI (dist/) with the fake core, 3 at a time.
# Prints each test's result and any failed checks, then a summary; exits 1 if anything failed.
# Usage: ./run.sh [t_name ...]   (build first: npm run build, or `just app-test`, which does)
cd "$(dirname "$0")"
TIMEOUT=$(command -v timeout || command -v gtimeout) || { echo "run.sh needs GNU timeout (brew install coreutils)"; exit 2; }
export TIMEOUT
tests=${@:-$(ls t_*.js t_*.mjs 2>/dev/null | sed -E 's/\.m?js$//')}
mkdir -p out
run1() { t=$1; limit=120; case $t in t_perf_gauges|t_loading) limit=300 ;; esac; f=$t.js; [ -f $f ] || f=$t.mjs; s=$(date +%s); "$TIMEOUT" $limit node $f > out/$t.txt 2>&1; c=$?; d=$(( $(date +%s) - s ));
  if [ $c -eq 0 ]; then echo "PASS $t (${d}s) $(tail -1 out/$t.txt | sed 's/.*: //')"; else echo "FAIL $t (${d}s)"; grep -E "^  FAIL" out/$t.txt | sed 's/^/     /'; fi; }
export -f run1
printf '%s\n' $tests | grep . | xargs -P 3 -I{} bash -c 'run1 {}' | tee out/summary.txt
n=$(grep -c "^PASS" out/summary.txt); f=$(grep -c "^FAIL" out/summary.txt)
echo "== $n passed, $f failed"
[ $f -eq 0 ]
