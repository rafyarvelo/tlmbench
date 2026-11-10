// The UI in a browser tab against the real daemon (`tlmbench-app --daemon`): no fake core. Opens a recording by path,
// draws it, keeps settings in the core and shows them in a second tab at once, and refuses a wrong token.
// Needs the app binary (cargo build -p tlmbench-app); without it the test says so and passes.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
// the newest build of the app (a release build can be older than the debug one)
const BIN = [join(ROOT, 'target/release/tlmbench-app'), join(ROOT, 'target/debug/tlmbench-app')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');

if (!BIN) {
  console.log('SKIP browser: no app binary (cargo build -p tlmbench-app)');
  console.log('PASS browser: 0 ok, 0 failed (skipped)');
  process.exit(0);
}

let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};
const until = async (p, fn, arg, ms = 8000) => {
  const t = Date.now();
  while (Date.now() - t < ms) {
    if (await p.evaluate(fn, arg).catch(() => false)) return true;
    await p.waitForTimeout(50);
  }
  return false;
};

const home = mkdtempSync(join(tmpdir(), 'tlm-browser-'));
const csv = join(home, 'bench run.csv');
writeFileSync(csv, 'time,rpm,temp\n' + Array.from({ length: 3000 }, (_, i) => `${i / 100},${Math.sin(i / 50)},${20 + i / 300}`).join('\n') + '\n');
// the daemon keeps its settings and cache under a home of its own
// TLMBENCH_GUI=1: the desktop app itself with browser access on (opens its window), instead of the daemon
const GUI = process.env.TLMBENCH_GUI === '1';
const daemon = spawn(BIN, GUI ? ['--browser'] : ['--daemon', '--ui', join(DIR, '../dist')], { env: { ...process.env, HOME: home }, stdio: ['ignore', 'pipe', 'inherit'] });
const url = await new Promise((res, rej) => {
  daemon.stdout.once('data', (d) => res(String(d).trim()));
  daemon.once('exit', () => rej(new Error('the daemon exited')));
});

const browser = await chromium.launch();
try {
  const page = async () => {
    const p = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
    p.on('dialog', (d) => d.accept(csv));
    return p;
  };
  check('the daemon prints the page address with a token', /^http:\/\/127\.0\.0\.1:\d+\/\?token=[0-9a-f]{32}$/.test(url), url);

  // a wrong token: the page loads but the socket is refused, and says so
  const bad1 = await page();
  await bad1.goto(url.replace(/token=.*/, 'token=wrong'));
  await bad1.waitForTimeout(800);
  check('a wrong token gets no app', !(await bad1.$('#wsTabs')));

  const a = await page();
  const errs = [];
  a.on('pageerror', (e) => errs.push(e.message));
  if (process.env.DEBUG_BROWSER) a.on('console', (m) => console.log('console:', m.text()));
  await a.goto(url);
  check('the page starts against the daemon', await until(a, () => !!document.getElementById('wsTabs')));
  await a.keyboard.press('ControlOrMeta+o');
  // a CSV goes through the import dialog first
  await until(a, () => !!document.getElementById('impGo'));
  await a.click('#impGo');
  check('Open… asks for a path and opens the recording', await until(a, () => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'bench run.csv'));
  check('its channels are drawn', await until(a, () => window.__tlmApp.drawn().length > 0 && window.__tlmApp.drawn().every((d) => d && d.length)));
  check('with no script error', errs.length === 0, errs);

  // a shared setting: the core keeps it, and another tab shows it at once
  const b = await page();
  await b.goto(url);
  await until(b, () => !!window.__tlmSettings);
  await a.evaluate(() => window.__tlmSettings.set('theme', 'light'));
  check('another tab gets the setting at once', await until(b, () => window.__tlmSettings.get('theme') === 'light'));
  const found = () => (function walk(d) {
    for (const n of readdirSync(d)) {
      const f = join(d, n);
      if (n === 'settings.json') return f;
      if (statSync(f).isDirectory()) { const r = walk(f); if (r) return r; }
    }
    return null;
  })(home);
  await a.waitForTimeout(300);
  const file = found();
  check('the core keeps it for the next start', !!file && JSON.parse(readFileSync(file, 'utf8')).theme === 'light', file);
} finally {
  await browser.close();
  daemon.kill();
}
console.log(`${bad ? 'FAIL' : 'PASS'} browser: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
