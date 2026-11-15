// Explorer follows its folder against the real core (the daemon, no fake), D-ui-02: a file written into the open folder
// shows in Explorer without asking, and one removed goes; nothing is written there by the app. Needs the app binary (as
// t_exportreal); without it the test says so and passes.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const BIN = [join(ROOT, 'target/release/tlmbench-app'), join(ROOT, 'target/debug/tlmbench-app')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
if (!BIN) {
  console.log('SKIP watchreal: needs the app binary (cargo build)');
  console.log('PASS watchreal: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const home = mkdtempSync(join(tmpdir(), 'tlm-watchreal-'));
const dir = join(home, 'runs');
mkdirSync(dir);
const csv = (n) => 'time,rpm\n' + Array.from({ length: n }, (_, i) => `${i / 10},${i}`).join('\n') + '\n';
writeFileSync(join(dir, 'a.csv'), csv(50));
const daemon = spawn(BIN, ['--daemon', '--ui', join(DIR, '../dist')], { env: { ...process.env, HOME: home }, stdio: ['ignore', 'pipe', 'inherit'] });
const url = await new Promise((res, rej) => {
  daemon.stdout.once('data', (d) => res(String(d).trim()));
  daemon.once('exit', () => rej(new Error('the daemon exited')));
});
const browser = await chromium.launch();
try {
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  page.on('dialog', (d) => d.accept(dir));
  const errs = [];
  page.on('pageerror', (e) => errs.push(e.message));
  const until = async (fn, arg, ms = 10000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await page.evaluate(fn, arg).catch(() => false)) return true;
      await page.waitForTimeout(100);
    }
    return false;
  };
  const rows = () => page.$$eval('#explorer .row.file', (r) => r.map((x) => x.textContent.trim()));
  await page.goto(url);
  await page.waitForSelector('#wsTabs');
  await page.click('#exFolder');
  check('the folder opens in Explorer', await until(() => document.querySelectorAll('#explorer .row.file').length === 1), await rows());
  await page.waitForTimeout(500);
  writeFileSync(join(dir, 'b.csv'), csv(50));
  const names = () => [...document.querySelectorAll('#explorer .row.file')].map((r) => r.dataset.path.split(/[\\/]/).pop()).join();
  check('a file written into it shows without asking', await until((n) => (0, eval)(n)() === 'a.csv,b.csv', names.toString(), 8000), await rows());
  rmSync(join(dir, 'a.csv'));
  check('and one removed goes', await until((n) => (0, eval)(n)() === 'b.csv', names.toString(), 8000), await rows());
  check('nothing was written into the folder by the app', readdirSync(dir).join() === 'b.csv', readdirSync(dir));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
daemon.kill();
await browser.close();
console.log(`${bad ? 'FAIL' : 'PASS'} watchreal: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
