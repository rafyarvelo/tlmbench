// Reset app… and Import profiles against the real core (the daemon, no fake), owner decision 1: a CSV's first open keeps
// an import profile in the app cache; Settings lists it; Reset app… with Import profiles ticked removes it, so the import
// dialog comes back. Needs the app binary (as t_exportreal); without it the test says so and passes.
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
  console.log('SKIP resetreal: needs the app binary (cargo build)');
  console.log('PASS resetreal: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const home = mkdtempSync(join(tmpdir(), 'tlm-resetreal-'));
const csv = join(home, 'run.csv');
writeFileSync(csv, 'time,rpm,temp\n' + Array.from({ length: 200 }, (_, i) => `${i / 10},${i},${20 + i / 50}`).join('\n') + '\n');
const daemon = spawn(BIN, ['--daemon', '--ui', join(DIR, '../dist')], { env: { ...process.env, HOME: home }, stdio: ['ignore', 'pipe', 'inherit'] });
const url = await new Promise((res, rej) => {
  daemon.stdout.once('data', (d) => res(String(d).trim()));
  daemon.once('exit', () => rej(new Error('the daemon exited')));
});
const browser = await chromium.launch();
const profilesIn = () => {
  const out = [];
  const walk = (d) => { for (const n of existsSync(d) ? readdirSync(d) : []) { const p = join(d, n); if (statSync(p).isDirectory()) (n === 'profiles' ? out.push(...readdirSync(p)) : walk(p)); } };
  walk(join(home, 'Library'));
  walk(join(home, '.cache'));
  return out;
};
try {
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  page.on('dialog', (d) => d.accept(csv));
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
  await page.goto(url);
  await page.waitForSelector('#wsTabs');
  await page.keyboard.press('ControlOrMeta+o');
  check('the first open asks how to import', await until(() => !!document.getElementById('impGo')));
  await page.click('#impGo');
  await until(() => /run/.test(document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent ?? ''));
  check('the import profile is kept in the app cache', profilesIn().length === 1, profilesIn());
  await page.click('[data-rail="settings"]');
  await page.locator('#profileSet').scrollIntoViewIfNeeded();
  check('Settings lists it by its columns', await until(() => /time, rpm, temp/.test(document.querySelector('#profileSet')?.textContent ?? '')));
  await page.click('#setResetApp');
  await until(() => !!document.querySelector('#resetDlg'));
  for (const k of ['settings', 'recipes', 'checks', 'layouts', 'events', 'widgets', 'alignments', 'cache']) await page.uncheck(`#resetDlg [data-reset="${k}"]`);
  await page.click('#resetGo');
  check('Reset app… with Import profiles removes it', await until(() => /^Reset: 1 item/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')) && profilesIn().length === 0, profilesIn());
  await page.click('#wsTabs [aria-selected=true] .tx');
  await page.keyboard.press('ControlOrMeta+o');
  check('so the import dialog comes back', await until(() => !!document.getElementById('impGo')));
  check('nothing was written beside the CSV', readdirSync(home).filter((n) => !n.startsWith('.') && n !== 'Library').join() === 'run.csv', readdirSync(home));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
daemon.kill();
await browser.close();
console.log(`${bad ? 'FAIL' : 'PASS'} resetreal: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
