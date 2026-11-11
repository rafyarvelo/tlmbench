// Live sources from CSV files against the real daemon (no fake core): a file playback and a tail of a growing CSV, each a file
// in Explorer with its channels and state, with and without a folder showing; the tail keeps growing; two sources are numbered slots.
// Needs the app binary (cargo build -p tlmbench-app); without it the test says so and passes.
import { spawn } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const BIN = [join(ROOT, 'target/release/tlmbench-app'), join(ROOT, 'target/debug/tlmbench-app')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
if (!BIN) {
  console.log('SKIP livecsv: no app binary (cargo build -p tlmbench-app)');
  console.log('PASS livecsv: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const home = mkdtempSync(join(tmpdir(), 'tlm-livecsv-'));
const dir = join(home, 'data');
mkdirSync(dir);
const csv = (name, cols, rows) => {
  const f = join(dir, name);
  writeFileSync(f, `time,${cols.join(',')}\n` + Array.from({ length: rows }, (_, i) => `${i / 10},` + cols.map((_c, k) => Math.sin(i / 20 + k)).join(',')).join('\n') + '\n');
  return f;
};
const a = csv('a.csv', ['rpm', 'temp'], 300);
const b = csv('b.csv', ['volt', 'amp', 'gyro'], 300);
const grow = join(home, 'grow.csv');
writeFileSync(grow, 'time,alt,spd\n0,1,2\n0.1,2,3\n');

const daemon = spawn(BIN, ['--daemon', '--ui', join(DIR, '../dist')], { env: { ...process.env, HOME: home }, stdio: ['ignore', 'pipe', 'inherit'] });
const url = await new Promise((res, rej) => {
  daemon.stdout.once('data', (d) => res(String(d).trim()));
  daemon.once('exit', () => rej(new Error('the daemon exited')));
});
const browser = await chromium.launch();
let p;
let answer = a;
const fresh = async () => {
  if (p) await p.context().close();
  p = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  p.on('dialog', (d) => d.accept(answer));
  await p.goto(url);
  await p.waitForSelector('#wsTabs');
};
const until = async (fn, arg, ms = 8000) => {
  const t = Date.now();
  while (Date.now() - t < ms) {
    if (await p.evaluate(fn, arg).catch(() => false)) return true;
    await p.waitForTimeout(100);
  }
  return false;
};
const kinds = () => p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.className.split(' ')[1]));
const names = (sel) => p.$$eval(sel, (r) => r.map((x) => (x.querySelector('.nm') ?? x).textContent ?? ''));
const connect = async (kind, field, path) => {
  await p.click('[data-rail="live"]');
  await p.waitForSelector('#liveDlg');
  await p.click(`[data-kind=${kind}]`);
  await p.fill(field, path);
  await p.click('#lvConnect');
  await p.click('[data-rail="explorer"]');
};
const chans = () => names('#explorer .tree .row.chan').then((n) => n.map((x) => x.trim()));

try {
  // a CSV playback, no folder
  await fresh();
  await connect('playback', '#lvRecording', a);
  check('playback of a CSV: a file row for the source, named for it', await until(() => document.querySelector('#explorer .row.file .nm')?.textContent === 'a.csv playback'), await kinds());
  check('...with its channels', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2), await chans());
  check('...and its state', await until(() => /Streaming|Ended/.test(document.querySelector('#explorer .row.file [data-lvstate]')?.textContent ?? '')));
  check('the Open list and the tab name the source', (await names('#openFiles .ofnm')).join() === 'a.csv playback' && (await names('#wsTabs [aria-selected=true]')).join().includes('a.csv playback'), await names('#openFiles .ofnm'));

  // tailing a growing CSV
  await fresh();
  await connect('file', '#lvPath', grow);
  check('a tailed CSV is a file with its channels', (await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2)) && (await chans()).join().includes('alt'), await chans());
  const before = await p.textContent('#lvRec');
  appendFileSync(grow, Array.from({ length: 40 }, (_, i) => `${1 + i / 10},${i},${i * 2}`).join('\n') + '\n');
  check('the session grows with the file', await until((r) => document.getElementById('lvRec')?.textContent !== r, before), await p.textContent('#lvRec'));
  check('...the core stays up and the channels stay listed', (await p.$$('#explorer .tree .row.chan')).length === 2);

  // a folder showing, then a playback of a CSV outside nothing: the source is above the folder's files
  await fresh();
  answer = dir;
  await p.click('#exFolder');
  check('the folder lists its CSVs', await until(() => document.querySelectorAll('#explorer .row.file').length === 2));
  await connect('playback', '#lvRecording', b);
  check('with a folder showing, the live source is a file row above the folder\'s files, with its channels', await until(() => document.querySelector('#explorer .row.file .nm')?.textContent === 'b.csv playback' && document.querySelectorAll('#explorer .tree .row.chan').length === 3), await kinds());

  // two sources: numbered slots, each with its channels and state
  await p.click('#lvAddSrc');
  await p.waitForSelector('#liveDlg');
  await p.click('[data-kind=playback]');
  await p.fill('#lvRecording', a);
  await p.click('#lvConnect');
  await p.click('[data-rail="explorer"]');
  check('two sources are numbered slots with their channels', await until(() => document.querySelectorAll('#explorer .row.slot').length === 2 && document.querySelectorAll('#explorer .tree .row.chan').length === 5), await kinds());
  check('...each with its state', await until(() => document.querySelectorAll('#explorer .row.slot [data-lvstate]').length === 2));
  check('the Open list names each source', (await names('#openFiles .ofnm')).join() === 'b.csv playback,a.csv playback', await names('#openFiles .ofnm'));
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
daemon.kill();
await browser.close();
console.log(`${bad ? 'FAIL' : 'PASS'} livecsv: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
