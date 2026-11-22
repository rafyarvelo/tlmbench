// The package smoke test (R4): run against an installed TelemetryBench, never the build tree. It checks the CLI answers
// --version with the release's version, opens a recording in a read-only folder through the installed app's daemon
// (`--daemon`, the same commands the window uses) and reads values back, then checks nothing was written next to the
// data. With --manual it also checks the bundled manual is a PDF.
//
//   node tools/smoke.mjs --app <installed tlmbench-app> --cli <installed tlmbench> --version 1.0.0 [--manual <pdf>]
//
// Node 22 or later (the built-in WebSocket). Exits 1 on the first failure, saying what failed.
import { spawn, spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = Object.fromEntries(
  process.argv
    .slice(2)
    .map((a, i, all) => (a.startsWith('--') ? [a.slice(2), all[i + 1]] : null))
    .filter(Boolean),
);
const fail = (what) => {
  console.error(`FAIL ${what}`);
  process.exit(1);
};
const ok = (what) => console.log(`ok   ${what}`);
for (const k of ['app', 'cli', 'version']) if (!args[k]) fail(`--${k} is missing`);
for (const k of ['app', 'cli']) if (!existsSync(args[k])) fail(`${args[k]} isn't there`);

// 1. the command-line tool
const v = spawnSync(args.cli, ['--version'], { encoding: 'utf8' });
if (v.status !== 0) fail(`tlmbench --version exited ${v.status}: ${v.stderr}`);
if (!v.stdout.includes(args.version)) fail(`tlmbench --version says ${JSON.stringify(v.stdout.trim())}, not ${args.version}`);
ok(`tlmbench --version: ${v.stdout.trim()}`);

// 2. a recording in a folder nobody may write to, and a home of the daemon's own
const work = mkdtempSync(join(tmpdir(), 'tlmbench-smoke-'));
const data = join(work, 'data');
const home = join(work, 'home');
for (const d of [data, home]) mkdirSync(d, { recursive: true });
const rows = Array.from({ length: 5000 }, (_, i) => `${(i / 1000).toFixed(3)},${i % 50}`).join('\n');
writeFileSync(join(data, 'run.csv'), `time,x\n${rows}\n`);
if (process.platform !== 'win32') chmodSync(data, 0o555);
const before = readdirSync(data).sort().join();

const env = { ...process.env, HOME: home, USERPROFILE: home, APPDATA: join(home, 'AppData', 'Roaming'), LOCALAPPDATA: join(home, 'AppData', 'Local'), XDG_CONFIG_HOME: join(home, '.config'), XDG_CACHE_HOME: join(home, '.cache'), XDG_DATA_HOME: join(home, '.local', 'share') };
const daemon = spawn(args.app, ['--daemon'], { env, stdio: ['ignore', 'pipe', 'inherit'] });
const stop = (code) => {
  daemon.kill();
  process.exit(code);
};
setTimeout(() => (console.error('FAIL the daemon took over 90 s'), stop(1)), 90_000);

const url = await new Promise((res, rej) => {
  let out = '';
  daemon.stdout.on('data', (d) => {
    out += String(d);
    const m = /http:\/\/127\.0\.0\.1:\d+\/\?token=[0-9a-f]+/.exec(out);
    if (m) res(m[0]);
  });
  daemon.once('exit', (c) => rej(new Error(`the daemon exited (${c})`)));
}).catch((e) => (console.error(`FAIL ${e.message}`), stop(1)));
ok(`the installed app's daemon is up: ${url.replace(/token=.*/, 'token=…')}`);

const u = new URL(url);
const ws = new WebSocket(`ws://${u.host}/ws?token=${u.searchParams.get('token')}`);
ws.binaryType = 'arraybuffer';
const replies = new Map();
await new Promise((res, rej) => {
  ws.onmessage = (e) => typeof e.data === 'string' && res();
  ws.onerror = () => rej(new Error('the socket refused the token'));
}).catch((e) => (console.error(`FAIL ${e.message}`), stop(1)));
ws.onmessage = (e) => {
  if (typeof e.data === 'string') return;
  const f = new DataView(e.data);
  const kind = f.getUint8(0);
  const id = f.getUint32(1, true);
  const body = e.data.slice(5);
  const r = replies.get(id);
  if (!r || kind > 2) return;
  replies.delete(id);
  if (kind === 0) r.ok(JSON.parse(new TextDecoder().decode(body)));
  else if (kind === 1) r.ok(body);
  else r.err(new Error(new TextDecoder().decode(body)));
};
let nextId = 1;
const call = (cmd, a) =>
  new Promise((ok, err) => {
    const id = nextId++;
    replies.set(id, { ok, err });
    ws.send(JSON.stringify({ id, cmd, args: a }));
  });

try {
  const opened = await call('open', { path: join(data, 'run.csv'), progress: '__CHANNEL__:1' });
  if (typeof opened.id !== 'number') fail(`open gave ${JSON.stringify(opened)}`);
  ok('a CSV in a read-only folder opens');
  // 2.512 s after the first sample is sample 2512: 2512 % 50 = 12
  if (!opened.range) fail(`open gave no time range: ${JSON.stringify(opened)}`);
  const at = await call('values_at', { file: opened.id, channels: [0], t: String(BigInt(opened.range[0]) + 2_512_000_000n) });
  const got = at?.[0]?.value;
  if (got !== 12) fail(`the value 2.512 s in is ${JSON.stringify(at)}, not 12`);
  ok('a value reads back from the file');
} catch (e) {
  console.error(`FAIL ${e.message}`);
  stop(1);
}

ws.close();
daemon.kill();
const after = readdirSync(data).sort().join();
if (after !== before) fail(`files appeared next to the data: ${after}`);
ok('nothing was written next to the data');

// 3. the manual
if (args.manual) {
  if (!existsSync(args.manual)) fail(`the manual isn't at ${args.manual}`);
  if (readFileSync(args.manual).subarray(0, 5).toString() !== '%PDF-') fail(`${args.manual} isn't a PDF`);
  ok('the manual is bundled');
}
console.log('PASS smoke');
process.exit(0);
