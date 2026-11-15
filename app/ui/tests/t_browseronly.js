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

// a tlmbench reference file (.tlm): the header holds the descriptor; 8-byte records: u32 time in us, i16 in mV, u8 flag, a pad
function makeTlm(n) {
  const toml = `schema = 1
name = "bench"
record_size = 8
[time]
source = "field"
field = "t"
unit = "us"
[[fields]]
name = "t"
type = "u32"
offset = 0
[[fields]]
name = "volts"
type = "i16"
offset = 4
unit = "V"
scale = 0.001
[[fields]]
name = "armed"
type = "u8"
offset = 6
`;
  const headerLen = Math.ceil((24 + Buffer.byteLength(toml)) / 64) * 64;
  const b = Buffer.alloc(headerLen + n * 8);
  b.write('TLMBENCH', 0);
  b.writeUInt16LE(1, 8);
  b.writeUInt32LE(headerLen, 12);
  b.writeUInt32LE(Buffer.byteLength(toml), 16);
  b.write(toml, 24);
  for (let i = 0; i < n; i++) {
    const o = headerLen + i * 8;
    b.writeUInt32LE(i * 1000, o);
    b.writeInt16LE(Math.round(Math.sin(i / 50) * 3000), o + 4);
    b.writeUInt8(i % 200 < 100 ? 1 : 0, o + 6);
  }
  return b;
}
const tlm = { name: 'flight.tlm', mimeType: 'application/octet-stream', buffer: makeTlm(4000) };

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
  check('it offers no folder and no live source: a tab has neither (audit, R2.2)', !(await p.$('#startPage [data-start="folder"]')) && !(await p.$('#startPage [data-start="live"]')) && !!(await p.$('#startPage [data-start="open"]')) && !/Open a folder/.test(await p.$eval('#explorer', (e) => e.textContent ?? '')));

  // the picker
  const chooser = p.waitForEvent('filechooser');
  await p.click('#explorer .exnone button:last-of-type');
  await (await chooser).setFiles(csv);
  check('the file opens in a tab, named as the file', await until(() => /bench run\.csv/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
  check('its channels list with the unit read from the header', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2 && /Speed/.test(document.querySelector('#explorer .tree')?.textContent ?? '')), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent)));
  check('the first open parses the file', (await p.evaluate(() => window.__tlmCache)) === 'miss');
  check('in a Worker, not on the page\'s thread (R2.2)', (await p.evaluate(() => window.__tlmParse)) === 'worker', await p.evaluate(() => window.__tlmParse));
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
  check('the file is back after the reload, with no picker (R2.2)', await until(() => /bench run\.csv/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? ''), null, 15000), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
  const how = await p.evaluate(() => window.__tlmCache);
  check('from the stored columns, with no parse', how === 'restored', how);
  check('with its channels listed again', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2 && /Speed/.test(document.querySelector('#explorer .tree')?.textContent ?? '')));

  // a big file: the page keeps answering while it is read (frames keep coming), and it opens
  await p.evaluate(() => {
    window.__frames = 0;
    const tick = () => ((window.__frames += 1), requestAnimationFrame(tick));
    requestAnimationFrame(tick);
    const rows = ['time,a,b,c'];
    for (let i = 0; i < 600000; i++) rows.push(`${i / 1000},${Math.sin(i / 500)},${i % 97},${(i * 7) % 1000}`);
    const dt = new DataTransfer();
    dt.items.add(new File([rows.join('\n') + '\n'], 'big.csv', { type: 'text/csv' }));
    window.dispatchEvent(new DragEvent('dragenter', { dataTransfer: dt, bubbles: true, cancelable: true }));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  const f0 = await p.evaluate(() => window.__frames);
  check('a big CSV opens', await until(() => [...document.querySelectorAll('#wsTabs .wstab')].some((t) => /big\.csv/.test(t.textContent ?? '')), null, 60000));
  const f1 = await p.evaluate(() => window.__frames);
  check('and the page kept drawing frames while it was parsed', f1 - f0 >= 5, [f0, f1]);

  // a drop
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File(['t,v\n0,1\n1,2\n2,3\n'], 'dropped.csv', { type: 'text/csv' }));
    window.dispatchEvent(new DragEvent('dragenter', { dataTransfer: dt, bubbles: true, cancelable: true }));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  check('a file dropped on the page opens too', await until(() => [...document.querySelectorAll('#wsTabs .wstab')].some((t) => /dropped\.csv/.test(t.textContent ?? ''))), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));

  // a .tlm: the decoder's own header and descriptor reader, in the page
  {
    const chooser2 = p.waitForEvent('filechooser');
    await p.evaluate(() => document.querySelector('#b-more')?.click());
    await p.click('.moremenu .mi:has-text("Open recording")');
    await (await chooser2).setFiles(tlm);
    check('a .tlm opens in a tab', await until(() => [...document.querySelectorAll('#wsTabs .wstab')].some((t) => /flight\.tlm/.test(t.textContent ?? ''))), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
    check('its fields list as channels, the time field left out, with the unit from its descriptor', await until(() => { const r = [...document.querySelectorAll('#explorer .tree .row.chan')].map((x) => x.textContent ?? ''); return r.length === 2 && /volts/.test(r[0]) && /armed/.test(r[1]); }), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent)));
    check('read in the Worker', (await p.evaluate(() => window.__tlmParse)) === 'worker');
    check('with all 4000 samples', await until(() => /4[,\s\u202f]?000/.test(document.body.textContent ?? '')));
  }

  // a file the page can't read says so
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File([new Uint8Array(64)], 'flight.bin'));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  check('a binary file it can’t read says what browser-only mode reads', await until(() => /Browser-only mode reads CSV and \.tlm files/.test(document.body.textContent ?? '')));
  // the viewer's rule (sheet R12) holds here too: what a tab can't do is left out, not a button that does nothing
  check('no Live in the rail (a tab has no live sources)', !(await p.$('[data-rail="live"]')));
  await p.click('[data-rail="settings"]');
  check('Settings leaves out what needs the app: browser access, plugins, live recording, shared configuration', await p.evaluate(() => !document.querySelector('#browserSet, #pluginSet, #liveSet, #sharedSet')));
  check('and keeps the cache, which a tab has', await p.evaluate(() => !!document.querySelector('#settings .cache, .cache')));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} browseronly: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
