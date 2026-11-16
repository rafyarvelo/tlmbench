// Browser-only mode opens a folder where the browser can (D-brow-10): `showDirectoryPicker` asked for read access only, the
// folder listed read-only in Explorer (dot folders left out, files the app can't read marked), a file in it opening from its
// handle; where the browser has no picker the start page says so once and offers Open recording…. The test gives the page a
// real directory handle (the browser's private storage) in place of the native picker, which headless Chromium can't drive.
// Needs the built page (just viewer); without it the test says so and passes.
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
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

const PLUGIN = join(ROOT, 'plugins/template-decoder/target/wasm32-wasip2/release/my_decoder.wasm');
// the template's DEMO format: the magic, a u32 count, then records of u64 µs, f32 volts, i16 status
const DEMO_FILE = (() => {
  const b = Buffer.alloc(8 + 14 * 50);
  b.write('DEMO', 0);
  b.writeUInt32LE(50, 4);
  for (let i = 0; i < 50; i++) (b.writeBigUInt64LE(BigInt(i * 1000), 8 + 14 * i), b.writeFloatLE(i, 16 + 14 * i));
  return b;
})();
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
        // shared configuration (D-brow-10): a category, a widget, and a descriptor for *.esc files
        await put(hid, 'categories.toml', 'schema = 1\n[[category]]\nid = "warn"\nname = "Warnings"\ncolor = "#f80"\n');
        const wd = await (await hid.getDirectoryHandle('widgets', { create: true })).getDirectoryHandle('dial', { create: true });
        await put(wd, 'widget.toml', 'schema = 1\nname = "Dial"\nkind = "svg"\nentry = "dial.svg"\n');
        await put(wd, 'dial.svg', '<svg viewBox="0 0 10 10"><text>{{value}}</text></svg>');
        await put(await hid.getDirectoryHandle('decoders', { create: true }), 'esc.toml', 'schema = 1\nname = "ESC"\nfiles = ["*.esc"]\nrecord_size = 4\n[time]\nsource = "rate"\nrate_hz = 100\n[[fields]]\nname = "rpm"\ntype = "u16"\noffset = 0\n[[fields]]\nname = "amps"\ntype = "i16"\noffset = 2\nunit = "A"\n');
        const bin = (n, salt) => Uint8Array.from({ length: n }, (_, i) => (i === 0 ? 0 : (i * 37 + salt) & 255));
        await put(dir, 'bench.esc', bin(800, 3));
        await put(dir, 'blob.bin', bin(400, 9));
        // its plugins (D-brow-09): the template decoder (when built) and a native library, which needs the app
        if (window.__plugin) {
          const pd = await hid.getDirectoryHandle('plugins', { create: true });
          await put(pd, 'my_decoder.wasm', new Uint8Array(window.__plugin));
          await put(pd, 'fast.dylib', 'native');
          await put(dir, 'bench.demo', new Uint8Array(window.__demo));
        }
        await dir.getDirectoryHandle('empty', { create: true });
        return dir;
      };
    });
    await p.goto(url, { waitUntil: 'domcontentloaded' });
    await p.evaluate((c) => (window.__csv = c), CSV);
    if (existsSync(PLUGIN)) await p.evaluate(([w, d]) => ((window.__plugin = w), (window.__demo = d)), [[...readFileSync(PLUGIN)], [...DEMO_FILE]]);
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
      return names.sort().join() === (window.__plugin ? '.tlmbench,bench.demo,bench.esc,blob.bin,day 1,empty,notes.md,readme.pdf' : '.tlmbench,bench.esc,blob.bin,day 1,empty,notes.md,readme.pdf');
    }));
    check('the folder menu has no Share… and no Recipes… (a tab writes nothing, recipes come after v1)', await (async () => {
      await p.click('#exFolder');
      const t = await p.$$eval('.exmenu .mi', (e) => e.map((x) => x.textContent));
      await p.keyboard.press('Escape');
      return !t.some((x) => /Share|Recipes/.test(x)) && t.some((x) => /Refresh/.test(x));
    })(), await p.$$eval('.exmenu .mi', (e) => e.map((x) => x.textContent)).catch(() => null));

    // the folder's .tlmbench/ (D-brow-10): its categories and widgets reach the app; its descriptor names bench.esc
    const sc = await p.evaluate(() => window.__tlmApp.sharedConfig());
    check('the folder\'s .tlmbench/ is read: its categories and widgets', sc.dir === 'local-folder://runs/.tlmbench' && sc.categories[0]?.name === 'Warnings' && sc.widgets.some((w) => w.name === 'Dial' && w.svg), sc);
    check('a file the folder\'s descriptor names lists as openable, with no "no descriptor"', await until(() => !!document.querySelector('#explorer .row.file[data-path="local://runs/bench.esc"]')) && !/no descriptor/.test(await p.$eval('#explorer .row.file[data-path="local://runs/bench.esc"]', (e) => e.textContent)));
    check('a binary file none names says "no descriptor"', /no descriptor/.test(await p.$eval('#explorer .row.file[data-path="local://runs/blob.bin"]', (e) => e.textContent).catch(() => '')));
    await p.dblclick('#explorer .row.file[data-path="local://runs/bench.esc"]');
    check('it opens through the folder\'s descriptor', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === 'bench.esc')) && (await until(() => [...document.querySelectorAll('#explorer .tree .row.chan')].map((r) => r.textContent.trim().replace(/pane \d+/, '')).join() === 'rpm,ampsA')), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent.trim())));
    await p.dblclick('#explorer .row.file[data-path="local://runs/blob.bin"]');
    check('the descriptor form lists the folder\'s descriptor as shared', await until(() => [...document.querySelectorAll('#dscPick option')].some((o) => /ESC.*\(shared\)/.test(o.textContent))), await p.$$eval('#dscPick option', (o) => o.map((x) => x.textContent)).catch(() => null));
    await p.keyboard.press('Escape');
    check('the descriptor stays in the folder: nothing kept in this browser for it', await p.evaluate(() => !localStorage.getItem('tlmbench.descriptors')));

    // the folder's plugins: waiting for your OK, a native one needing the app; trusted, a file one names opens through it
    if (existsSync(PLUGIN)) {
      await p.click('[data-rail="settings"]');
      check('the folder\'s plugin waits for your OK', await until(() => /waiting for your OK/.test(document.querySelector('#pluginSet')?.textContent ?? '') && !!document.querySelector('#pluginSet [data-plugtrust]'), null, 60000), await p.$eval('#pluginSet', (e) => e.textContent).catch(() => null));
      check('its native plugin is listed as needing the app', /Native plugins need the app/.test(await p.$eval('#pluginSet [data-plugin="fast.dylib"]', (e) => e.textContent).catch(() => '')));
      await p.click('#pluginSet [data-plugtrust]');
      check('trusted, it loads', await until(() => /trusted/.test(document.querySelector('#pluginSet')?.textContent ?? '') && /demo/.test(document.querySelector('#pluginSet [data-plugin="my_decoder.wasm"]')?.textContent ?? '')));
      await p.click('[data-rail="explorer"]');
      await p.click('#exFolder');
      await p.click('.exmenu [data-ex="refresh"]');
      check('a file its pattern names lists as openable', await until(() => !!document.querySelector('#explorer .row.file[data-path="local://runs/bench.demo"]')));
      await p.dblclick('#explorer .row.file[data-path="local://runs/bench.demo"]');
      check('and opens through the folder\'s plugin', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === 'bench.demo'), null, 60000) && (await p.evaluate(() => window.__tlmParse)) === 'plugin', await p.evaluate(() => [window.__tlmParse, document.querySelector('#wsNote')?.textContent]));
    } else console.log('  (skipped the folder\'s plugins: just plugin-template builds the template)');
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
