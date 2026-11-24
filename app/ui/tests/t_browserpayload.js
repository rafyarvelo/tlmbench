// Browser-only mode runs payload plugins (D-ingest-09's last part): a descriptor kept in this browser names the template's
// payload plugin for a packet type (each packet's payload after framing goes to the plugin, its rows load as channels) and
// for a field (its bytes of every record go to the plugin, its values join the descriptor's channels). The page's core
// lists the plugin's work, the plugin runs in its own Worker, then the core reads the file. Without the plugin added it
// says to add it. Needs the built page (just viewer) and the template built for wasm32-wasip2 (just plugin-template).
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const SITE = join(ROOT, 'app/browser/dist');
const PAYLOAD = join(ROOT, 'plugins/template-payload/target/wasm32-wasip2/release/my_payload.wasm');
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
if (!existsSync(join(SITE, 'index.html')) || !existsSync(PAYLOAD)) {
  console.log('SKIP browserpayload: needs the page (just viewer) and the payload template (just plugin-template)');
  console.log('PASS browserpayload: 0 ok, 0 failed (skipped)');
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

// packets: sync EB90, a u16 payload length, a type byte and a pad, then the payload
const packet = (kind, payload) => Buffer.concat([Buffer.from([0xeb, 0x90, payload.length & 255, payload.length >> 8, kind, 0]), Buffer.from(payload)]);
// type 1, the template's status packet: u64 µs, a u16 word (mode in bits 0 to 3, fault in bit 15), an i16 temperature in tenths
const status = (us, mode, fault, tenths) => {
  const b = Buffer.alloc(12);
  b.writeBigUInt64LE(BigInt(us), 0);
  b.writeUInt16LE(mode | (fault ? 0x8000 : 0), 8);
  b.writeInt16LE(tenths, 10);
  return b;
};
// type 2: ax i16, then a 3-byte big-endian integer the plugin reads (a field with `plugin` and `size`)
const imu = (ax, packed) => {
  const b = Buffer.alloc(5);
  b.writeInt16LE(ax, 0);
  b.writeIntBE(packed, 2, 3);
  return b;
};
const file = Buffer.concat([
  packet(1, status(0, 2, false, 215)),
  packet(2, imu(7, 1000)),
  packet(1, status(500_000, 3, true, 230)),
  packet(2, imu(8, -2)),
  packet(1, status(1_000_000, 3, false, 241)),
]);
const PDESC = {
  schema: 1, name: 'Payload demo', files: ['*.ppk'], endian: 'little', header_size: 0,
  packets: { sync: 'EB90', header_size: 6, length: { offset: 2, type: 'u16' }, id: { offset: 4, type: 'u8' } },
  packet_types: [
    { id: 1, name: 'status', plugin: 'demo-payload' },
    { id: 2, name: 'imu', record_size: 5, time: { source: 'index' }, fields: [{ name: 'ax', type: 'i16', offset: 6 }, { name: 'packed', type: 'f64', offset: 8, size: 3, plugin: 'demo-payload', params: { scale: 0.5 } }] },
  ],
};

const browser = await chromium.launch();
try {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.addInitScript((d) => {
    if (!sessionStorage.getItem('seeded')) {
      sessionStorage.setItem('seeded', '1');
      localStorage.setItem('tlmbench.descriptors', JSON.stringify([{ id: 'payload-demo', descriptor: d }]));
    }
  }, PDESC);
  await ctx.addInitScript(() => (window.__tlmPluginLimits = { cpuMs: 20000, maxPages: 4096 }));
  const p = await ctx.newPage();
  const errs = [];
  p.on('pageerror', (e) => errs.push(e.message));
  const until = async (fn, arg, ms = 30000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await p.evaluate(fn, arg).catch(() => false)) return true;
      await p.waitForTimeout(80);
    }
    return false;
  };
  const tabs = () => p.$$eval('#wsTabs .wstab .nm', (t) => t.map((x) => x.textContent));
  const noteText = () => p.evaluate(() => document.querySelector('#wsNote')?.textContent ?? '');
  const drop = (name, bytes) =>
    p.evaluate(
      ([name, bytes]) => {
        const dt = new DataTransfer();
        dt.items.add(new File([new Uint8Array(bytes)], name));
        window.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }));
      },
      [name, [...bytes]],
    );
  const chans = () => p.$$eval('#explorer .row.chan .nm', (r) => r.map((x) => x.textContent));
  const values = (file, t) =>
    p.evaluate(([file, t]) => window.__TAURI_INTERNALS__.invoke('values_at', { file, channels: [0, 1, 2], t }).catch((e) => String(e)), [file, t]);
  await p.goto(url, { waitUntil: 'domcontentloaded' });
  // the page's core (WASM) can take a while to start on a loaded runner: wait for it, and say so if it never does, rather
  // than dropping the file before anything listens (CI, Nov 22: the drop was lost and the next check failed instead)
  check('the page and its core start', await until(() => !!window.__tlmApp?.dropReady?.() && typeof window.__tlmBrowserOnly?.add === 'function', null, 90000));

  // without the plugin: it says to add it
  await drop('bench.ppk', file);
  check('without the plugin, opening says to add it', await until(() => /payload plugin "demo-payload": add it in Settings › Plugins/.test(document.querySelector('#wsNote')?.textContent ?? '')), [await tabs(), await noteText()]);
  await p.click('#wsNote [aria-label="Dismiss"]').catch(() => {});

  // add it, as any WASM plugin
  await p.click('[data-rail="settings"]');
  await until(() => !!document.querySelector('#pluginSet #pluginsAdd'));
  const chooser = p.waitForEvent('filechooser');
  await p.click('#pluginsAdd');
  await (await chooser).setFiles({ name: 'my_payload.wasm', mimeType: 'application/wasm', buffer: readFileSync(PAYLOAD) });
  check('Settings lists the payload plugin', await until(() => /demo-payload/.test(document.querySelector('#pluginSet [data-plugin="my_payload.wasm"]')?.textContent ?? ''), null, 60000), await p.$eval('#pluginSet', (e) => e.textContent));
  await p.click('[data-rail="explorer"]');

  await drop('run.ppk', file);
  check('a file with a plugin packet type and a plugin field opens a tab per type', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].filter((x) => /^run\.ppk · /.test(x.textContent)).length === 2, null, 60000), [await tabs(), await noteText()]);
  check('read through the payload plugin', (await p.evaluate(() => window.__tlmParse)) === 'payload', await p.evaluate(() => window.__tlmParse));
  const names = await tabs();
  const statusTab = names.findIndex((n) => n === 'run.ppk · status');
  await p.click(`#wsTabs .wstab:nth-child(${statusTab + 1})`);
  check("the plugin type's channels are the plugin's (mode, fault, temp)", await until(() => [...document.querySelectorAll('#explorer .row.chan .nm')].map((x) => x.textContent).join() === 'mode,fault,temp'), await chans());
  check('its time is the plugin\'s: three rows over 1 s', await until(() => /00:00:01\.0|1\.000/.test(document.querySelector('#rangeTxt')?.textContent ?? '')), await p.$eval('#rangeTxt', (e) => e.textContent));
  const imuTab = names.findIndex((n) => n === 'run.ppk · imu');
  await p.click(`#wsTabs .wstab:nth-child(${imuTab + 1})`);
  check("the field's plugin values join the descriptor's channels (ax, packed)", await until(() => [...document.querySelectorAll('#explorer .row.chan .nm')].map((x) => x.textContent).join() === 'ax,packed'), await chans());
  // the values the page's core answers: the packed field is the plugin's (1000 and -2, times the descriptor's scale 0.5)
  const got = [];
  for (let id = 0; id < 8; id++) {
    const v0 = await values(id, '0');
    const v1 = await values(id, '1000000000');
    if (Array.isArray(v0) && v0.length === 2) got.push([v0.map((x) => x.value), v1.map((x) => x.value)]);
  }
  const imuVals = got.find(([a]) => a[0] === 7);
  check('packed reads 500 then -1 (the plugin scaled them)', imuVals && imuVals[0][1] === 500 && imuVals[1][1] === -1, got);
  const statusVals = [];
  for (let id = 0; id < 8; id++) {
    const v = await values(id, String(500_000_000));
    if (Array.isArray(v) && v.length === 3) statusVals.push(v.map((x) => x.value));
  }
  check('the status row at 0.5 s: mode 3, fault 1, temp 23 (tenths × 0.1)', statusVals.some((v) => v[0] === 3 && v[1] === 1 && Math.abs(v[2] - 23) < 1e-4), statusVals);
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} browserpayload: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
