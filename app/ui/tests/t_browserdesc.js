// Browser-only mode reads binary files through descriptors (D-brow-08): descriptors are kept in this browser and found by their
// file name patterns; a packet file (framing, checksums, Zstandard payloads, by the app's own packets_core.rs in WASM) opens, a
// file with several packet types as a tab per type; a binary file nothing names opens the descriptor form, whose preview and
// Find frames run in the page's core, with no Share (a tab never writes to the folder); Open there keeps the descriptor and
// opens the file, and the next file the pattern names opens with no form. A reload brings a packet type back from storage.
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
  console.log('SKIP browserdesc: needs the page (just viewer)');
  console.log('PASS browserdesc: 0 ok, 0 failed (skipped)');
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

// packets: sync EB90, a u16 payload length, a type byte and a pad, the payload, a sum8 checksum
const packet = (kind, payload, sum = true) => {
  const head = Buffer.from([0xeb, 0x90, payload.length & 255, payload.length >> 8, kind, 0]);
  const body = Buffer.concat([head, Buffer.from(payload)]);
  return sum ? Buffer.concat([body, Buffer.from([body.reduce((a, b) => (a + b) & 255, 0)])]) : body;
};
// Zstandard frames (the zstd tool) of records (t u16 ms, v i16): (0, 10), (10, 20), (20, 30); and (30, 40)
const ZST3 = [0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x0c, 0x61, 0, 0, 0, 0, 0x0a, 0, 0x0a, 0, 0x14, 0, 0x14, 0, 0x1e, 0, 0x59, 0x7c, 0x80, 0x63];
const ZST1 = [0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x04, 0x21, 0, 0, 0x1e, 0, 0x28, 0, 0x23, 0xfc, 0x9d, 0xa9];
const zfile = { name: 'run 1.zpk', mimeType: 'application/octet-stream', buffer: Buffer.concat([packet(0, ZST3), packet(0, ZST1)]) };
const ZDESC = { schema: 1, name: 'Zstd packets', files: ['*.zpk'], endian: 'little', header_size: 0, record_size: 4, time: { source: 'field', field: 't', unit: 'ms' }, packets: { sync: 'EB90', header_size: 6, length: { offset: 2, type: 'u16' }, checksum: 'sum8', compression: 'zstd' }, fields: [{ name: 't', type: 'u16', offset: 6 }, { name: 'v', type: 'i16', offset: 8, unit: 'V' }] };
// two packet types by the type byte: gps (t u16 ms, alt i16) and imu (ax i16)
const tfile = { name: 'two.pkt', mimeType: 'application/octet-stream', buffer: Buffer.concat([packet(1, [0, 0, 5, 0], false), packet(2, [7, 0, 8, 0], false), packet(1, [100, 0, 6, 0], false)]) };
const TDESC = { schema: 1, name: 'Two types', files: ['*.pkt'], endian: 'little', header_size: 0, packets: { sync: 'EB90', header_size: 6, length: { offset: 2, type: 'u16' }, id: { offset: 4, type: 'u8' } }, packet_types: [{ id: 1, name: 'gps', record_size: 4, time: { source: 'field', field: 't', unit: 'ms' }, fields: [{ name: 't', type: 'u16', offset: 6 }, { name: 'alt', type: 'i16', offset: 8 }] }, { id: 2, name: 'imu', record_size: 2, time: { source: 'index' }, fields: [{ name: 'ax', type: 'i16', offset: 6 }] }] };
// binary files nothing names: u16 values, with bytes no text has
const raw = (n) => ({ name: `sensor_${n}.bin`, mimeType: 'application/octet-stream', buffer: Buffer.from(Uint16Array.from({ length: 500 }, (_, i) => (i * 37 + n) & 0xffff).buffer).fill(0, 0, 2) });

const browser = await chromium.launch();
try {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.addInitScript((list) => {
    if (!sessionStorage.getItem('seeded')) {
      sessionStorage.setItem('seeded', '1');
      localStorage.setItem('tlmbench.descriptors', JSON.stringify(list));
    }
  }, [{ id: 'zstd-packets', descriptor: ZDESC }, { id: 'two-types', descriptor: TDESC }]);
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
  const tabs = () => p.$$eval('#wsTabs .wstab .nm', (t) => t.map((x) => x.textContent));
  // a file dropped on the page (as the app takes a Finder drop)
  const choose = (file) =>
    p.evaluate(
      ([name, bytes]) => {
        const dt = new DataTransfer();
        dt.items.add(new File([new Uint8Array(bytes)], name));
        window.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }));
      },
      [file.name, [...file.buffer]],
    );
  await p.goto(url, { waitUntil: 'domcontentloaded' });
  // the page takes drops once browser-only mode is installed (it sets __tlmBrowserOnly to its handle then)
  await until(() => /No recording open/.test(document.body.textContent ?? '') && typeof window.__tlmBrowserOnly?.add === 'function');

  // a Zstandard packet file, by the descriptor whose pattern names it
  await choose(zfile);
  check('a packet file opens through the descriptor that names it', await until(() => /run 1\.zpk/.test(document.querySelector('#wsTabs')?.textContent ?? '')), await tabs());
  check('its channel is listed, with its unit', await until(() => [...document.querySelectorAll('#explorer .tree .row.chan')].map((r) => r.textContent.trim()).join() === 'vV'), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent.trim())));
  check('read in the Worker', (await p.evaluate(() => window.__tlmParse)) === 'worker', await p.evaluate(() => window.__tlmParse));
  check('four records from two Zstandard payloads: the time span is 30 ms', await until(() => /30(\.0)? ms|0\.03/.test(document.querySelector('#rangeTxt')?.textContent ?? '')), await p.$eval('#rangeTxt', (e) => e.textContent));

  // two packet types: a tab each
  await choose(tfile);
  check('a file with two packet types opens a tab per type', await until(() => ['two.pkt · gps', 'two.pkt · imu'].every((n) => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === n))), await tabs());

  // a binary file nothing names: the descriptor form
  await choose(raw(3));
  check('a binary file no descriptor names opens the descriptor form', await until(() => !!document.querySelector('#dscDlg')), await tabs());
  check('with a preview from the page\'s core', await until(() => document.querySelectorAll('#dscTable tbody tr').length > 0), await p.evaluate(() => document.querySelector('#dscErr')?.textContent ?? null));
  check('and no Share (a browser tab never writes to the folder)', !(await p.$('#dscShare')));
  await p.click('#dscFind');
  check('Find frames answers from the core (none in these bytes)', await until(() => !!document.querySelector('#dscFound')), await p.evaluate(() => document.querySelector('#dscFound')?.textContent));
  await p.click('#dscOpen');
  check('Open keeps the descriptor in this browser and opens the file', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === 'sensor_3.bin')) && (await p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.descriptors')).length)) === 3, await tabs());
  const pattern = await p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.descriptors')).at(-1).descriptor.files);
  check('named by a pattern for its siblings', pattern.some((g) => /\*/.test(g)), pattern);
  await choose(raw(4));
  check('the next file the pattern names opens with no form', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === 'sensor_4.bin')) && !(await p.$('#dscDlg')), await tabs());

  // a reload brings a packet type back from the stored columns
  await p.waitForTimeout(800);
  await p.reload({ waitUntil: 'domcontentloaded' });
  check('after a reload the packet types are back, named', await until(() => ['two.pkt · gps', 'two.pkt · imu'].every((n) => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === n)), null, 15000), await tabs());
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} browserdesc: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
