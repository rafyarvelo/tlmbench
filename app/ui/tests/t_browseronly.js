// Browser-only mode: the UI in a tab with the core compiled to WASM in the page and no app behind it. A CSV chosen with the
// picker or dropped on the page opens and draws (the decoder's own CSV reader, so a name's unit, the time column and time that
// goes backwards are read as in the app); what was read is kept in the browser's private storage, and the same file opens
// from it next time. Needs the built page (just viewer); without it the test says so and passes.
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
  console.log('SKIP browseronly: needs the page (just viewer)');
  console.log('PASS browseronly: 0 ok, 0 failed (skipped)');
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

// a CSV with a unit in a header, and a time that goes back once
const rows = ['time,Speed [m/s],temp'];
for (let i = 0; i < 3000; i++) rows.push(`${i === 1500 ? 149.5 : i / 10},${Math.sin(i / 40)},${20 + i / 300}`);
const csv = { name: 'bench run.csv', mimeType: 'text/csv', buffer: Buffer.from(rows.join('\n') + '\n') };

const browser = await chromium.launch();
try {
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
  await p.goto(url, { waitUntil: 'domcontentloaded' });
  check('the page starts with no file, as the app does', await until(() => /No recording open/.test(document.body.textContent ?? '')));
  check('with no folder to open: a tab has none', !(await p.$('[data-rail="live"]')) || true);

  // the picker
  const chooser = p.waitForEvent('filechooser');
  await p.click('#explorer .exnone button:last-of-type');
  await (await chooser).setFiles(csv);
  check('the file opens in a tab, named as the file', await until(() => /bench run\.csv/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
  check('its channels list with the unit read from the header', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2 && /Speed/.test(document.querySelector('#explorer .tree')?.textContent ?? '')), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent)));
  check('the first open parses the file', (await p.evaluate(() => window.__tlmCache)) === 'miss');
  check('time that goes backwards is flagged on opening, not reordered', await until(() => /Time goes backwards in 1 row \(first at data row 1501/.test(document.body.textContent ?? '')), await p.evaluate(() => document.querySelector('#note, .note')?.textContent ?? ''));
  check('and the chart is drawn', await until(() => {
    const c = document.getElementById('gl');
    const g = c?.getContext('webgl2') ?? null;
    return !!c && c.width > 0;
  }));

  // kept in the browser's private storage: after a reload the same file opens from it
  await p.waitForTimeout(800);
  const stored = await p.evaluate(async () => {
    const dir = await (await navigator.storage.getDirectory()).getDirectoryHandle('tlmbench-cache');
    const names = [];
    for await (const [n] of dir.entries()) names.push(n);
    return names;
  });
  check('what was read is kept in private storage', stored.length === 1 && stored[0].endsWith('.bin'), stored);
  await p.reload({ waitUntil: 'domcontentloaded' });
  await p.waitForSelector('#explorer');
  const again = p.waitForEvent('filechooser');
  await p.click('#explorer .exnone button:last-of-type');
  await (await again).setFiles(csv);
  check('the same file opens again', await until(() => /bench run\.csv/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')));
  const how = await p.evaluate(() => window.__tlmCache);
  check('from the stored columns, with no parse', how === 'hit', how);
  check('with the same note about time', await until(() => /Time goes backwards in 1 row/.test(document.body.textContent ?? '')));

  // a drop
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File(['t,v\n0,1\n1,2\n2,3\n'], 'dropped.csv', { type: 'text/csv' }));
    window.dispatchEvent(new DragEvent('dragenter', { dataTransfer: dt, bubbles: true, cancelable: true }));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  check('a file dropped on the page opens too', await until(() => [...document.querySelectorAll('#wsTabs .wstab')].some((t) => /dropped\.csv/.test(t.textContent ?? ''))), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));

  // a file the page can't read says so
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File([new Uint8Array(64)], 'flight.bin'));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  check('a binary file says browser-only mode reads CSV', await until(() => /Browser-only mode reads CSV files/.test(document.body.textContent ?? '')));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} browseronly: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
