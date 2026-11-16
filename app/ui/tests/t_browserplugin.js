// Browser-only mode runs WASM decoder plugins (D-brow-09; owner, Nov 16): Settings › Plugins adds the same .wasm the app runs
// (translated in the page by jco) and keeps it in this browser; a file its patterns name opens through it, in a Worker of its
// own; the sandbox gives it the file and nothing else (no file access, clocks at zero) with the app's limits (a loop is
// stopped, memory is capped, a bad column is refused, each saying so); a native plugin is refused as needing the app; Remove
// takes a plugin away. Short limits stand in for the app's 30 s and 1 GiB. Needs the built page (just viewer) and the plugins
// built for wasm32-wasip2 (just plugin-template, or the Rust tests); without them it says so and passes.
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const SITE = join(ROOT, 'app/browser/dist');
const DEMO = join(ROOT, 'plugins/template-decoder/target/wasm32-wasip2/release/my_decoder.wasm');
const HOSTILE = join(ROOT, 'plugins/test-hostile/target/wasm32-wasip2/release/hostile.wasm');
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
if (!existsSync(join(SITE, 'index.html')) || !existsSync(DEMO) || !existsSync(HOSTILE)) {
  console.log('SKIP browserplugin: needs the page (just viewer) and the plugins (just plugin-template)');
  console.log('PASS browserplugin: 0 ok, 0 failed (skipped)');
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

// the template's DEMO format: the magic, a u32 count, then records of u64 µs, f32 volts, i16 status
const demo = (n) => {
  const b = Buffer.alloc(8 + 14 * n);
  b.write('DEMO', 0);
  b.writeUInt32LE(n, 4);
  for (let i = 0; i < n; i++) {
    const o = 8 + 14 * i;
    b.writeBigUInt64LE(BigInt(i * 10_000), o);
    b.writeFloatLE(12 + i / 100, o + 8);
    b.writeInt16LE(i % 3, o + 12);
  }
  return b;
};

const browser = await chromium.launch();
try {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.addInitScript(() => (window.__tlmPluginLimits = { cpuMs: 4000, maxPages: 4096 }));
  const p = await ctx.newPage();
  const errs = [];
  p.on('pageerror', (e) => errs.push(e.message));
  const until = async (fn, arg, ms = 20000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await p.evaluate(fn, arg).catch(() => false)) return true;
      await p.waitForTimeout(80);
    }
    return false;
  };
  const tabs = () => p.$$eval('#wsTabs .wstab .nm', (t) => t.map((x) => x.textContent));
  const drop = (name, bytes) =>
    p.evaluate(
      ([name, bytes]) => {
        const dt = new DataTransfer();
        dt.items.add(new File([new Uint8Array(bytes)], name));
        window.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }));
      },
      [name, [...bytes]],
    );
  const noteText = () => p.evaluate(() => document.querySelector('#wsNote')?.textContent ?? '');
  const add = async (path, name) => {
    const chooser = p.waitForEvent('filechooser');
    await p.click('#pluginsAdd');
    await (await chooser).setFiles({ name, mimeType: 'application/wasm', buffer: readFileSync(path) });
  };
  await p.goto(url, { waitUntil: 'domcontentloaded' });
  await until(() => !!window.__tlmApp && typeof window.__tlmBrowserOnly?.add === 'function');
  await p.click('[data-rail="settings"]');
  check('Settings › Plugins is there, with Add plugin… and no Open folder', await until(() => !!document.querySelector('#pluginSet #pluginsAdd')) && !(await p.$('#pluginsOpen')) && /Native plugins need the app/.test(await p.$eval('#pluginSet', (e) => e.textContent)));

  await add(DEMO, 'my_decoder.wasm');
  check('Add plugin… translates it in the page and lists it, as WASM, sandboxed', await until(() => /demo/.test(document.querySelector('#pluginSet [data-plugin="my_decoder.wasm"]')?.textContent ?? '') && /WASM, sandboxed/.test(document.querySelector('#pluginSet [data-plugin="my_decoder.wasm"]')?.textContent ?? ''), null, 60000), await p.$eval('#pluginSet', (e) => e.textContent));
  check('and keeps it in this browser', await p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.plugins') ?? '[]').some((k) => k.about.name === 'demo' && k.about.files.includes('*.demo'))));
  await add(join(ROOT, 'plugins/template-decoder/Cargo.toml'), 'decoder.dylib');
  check('a native plugin is refused: it needs the app', await until(() => /Native plugins need the app/.test(document.querySelector('#pluginNote')?.textContent ?? '')), await p.evaluate(() => document.querySelector('#pluginNote')?.textContent));
  await p.click('[data-rail="explorer"]');

  // a file the plugin's pattern names
  await drop('bench.demo', demo(500));
  check('a file its pattern names opens through the plugin', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === 'bench.demo'), null, 60000), [await tabs(), await noteText()]);
  check('decoded in the plugin\'s Worker', (await p.evaluate(() => window.__tlmParse)) === 'plugin', await p.evaluate(() => window.__tlmParse));
  check('with the plugin\'s channels and units', await until(() => /volts/.test(document.querySelector('#explorer .tree')?.textContent ?? '') && /status/.test(document.querySelector('#explorer .tree')?.textContent ?? '')), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent.trim())));
  check('and its time: 500 records 10 ms apart', await until(() => /4\.99|00:00:04\.9/.test(document.querySelector('#rangeTxt')?.textContent ?? '')), await p.$eval('#rangeTxt', (e) => e.textContent));

  // the sandbox and its limits, through the test plugin that misbehaves on purpose (it claims files starting with 4 capitals)
  await p.click('[data-rail="settings"]');
  await add(HOSTILE, 'hostile.wasm');
  await until(() => !!document.querySelector('#pluginSet [data-plugin="hostile.wasm"]'), null, 60000);
  await p.click('[data-rail="explorer"]');
  const opens = async (name, head) => {
    await p.click('#wsNote [aria-label="Dismiss"]').catch(() => {});
    await drop(name, Buffer.from(head + '----'));
  };
  await opens('a.dat', 'FILE');
  check('it can\'t read a file of the machine\'s', await until(() => /no file access/.test(document.querySelector('#wsNote')?.textContent ?? ''), null, 60000), await noteText());
  await opens('b.dat', 'TIME');
  check('its clock says zero', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === 'b.dat'), null, 60000), [await tabs(), await noteText()]);
  // the value it reported: the page's core answers what the app's values_at does (file ids count up from 0 here)
  const tv = await p.evaluate(async () => {
    // b.dat is the last file that opened (the ones after it failed): the highest id that answers
    let last = null;
    for (let id = 0; id < 16; id++) last = (await window.__TAURI_INTERNALS__.invoke('values_at', { file: id, channels: [0], t: '0' }).catch(() => null)) ?? last;
    return last;
  });
  check('(the seconds it read from the clock are 0)', tv?.[0]?.value === 0, tv);
  await opens('c.dat', 'BADC');
  check('a column of the wrong size is refused, saying so', await until(() => /column of 8 bytes for 2 rows/.test(document.querySelector('#wsNote')?.textContent ?? ''), null, 60000), await noteText());
  await opens('d.dat', 'MEMO');
  // a 2 GB allocation, which a browser would grant past 1 GiB: under the cap it fails inside the plugin (Rust aborts)
  check('memory past the cap fails inside the plugin and says so', await until(() => /d\.dat: Error: hostile: .*unreachable/.test(document.querySelector('#wsNote')?.textContent ?? ''), null, 60000), await noteText());
  const t0 = Date.now();
  await opens('e.dat', 'LOOP');
  check('a loop is stopped at the CPU limit, saying so', await until(() => /ran for more than 4 s and was stopped/.test(document.querySelector('#wsNote')?.textContent ?? ''), null, 60000), await noteText());
  check('(at the limit, not before)', Date.now() - t0 >= 3900, Date.now() - t0);

  // Remove
  await p.click('[data-rail="settings"]');
  await p.click('#pluginSet [data-plugremove="hostile.wasm"]');
  check('Remove takes it away', await until(() => !document.querySelector('#pluginSet [data-plugin="hostile.wasm"]') && !!document.querySelector('#pluginSet [data-plugin="my_decoder.wasm"]')));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} browserplugin: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
