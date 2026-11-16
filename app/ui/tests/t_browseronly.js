// Browser-only mode: the UI in a tab with the core compiled to WASM in the page and no app behind it. A CSV chosen with the
// picker or dropped on the page opens and draws (the decoder's own CSV reader, so a name's unit, the time column and time that
// goes backwards are read as in the app); what was read is kept in the browser's private storage, and the same file opens
// from it next time. Needs the built page (just viewer); without it the test says so and passes.
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
  const fetched = [];
  p.on('request', (r) => fetched.push(new URL(r.url()).pathname.split('/').pop()));
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
  check('it offers no live source, and a folder only where the browser has the picker (audit, R2.2; D-brow-10)', (!(await p.$('#startPage [data-start="folder"]')) === !(await p.evaluate(() => typeof window.showDirectoryPicker === 'function'))) && !(await p.$('#startPage [data-start="live"]')) && !!(await p.$('#startPage [data-start="open"]')) && !/Open a folder/.test(await p.$eval('#explorer', (e) => e.textContent ?? '')));

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

  // what comes after v1 is hidden, not greyed (D-brow-05): layouts, rules and checks, derived channels, several files in a tab,
  // editing events (owner's scope, Nov 15)
  const none = async (sel) => (await p.$$(sel)).length === 0;
  // the start page shows over the restored tabs at launch
  if (await p.$('#startPage.overlay')) await p.click('#startPage.overlay [aria-label="Close the start page"]');
  await until(() => !document.querySelector('#startPage.overlay'));
  check('the rail has no Layouts, Rules & checks or Events (Explorer, Measure and Settings stay; owner, Nov 16)', (await none('[data-rail="layouts"]')) && (await none('[data-rail="checks"]')) && (await none('[data-rail="live"]')) && (await none('[data-rail="events"]')) && !!(await p.$('[data-rail="explorer"]')) && !!(await p.$('[data-rail="measure"]')) && !!(await p.$('[data-rail="settings"]')), await p.$$eval('[data-rail]', (e) => e.map((x) => x.dataset.rail)));
  check('Explorer has no Derive button', await none('#exDerive'));
  await p.click('#explorer .tree .row.chan');
  check('a channel selected offers Add, Chart, Table and Remove, but not Derive', !!(await p.$('#selBar [data-sel="add"]')) && (await none('#selBar [data-sel="derive"]')));
  await p.click('#explorer .tree .row.chan', { button: 'right' });
  const cm = await p.$$eval('.chanmenu .mi', (e) => e.map((x) => x.textContent ?? ''));
  check('a channel\'s right-click menu has no Mark when, Add check or Derive', cm.length > 0 && !cm.some((t) => /Mark when|Add check|Derive|recipe/i.test(t)), cm);
  await p.keyboard.press('Escape');
  await p.click('#explorer .row.file', { button: 'right' });
  const fm = await p.$$eval('.filemenu .mi', (e) => e.map((x) => x.textContent ?? ''));
  check('a file\'s menu has no Open with layout, Compare, Add to this tab, Make primary or Alignment', fm.length > 0 && !fm.some((t) => /layout|Compare|Add to this tab|primary|Alignment/i.test(t)) && fm.some((t) => /Close this file/.test(t)), fm);
  await p.keyboard.press('Escape');
  await p.click('#exMore');
  check('Explorer\'s ⋯ has no Recipes…', !(await p.$$eval('.exmenu .mi', (e) => e.map((x) => x.textContent ?? ''))).some((t) => /Recipes/.test(t)));
  await p.keyboard.press('Escape');
  await p.evaluate(() => window.__tlmSettings.set('leftPanel', 'events'));
  check('a saved Events panel opens as Explorer', await until(() => !document.querySelector('[data-panel="events"]:not([hidden])') && !!document.querySelector('[data-panel="explorer"]:not([hidden]), #explorer')), await p.evaluate(() => window.__tlmSettings?.get?.('leftPanel')));
  check('the bar has no + for layouts and no Event markers button', (await none('#wsTabNew')) && (await none('#b-events')));
  await p.click('#wsTabs .wstab .tm');
  const tm = await p.$$eval('.tabmenu .mi', (e) => e.map((x) => x.textContent ?? ''));
  check('the tab menu keeps Close items only: no Save, Share, Export file, Rename or window', tm.length > 0 && tm.every((t) => /^Close/.test(t)), tm);
  await p.keyboard.press('Escape');
  await p.click('#b-more');
  const mm = await p.$$eval('.moremenu .mi', (e) => e.map((x) => x.textContent ?? ''));
  check('⋯ has no Link views (a tab has one window)', mm.length > 0 && !mm.some((t) => /Link/.test(t)), mm);
  await p.keyboard.press('Escape');
  await p.click('#stack [data-vmenu]');
  const vm = await p.$$eval('.viewmenu .mi', (e) => e.map((x) => x.textContent ?? ''));
  check('a pane\'s view menu has no Checks', vm.length > 0 && !vm.some((t) => /^Checks/.test(t)) && vm.some((t) => /Chart/.test(t)), vm);
  await p.keyboard.press('Escape');
  await p.click('[data-rail="settings"]');
  await p.click('#setResetApp');
  const rs = await p.$$eval('.rrow', (e) => e.map((x) => x.textContent ?? ''));
  check('Reset app lists none of what comes after v1', !rs.some((t) => /Recipes|Rules and checks|Layouts|Your events|Alignments/.test(t)), rs);
  await p.click('.dlg .dclose');
  await p.click('[data-rail="explorer"]');

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
    const w = await until(() => window.__tlmApp.view().t0 !== window.__tlmApp.view().t1) && (await p.evaluate(() => window.__tlmApp.view()));
    check('its time field gives the times: 4,000 records a millisecond apart (the view ends 1 ns after the last)', w && w.t0 === '0' && w.t1 === '3999000001', w);
  }

  // Parquet: its reader is a module of its own, fetched only now; every codec reads the same file (files from tools/testdata/parquet_codecs.py)
  {
    check('the Parquet module was not fetched before a Parquet file was opened', !fetched.includes('tlmbench_parquet.wasm'), fetched);
    for (const codec of ['snappy', 'zstd', 'brotli', 'gzip', 'lz4', 'none']) {
      const name = `codecs-${codec}.parquet`;
      const buffer = readFileSync(join(ROOT, 'crates/web-parquet/tests/fixtures', name));
      await p.evaluate(([name, bytes]) => {
        const dt = new DataTransfer();
        dt.items.add(new File([new Uint8Array(bytes)], name));
        window.dispatchEvent(new DragEvent('dragenter', { dataTransfer: dt, bubbles: true, cancelable: true }));
        window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
      }, [name, [...buffer]]);
      check(`a ${codec}-compressed Parquet file opens`, await until((n) => [...document.querySelectorAll('#wsTabs .wstab')].some((t) => t.textContent?.includes(n)), name), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
      if (codec === 'snappy') {
        check('its numeric columns are channels, the time column and the text column left out, units from the field and the name', await until(() => { const r = [...document.querySelectorAll('#explorer .tree .row.chan')].map((x) => x.textContent ?? ''); return r.length === 3 && /speed/.test(r[0]) && /Temp/.test(r[1]) && /armed/.test(r[2]); }), await p.$$eval('#explorer .tree .row.chan', (r) => r.map((x) => x.textContent)));
        check('read in the Worker', (await p.evaluate(() => window.__tlmParse)) === 'worker');
        const v = await until(() => window.__tlmApp.view().t0 !== window.__tlmApp.view().t1) && (await p.evaluate(() => window.__tlmApp.view()));
        check('its time runs from the first row to the last one that has a time (1,999 ms: the microsecond column read right)', v && v.t0 === '0' && v.t1 === '1999000001', v);
      }
    }
    check('the Parquet module was fetched when one was opened', fetched.includes('tlmbench_parquet.wasm'), fetched);
  }

  // a file the page can't read says so (a binary one opens the descriptor form instead: t_browserdesc)
  await p.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File(['a log, not data\n'], 'notes.log'));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  check('a text file it can’t read says what browser-only mode reads', await until(() => /browser-only mode reads CSV, Parquet and \.tlm files, and binary files a descriptor describes/.test(document.body.textContent ?? '')), await p.evaluate(() => document.querySelector('#wsNote')?.textContent));
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
