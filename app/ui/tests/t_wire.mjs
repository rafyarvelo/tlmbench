// The wire format three ways: the core's golden batch (crates/core/src/wire.rs, golden_batch) must equal what the
// fake core encodes from the same traces, and wire.ts must decode it to those traces. No browser.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

import { decodeBatch } from '../src/wire.ts';

const require = createRequire(import.meta.url);
require('./fakecore.js');
const { encode, encodeBatch } = globalThis.__tlmFakeCore;

let failed = 0;
let passed = 0;
const check = (label, cond, got) => {
  if (cond) passed++;
  else failed++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${label}${cond || got === undefined ? '' : ' — got ' + JSON.stringify(got, (_, v) => (typeof v === 'bigint' ? String(v) : v))}`);
};

const golden = readFileSync(new URL('../../../crates/core/tests/fixtures/wire-batch.bin', import.meta.url));
const t0 = 1767225600000000123n;
const pane = { t0: String(t0), t1: String(t0 + 10000n), widthPx: 4 };
const ov = { t0: '0', t1: '1000', widthPx: 3 };
const mine = encodeBatch(7, [
  [0, encode(7, pane, t0 + 5000n, [
    { channel: 2, kind: 'm4', coarse: true, fromPyramid: true, reduceUs: 11, t: [t0 - 500n, t0, t0 + 2500n, t0 + 7499n, t0 + 10000n], y: [1.5, -2, NaN, 3.25, 0.5] },
    { channel: 5, kind: 'raw', coarse: false, fromPyramid: false, reduceUs: 3, t: [t0 + 1000n, t0 + 9000n], y: [-1, 1] },
  ])],
  [0xffffffff, encode(7, ov, null, [{ channel: 0, kind: 'raw', coarse: false, fromPyramid: true, reduceUs: 1, t: [0n, 999n], y: [4, 4] }])],
]);
const same = Buffer.from(mine).equals(golden);
let at = -1;
if (!same) at = [...golden].findIndex((b, i) => b !== new Uint8Array(mine)[i]);
check('the fake core encodes the golden batch byte for byte', same, { at, mine: mine.byteLength, golden: golden.length });

const b = decodeBatch(golden.buffer.slice(golden.byteOffset, golden.byteOffset + golden.length));
const p = b.entries.get(0);
const o = b.entries.get(0xffffffff);
check('generation and entries', b.generation === 7 && b.entries.size === 2, [b.generation, [...b.entries.keys()]]);
check('origin keeps every nanosecond', p.originNs === t0, p.originNs);
check('bucket and built-until', p.xScaleNs === 2500 && p.builtUntil === t0 + 5000n && o.builtUntil === null, [p.xScaleNs, p.builtUntil]);
const [m, r] = p.traces;
check('M4 trace flags', m.channel === 2 && m.kind === 'm4' && m.coarse && m.fromPyramid && m.reduceUs === 11, m);
check('times come back as buckets from the origin', [...m.x].join() === [-0.2, 0, 1, 2.9996, 4].map(Math.fround).join(), [...m.x]);
check('a NaN is a gap', Number.isNaN(m.y[2]) && m.y[3] === 3.25, [...m.y]);
check('y range covers only samples inside the view', m.yMin === -2 && m.yMax === 3.25, [m.yMin, m.yMax]);
check('raw trace', r.channel === 5 && r.kind === 'raw' && !r.coarse && !r.fromPyramid && [...r.y].join() === '-1,1', r);
check('overview entry', o.traces[0].yMin === 4 && o.traces[0].yMax === 4 && o.xScaleNs === 333, o.traces[0]);

console.log(`${failed ? 'FAIL' : 'PASS'} wire: ${passed} ok, ${failed} failed`);
process.exit(failed ? 1 : 0);
