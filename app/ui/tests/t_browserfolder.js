// Browser-only mode opens a folder where the browser can (D-brow-10): `showDirectoryPicker` asked for read access only, the
// folder listed read-only in Explorer (dot folders left out, files the app can't read marked), a file in it opening from its
// handle; where the browser has no picker the start page says so once and offers Open recording…. The test gives the page a
// real directory handle (the browser's private storage) in place of the native picker, which headless Chromium can't drive.
// Needs the built page (just viewer); without it the test says so and passes.
import { createReadStream, existsSync, statSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const SITE = join(ROOT, 'app/browser/dist');
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');

if (!existsSync(join(SITE, 'index.html')) || !existsSync(join(SITE, 'tlmbench_web.wasm'))) {
  console.log('SKIP browserfolder: needs the page (just viewer)');
  console.log('PASS browserfolder: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.woff2': 'font/woff2' };
const server = http.createServer((q, s) => {
  const p = join(SITE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(SITE) || !existsSync(p) || statSync(p).isDirectory()) return s.writeHead(404).end();
  s.writeHead(200, { 'content-type': types[extname(p)] ?? 'application/octet-stream' });
  createReadStream(p).pipe(s);
});
await new Promise((res) => server.listen(0, '127.0.0.1', res));
const url = `http://127.0.0.1:${server.address().port}/index.html`;

const rows = ['time,Speed [m/s],temp'];
for (let i = 0; i < 500; i++) rows.push(`${i / 10},${Math.sin(i / 40)},${20 + i / 300}`);
const CSV = rows.join('\n') + '\n';

const browser = await chromium.launch();
try {
  // ---- a browser with the picker: a folder in private storage stands in for the native one
  {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const p = await ctx.newPage();
    const errs = [];
    p.on('pageerror', (e) => errs.push(e.message));
    const until = async (fn, arg, ms = 10000) => {
      const t = Date.now();
      while (Date.now() - t < ms) {
        if (await p.evaluate(fn, arg).catch(() => false)) return true;
        await p.waitForTimeout(60);
      }
      return false;
    };
    await p.addInitScript(() => {
      window.__pickerArgs = [];
      window.showDirectoryPicker = async (o) => {
        window.__pickerArgs.push(o);
        const root = await navigator.storage.getDirectory();
        const dir = await root.getDirectoryHandle('runs', { create: true });
        const put = async (d, name, text) => {
          const w = await (await d.getFileHandle(name, { create: true })).createWritable();
          await w.write(text);
          await w.close();
        };
        await put(dir, 'notes.md', '# notes');
        await put(dir, 'readme.pdf', 'x');
        const sub = await dir.getDirectoryHandle('day 1', { create: true });
        await put(sub, 'run a.csv', window.__csv);
        const hid = await dir.getDirectoryHandle('.tlmbench', { create: true });
        await put(hid, 'secret.toml', 'a = 1');
        await dir.getDirectoryHandle('empty', { create: true });
        return dir;
      };
    });
    await p.goto(url, { waitUntil: 'domcontentloaded' });
    await p.evaluate((c) => (window.__csv = c), CSV);
    check('the page offers Open folder… where the browser has the picker', await until(() => !!document.querySelector('#startPage [data-start="folder"]')));
    check('and still no live source', !(await p.$('#startPage [data-start="live"]')));
    await p.click('#startPage [data-start="folder"]');
    check('the picker is asked for read access only', await until(() => window.__pickerArgs?.length === 1 && window.__pickerArgs[0]?.mode === 'read'), await p.evaluate(() => window.__pickerArgs));
    check('Explorer lists the folder: its name, the sub-folder, no hidden folder, no empty folder', await until(() => /runs/.test(document.querySelector('#exFolder')?.textContent ?? '') && !!document.querySelector('#explorer [data-path="local-folder://runs/day 1"]')) && !(await p.$('#explorer [data-path*=".tlmbench"]')) && !(await p.$('#explorer [data-path="local-folder://runs/empty"]')), await p.$$eval('#explorer .row', (r) => r.map((x) => x.textContent)));
    // the sub-folder is expanded: its CSV is listed and opens from its handle
    if (!(await p.$('#explorer .row.file[data-path="local://runs/day 1/run a.csv"]'))) await p.click('#explorer [data-path="local-folder://runs/day 1"]');
    check('a file in the sub-folder is listed', await until(() => !!document.querySelector('#explorer .row.file[data-path="local://runs/day 1/run a.csv"]')));
    await p.dblclick('#explorer .row.file[data-path="local://runs/day 1/run a.csv"]');
    check('and opens from its handle, in a tab named for the file', await until(() => /run a\.csv/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
    check('with its channels', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2));
    check('files the app cannot read are not listed unless asked', !(await p.$('#explorer [data-path="local://runs/notes.md"]')));
    check('nothing was written into the folder', await p.evaluate(async () => {
      const dir = await (await navigator.storage.getDirectory()).getDirectoryHandle('runs');
      const names = [];
      for await (const [n] of dir.entries()) names.push(n);
      return names.sort().join() === '.tlmbench,day 1,empty,notes.md,readme.pdf';
    }));
    check('the folder menu has no Share… and no Recipes… (a tab writes nothing, recipes come after v1)', await (async () => {
      await p.click('#exFolder');
      const t = await p.$$eval('.exmenu .mi', (e) => e.map((x) => x.textContent));
      await p.keyboard.press('Escape');
      return !t.some((x) => /Share|Recipes/.test(x)) && t.some((x) => /Refresh/.test(x));
    })(), await p.$$eval('.exmenu .mi', (e) => e.map((x) => x.textContent)).catch(() => null));
    check('no page errors', errs.length === 0, errs);
    await ctx.close();
  }

  // ---- a browser without the picker: it says so, and Open recording… is there
  {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const p = await ctx.newPage();
    await p.addInitScript(() => {
      delete window.showDirectoryPicker;
    });
    await p.goto(url, { waitUntil: 'domcontentloaded' });
    await p.waitForSelector('#startPage');
    check('without the picker the start page has no Open folder…', !(await p.$('#startPage [data-start="folder"]')));
    check('it says so, once, and offers Open recording…', /can't open folders/.test(await p.$eval('#startPage', (e) => e.textContent ?? '')) && !!(await p.$('#startPage [data-start="open"]')));
    check('Explorer says the same on hover instead of offering a folder', (await p.$eval('#explorer .exfolder', (e) => e.getAttribute('data-tip') ?? '')).includes("can't open folders") && !(await p.$('#exFolder')));
    await ctx.close();
  }
} finally {
  await browser.close();
  server.close();
}
console.log(`${bad ? 'FAIL' : 'PASS'} browserfolder: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
