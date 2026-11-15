// Share on a read-only folder, against the real core (the daemon, no fake), sheet A6: the folder is `chmod a-w`. The tab
// menu's Share with this folder… says it can't write and offers Export file… before anything is tried; the folder's
// Share… dialog opens as Export configuration; nothing is written beside the data. Needs the app binary (as t_exportreal);
// without it the test says so and passes.
import { spawn } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const BIN = [join(ROOT, 'target/release/tlmbench-app'), join(ROOT, 'target/debug/tlmbench-app')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
if (!BIN) {
  console.log('SKIP sharero: needs the app binary (cargo build)');
  console.log('PASS sharero: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const home = mkdtempSync(join(tmpdir(), 'tlm-sharero-'));
const ro = join(home, 'locked');
mkdirSync(ro);
const csv = join(ro, 'run.csv');
writeFileSync(csv, 'time,rpm,temp\n' + Array.from({ length: 2000 }, (_, i) => `${i / 100},${Math.sin(i / 50)},${20 + i / 300}`).join('\n') + '\n');
chmodSync(ro, 0o555);
const before = readdirSync(ro).join();
const daemon = spawn(BIN, ['--daemon', '--ui', join(DIR, '../dist')], { env: { ...process.env, HOME: home }, stdio: ['ignore', 'pipe', 'inherit'] });
const url = await new Promise((res, rej) => {
  daemon.stdout.once('data', (d) => res(String(d).trim()));
  daemon.once('exit', () => rej(new Error('the daemon exited')));
});
const browser = await chromium.launch();
try {
  let answer = ro;
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  page.on('dialog', (d) => d.accept(answer));
  const errs = [];
  page.on('pageerror', (e) => errs.push(e.message));
  const until = async (fn, arg, ms = 10000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await page.evaluate(fn, arg).catch(() => false)) return true;
      await page.waitForTimeout(60);
    }
    return false;
  };
  const text = (s) => page.$eval(s, (e) => e.textContent).catch(() => null);
  await page.goto(url);
  await page.waitForSelector('#wsTabs');
  await page.click('#exFolder');
  check('the read-only folder opens in Explorer', await until(() => document.querySelectorAll('#explorer .row.file').length === 1));
  await page.dblclick('#explorer .row.file');
  await until(() => !!document.getElementById('impGo'));
  await page.click('#impGo');
  check('its CSV opens (the cache holds what the app needs)', await until(() => /run/.test(document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent ?? '')));
  await page.screenshot({ path: join(DIR, 'out/sharero-open.png') });

  // the tab menu's Share with this folder…
  await page.click('#wsTabs [aria-selected=true] .tm');
  await page.waitForSelector('.tabmenu');
  const item = await text('.tabmenu [data-ws="share"]');
  check('the tab menu offers Export file… in place of Share for a read-only folder', /Export file…/.test(item ?? '') && !/Share with this folder/.test(item ?? ''), item);
  answer = join(home, 'run.tlmview.json');
  await page.click('.tabmenu [data-ws="share"]');
  check('which writes the view where you chose', await until((f) => true, null, 300) && (await (async () => { const t = Date.now(); while (Date.now() - t < 5000) { if (existsSync(answer)) return true; await page.waitForTimeout(60); } return false; })()), existsSync(answer));
  await page.screenshot({ path: join(DIR, 'out/sharero-tab.png') });

  // the folder's Share… dialog
  await page.click('#exMore').catch(() => {});
  await page.click('#exFolder');
  await page.click('.exmenu [data-ex="share"]');
  check('the folder\'s Share… opens as Export configuration and says why', await until(() => /Export configuration/.test(document.querySelector('#shareDlg header')?.textContent ?? '') && /can't write|read-only|can’t write/i.test(document.querySelector('#shareDlg')?.textContent ?? '')), await text('#shareDlg'));
  await page.screenshot({ path: join(DIR, 'out/sharero-dlg.png') });
  await page.keyboard.press('Escape');

  check('nothing was written beside the data', readdirSync(ro).join() === before, readdirSync(ro));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
daemon.kill();
await browser.close();
chmodSync(ro, 0o755);
console.log(`${bad ? 'FAIL' : 'PASS'} sharero: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
