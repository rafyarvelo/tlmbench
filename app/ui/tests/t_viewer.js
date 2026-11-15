// The viewer of an HTML export, from a plain static file server (no server code): `tlmbench export html` writes the folder
// (a layout, a check set's summary), a headless browser opens it. The UI is the app's over the core compiled to WASM:
// the layout's panes draw, channels list, the checks summary shows with the violations marked, and nothing edits.
// Needs the CLI (cargo build -p tlmbench) and the built viewer (just viewer); without either the test says so and passes.
import { spawnSync } from 'node:child_process';
import { createReadStream, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const CLI = [join(ROOT, 'target/release/tlmbench'), join(ROOT, 'target/debug/tlmbench')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const VIEWER = join(ROOT, 'app/viewer/dist');
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');

if (!CLI || !existsSync(join(VIEWER, 'index.html')) || !existsSync(join(VIEWER, 'tlmbench_web.wasm'))) {
  console.log('SKIP viewer: needs the CLI (cargo build -p tlmbench) and the viewer (just viewer)');
  console.log('PASS viewer: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const work = mkdtempSync(join(tmpdir(), 'tlm-viewer-'));
const csv = join(work, 'bench.csv');
writeFileSync(csv, 'time,rpm,temp\n' + Array.from({ length: 20000 }, (_, i) => `${i / 100},${Math.sin(i / 50)},${20 + i / 300}`).join('\n') + '\n');
mkdirSync(join(work, 'cfg/checks'), { recursive: true });
writeFileSync(join(work, 'cfg/checks/t.toml'), 'schema = 1\nname = "Test limits"\nruns_on_any_file = false\n\n[[check]]\nname = "RPM under 0.9"\nkind = "check"\ngroup = "Limits"\nexpect = "rpm < 0.9"\nseverity = "fail"\ngrace = 0\nsettle = 0\nenabled = true\n');
writeFileSync(join(work, 'layout.json'), JSON.stringify({ schema: 1, name: 'Two panes', panes: [{ channels: ['rpm'], view: 'chart' }, { channels: ['temp'], view: 'chart' }] }));
let site = join(work, 'site');
const r = spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'layout.json'), '--config', join(work, 'cfg'), '--checks', 'Test limits', '--cache', join(work, 'cache'), '--out', site, '--viewer', VIEWER], { encoding: 'utf8' });
check('the CLI writes the export', r.status === 0 && existsSync(join(site, 'export.json')) && existsSync(join(site, 'index.html')), r.stderr);

// a file server and nothing else
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm', '.woff2': 'font/woff2' };
const server = http.createServer((q, s) => {
  const p = join(site, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(site) || !existsSync(p) || statSync(p).isDirectory()) return s.writeHead(404).end();
  s.writeHead(200, { 'content-type': types[extname(p)] ?? 'application/octet-stream' });
  createReadStream(p).pipe(s);
});
await new Promise((res) => server.listen(0, '127.0.0.1', res));
const url = `http://127.0.0.1:${server.address().port}/index.html`;

const browser = await chromium.launch();
try {
  const p = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs = [];
  p.on('pageerror', (e) => errs.push(e.message));
  const until = async (fn, ms = 10000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await p.evaluate(fn).catch(() => false)) return true;
      await p.waitForTimeout(50);
    }
    return false;
  };
  await p.goto(url, { waitUntil: 'domcontentloaded' });
  check('the first chart view arrives', await until(() => window.__tlmFirstView != null));
  check('its time is noted (Gate 5 reads it)', (await p.evaluate(() => window.__tlmFirstView)) < 5000);
  check('the layout opens as the one tab', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1 && /Two panes/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')), await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
  check('the raw samples arrive in the background', await until(() => window.__tlmViewerLoaded === true));
  await p.click('[data-rail="explorer"]');
  const vinfo = await p.$eval('#viewerInfo', (e) => e.textContent.replace(/\s+/g, ' ').trim()).catch(() => null);
  check('the header says it is a read-only view, with the file and the app version (decision 6)', /Locked/.test(vinfo ?? '') && /bench\.csv/.test(vinfo ?? '') && /\d{4}/.test(vinfo ?? '') && /TelemetryBench \d/.test(vinfo ?? ''), vinfo);
  check('and has no Export button (nothing is exported from an export)', !(await p.$('#b-export')));
  check('Explorer lists the exported channels, with the layout\'s panes', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2 && /pane 2/.test(document.querySelector('#explorer .tree')?.textContent ?? '')), await p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.className)));
  check('nothing edits: no new tab, no layouts, no live, no start page', !(await p.$('#wsTabNew')) && !(await p.$('[data-rail="layouts"]')) && !(await p.$('[data-rail="live"]')) && !(await p.$('#b-start')));
  check('the tab shows its name from the start, never cut at the left (D-ui-41)', await p.evaluate(() => { const st = document.querySelector('#wsTabs').getBoundingClientRect(); const nm = document.querySelector('#wsTabs [aria-selected=true] .nm').getBoundingClientRect(); return nm.left >= st.left - 1; }));
  check('a viewer cannot make exports: Export data and Export to HTML are not in its menu (cut from the build, not disabled)', await (async () => {
    await p.click('#b-more');
    const gone = !(await p.$('.moremenu [data-more="exporthtml"]')) && !(await p.$('.moremenu [data-more="exportdata"]')) && !!(await p.$('.moremenu [data-more="exportimage"]'));
    await p.keyboard.press('Escape');
    return gone;
  })());
  await p.click('[data-rail="checks"]');
  check('the checks summary shows the set and its failure', await until(() => /Test limits/.test(document.querySelector('#checksBody')?.textContent ?? '') && /1 failed/.test(document.querySelector('#checksBody')?.textContent ?? '')), await p.$eval('#checksBody', (b) => b.textContent).catch(() => null));
  check('without the buttons that make sets', !(await p.isVisible('[data-cnew]')));
  check('no page errors', errs.length === 0, errs);

  // Light: a chart-only channel has only its pyramid, a table channel its raw samples
  writeFileSync(join(work, 'light.json'), JSON.stringify({ schema: 1, name: 'Light', panes: [{ channels: ['rpm'], view: 'chart' }, { channels: ['temp'], view: 'table' }] }));
  const light = join(work, 'light');
  const rl = spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'light.json'), '--mode', 'light', '--cache', join(work, 'cache'), '--out', light, '--viewer', VIEWER], { encoding: 'utf8' });
  check('Light writes the chart-only channel as a pyramid alone', rl.status === 0 && !existsSync(join(light, 'data/f0/c0.0.bin')) && existsSync(join(light, 'data/f0/c1.0.bin')), rl.stderr);
  site = light;
  const q = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs2 = [];
  q.on('pageerror', (e) => errs2.push(e.message));
  await q.goto(url, { waitUntil: 'domcontentloaded' });
  const untilQ = async (fn) => {
    const t = Date.now();
    while (Date.now() - t < 10000) {
      if (await q.evaluate(fn).catch(() => false)) return true;
      await q.waitForTimeout(60);
    }
    return false;
  };
  check('the chart-only channel draws from its summaries, labelled coarse', await untilQ(() => !!document.querySelector('[data-coarse]')), await q.$$eval('[data-coarse]', (x) => x.length));
  check('a coarse plot draws the same min/max band as the app (decision 7c)', await untilQ(() => (window.__tlmApp?.envelopes()[0] ?? []).some((n) => n > 0)), await q.evaluate(() => window.__tlmApp?.envelopes() ?? 'no hook'));
  check('the table shows the raw samples once they are here, never a pyramid value as one', await untilQ(() => /20\.00/.test(document.body.textContent ?? '') && !/—/.test(document.querySelector('.tbl, [role="grid"], table')?.textContent ?? '—')), await q.evaluate(() => document.body.textContent?.slice(0, 300)));
  check('no page errors in Light', errs2.length === 0, errs2);

  // Light with a check set: raw windows around the violations, and the chart-only channel holds only those samples
  const win = join(work, 'win');
  const rw = spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'light.json'), '--mode', 'light', '--config', join(work, 'cfg'), '--checks', 'Test limits', '--detail', '2s', '--cache', join(work, 'cache'), '--out', win, '--viewer', VIEWER], { encoding: 'utf8' });
  const mw = existsSync(join(win, 'export.json')) ? JSON.parse(readFileSync(join(win, 'export.json'), 'utf8')) : null;
  const held = (mw?.files[0].channels[0].raw_chunks ?? []).reduce((n, c) => n + c[1], 0);
  check('Light keeps raw windows around the violations for a chart-only channel', rw.status === 0 && held > 0 && held < 20000 && mw.files[0].channels[0].raw === true, [rw.stderr, held]);
  check('--detail 0 turns the windows off', (() => {
    const off = join(work, 'nowin');
    spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'light.json'), '--mode', 'light', '--config', join(work, 'cfg'), '--checks', 'Test limits', '--detail', '0', '--cache', join(work, 'cache'), '--out', off, '--viewer', VIEWER], { encoding: 'utf8' });
    return JSON.parse(readFileSync(join(off, 'export.json'), 'utf8')).files[0].channels[0].raw_chunks.length === 0;
  })());
  site = win;
  const w = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs3 = [];
  w.on('pageerror', (e) => errs3.push(e.message));
  await w.goto(url, { waitUntil: 'domcontentloaded' });
  check('a Light export with windows opens and draws', await (async () => {
    for (let i = 0; i < 150; i++) {
      if (await w.evaluate(() => window.__tlmViewerLoaded === true && !!document.querySelector('[data-coarse]')).catch(() => false)) return true;
      await w.waitForTimeout(60);
    }
    return false;
  })());
  check('no page errors with windows', errs3.length === 0, errs3);

  // a trimmed export (the finest pyramid levels left out) still draws, from the levels it has
  const trim = join(work, 'trim');
  const rt = spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'layout.json'), '--mode', 'light', '--budget', '2KB', '--cache', join(work, 'cache'), '--out', trim, '--viewer', VIEWER], { encoding: 'utf8' });
  const mt = existsSync(join(trim, 'export.json')) ? JSON.parse(readFileSync(join(trim, 'export.json'), 'utf8')) : null;
  check('a tiny budget trims the pyramid', rt.status === 0 && mt?.report.trimmed === true && mt.files[0].channels.every((c) => c.pyramid.first_level > 4), [rt.stderr, mt?.files[0].channels.map((c) => c.pyramid)]);
  site = trim;
  const tp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs4 = [];
  tp.on('pageerror', (e) => errs4.push(e.message));
  await tp.goto(url, { waitUntil: 'domcontentloaded' });
  check('a trimmed channel draws from its coarser levels, labelled coarse', await (async () => {
    for (let i = 0; i < 150; i++) {
      if (await tp.evaluate(() => !!document.querySelector('[data-coarse]')).catch(() => false)) return true;
      await tp.waitForTimeout(60);
    }
    return false;
  })());
  check('no page errors when trimmed', errs4.length === 0, errs4);

  // two files in one export: one tab, the second on the first's timeline (separate runs line up at their starts), and the
  // layout's `rpm·2` pane draws the second file's channel
  const csv2 = join(work, 'bench2.csv');
  writeFileSync(csv2, 'time,rpm,temp\n' + Array.from({ length: 20000 }, (_, i) => `${1000 + i / 100},${Math.cos(i / 50)},${30 + i / 300}`).join('\n') + '\n');
  writeFileSync(join(work, 'two.json'), JSON.stringify({ schema: 1, name: 'Both runs', panes: [{ channels: ['rpm', 'rpm·2'], view: 'chart' }, { channels: ['temp·2'], view: 'chart' }] }));
  const two = join(work, 'two');
  const r2 = spawnSync(CLI, ['export', 'html', csv, csv2, '--layout', join(work, 'two.json'), '--cache', join(work, 'cache'), '--out', two, '--viewer', VIEWER], { encoding: 'utf8' });
  const m2 = existsSync(join(two, 'export.json')) ? JSON.parse(readFileSync(join(two, 'export.json'), 'utf8')) : null;
  check('a two-file export records each file\'s alignment', r2.status === 0 && m2?.files.length === 2 && m2.group?.slots.length === 1 && m2.group.slots[0].method === 'starts' && m2.group.slots[0].knots.length === 1, [r2.stderr, m2?.group]);
  site = two;
  const gp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs7 = [];
  gp.on('pageerror', (e) => errs7.push(e.message));
  await gp.goto(url, { waitUntil: 'domcontentloaded' });
  const untilG = async (fn) => {
    for (let i = 0; i < 200; i++) {
      if (await gp.evaluate(fn).catch(() => false)) return true;
      await gp.waitForTimeout(60);
    }
    return false;
  };
  check('both files open as one tab', await untilG(() => window.__tlmFirstView != null && document.querySelectorAll('#wsTabs .wstab').length === 1), await gp.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent)));
  check('the second file is on the first\'s timeline: the view runs to about 200 s, not 1200 s', await untilG(() => { const v = window.__tlmApp?.view?.(); return !!v && BigInt(v.t1) > 190_000_000_000n && BigInt(v.t1) < 210_000_000_000n; }), await gp.evaluate(() => window.__tlmApp?.view?.()));
  check('the layout draws both runs (the pane with rpm and rpm·2 has two traces)', await untilG(() => (window.__tlmApp?.drawn?.()?.[0] ?? []).length === 2), await gp.evaluate(() => window.__tlmApp?.drawn?.()));
  check('no page errors with two files', errs7.length === 0, errs7);

  // a reduced bundle: the editing, importing and exporting dialogs are cut out of the viewer's code, not hidden
  const code = readdirSync(join(VIEWER, 'assets')).filter((f) => f.endsWith('.js')).map((f) => readFileSync(join(VIEWER, 'assets', f), 'utf8')).join('\n');
  const appCode = readdirSync(join(ROOT, 'app/ui/dist/assets')).filter((f) => f.endsWith('.js')).map((f) => readFileSync(join(ROOT, 'app/ui/dist/assets', f), 'utf8')).join('\n');
  const markers = ['htmlDlg', 'recipesDlg', 'importDlg'].filter((m) => appCode.includes(m));
  check('the viewer\'s code has none of the dialogs the app\'s has (export, recipes, import…)', markers.length >= 2 && markers.every((m) => !code.includes(m)), markers.filter((m) => code.includes(m)));
  check('and is smaller for it', code.length < appCode.length * 0.9, [code.length, appCode.length]);

  // gaps connected, as exported
  const cg = join(work, 'cg');
  spawnSync(CLI, ['export', 'html', csv, '--connect-gaps', '--layout', join(work, 'layout.json'), '--cache', join(work, 'cache'), '--out', cg, '--viewer', VIEWER], { encoding: 'utf8' });
  site = cg;
  const cp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  await cp.goto(url, { waitUntil: 'domcontentloaded' });
  check('--connect-gaps: the viewer draws gaps connected', await (async () => {
    for (let i = 0; i < 100; i++) {
      if (await cp.evaluate(() => window.__tlmSettings?.get('gapLines') === 'connect' && window.__tlmFirstView != null).catch(() => false)) return true;
      await cp.waitForTimeout(60);
    }
    return false;
  })(), await cp.evaluate(() => window.__tlmSettings?.get('gapLines')));
  check('without it the line breaks', JSON.parse(readFileSync(join(work, 'site/export.json'), 'utf8')).display.gap_lines === 'break' && JSON.parse(readFileSync(join(cg, 'export.json'), 'utf8')).display.gap_lines === 'connect');

  // XY and the spectrum work in a viewer (they were stubs): the pairs and the window are held samples
  writeFileSync(join(work, 'xy.json'), JSON.stringify({ schema: 1, name: 'XY and spectrum', panes: [{ channels: ['rpm'], view: 'xy', x: 'temp' }, { channels: ['rpm'], view: 'spectrum' }] }));
  const xyd = join(work, 'xy');
  spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'xy.json'), '--cache', join(work, 'cache'), '--out', xyd, '--viewer', VIEWER], { encoding: 'utf8' });
  site = xyd;
  const xp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs8 = [];
  xp.on('pageerror', (e) => errs8.push(e.message));
  await xp.goto(url, { waitUntil: 'domcontentloaded' });
  check('an XY pane and a Spectrum pane draw from the export', await (async () => {
    for (let i = 0; i < 200; i++) {
      if (await xp.evaluate(() => !!document.querySelector('[data-xy]') && !!document.querySelector('[data-spec]')).catch(() => false)) return true;
      await xp.waitForTimeout(60);
    }
    return false;
  })(), await xp.evaluate(() => [!!document.querySelector('[data-xy]'), !!document.querySelector('[data-spec]')]));
  check('and say nothing is unavailable', errs8.length === 0 && !(await xp.evaluate(() => /not available here/.test(document.body.textContent ?? ''))), errs8);

  // a link from a check report frames the violation: #show=<start>,<end> (the data runs 0..200 s; the violation is 100..101 s)
  site = join(work, 'site');
  const sp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  await sp.goto(url + '#show=100000000000,101000000000', { waitUntil: 'domcontentloaded' });
  check('#show frames the violation with context either side', await (async () => {
    for (let i = 0; i < 150; i++) {
      const v = await sp.evaluate(() => window.__tlmApp?.view?.()).catch(() => null);
      if (v && BigInt(v.t0) > 90000000000n && BigInt(v.t1) < 112000000000n && BigInt(v.t0) < 100000000000n && BigInt(v.t1) > 101000000000n) return true;
      await sp.waitForTimeout(80);
    }
    return false;
  })(), await sp.evaluate(() => window.__tlmApp?.view?.()));

  // one file: opens from file:// with no server, fetches nothing, and stays under 25 MB
  const one = join(work, 'one', 'report.html');
  const r1 = spawnSync(CLI, ['export', 'html', csv, '--single-file', '--layout', join(work, 'layout.json'), '--config', join(work, 'cfg'), '--checks', 'Test limits', '--cache', join(work, 'cache'), '--out', one], { encoding: 'utf8' });
  check('--single-file writes one HTML file and nothing beside it', r1.status === 0 && existsSync(one) && statSync(one).size < 25 * 1024 * 1024 && readdirSync(join(work, 'one')).length === 1, [r1.stderr, existsSync(one) && statSync(one).size]);
  const fp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs6 = [];
  const requests = [];
  fp.on('pageerror', (e) => errs6.push(e.message));
  fp.on('request', (q) => !q.url().startsWith('file:') && !q.url().startsWith('data:') && !q.url().startsWith('blob:') && requests.push(q.url()));
  await fp.goto('file://' + one, { waitUntil: 'domcontentloaded' });
  check('the one-file export draws from file://', await (async () => {
    for (let i = 0; i < 150; i++) {
      if (await fp.evaluate(() => window.__tlmFirstView != null && /Two panes/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')).catch(() => false)) return true;
      await fp.waitForTimeout(60);
    }
    return false;
  })(), errs6);
  check('it fetches nothing and has no page errors', requests.length === 0 && errs6.length === 0, [requests, errs6]);
  // a folder export from file:// can't load: the page says why, it doesn't sit empty
  const fold = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  await fold.goto('file://' + join(work, 'site', 'index.html'), { waitUntil: 'domcontentloaded' });
  await fold.waitForTimeout(1200);
  check('a folder export from file:// never draws data, and the page says it needs a server and offers --single-file', await fold.evaluate(() => window.__tlmFirstView == null && /web server/.test(document.body.textContent ?? '') && /--single-file/.test(document.body.textContent ?? '')), await fold.evaluate(() => document.body.textContent?.slice(0, 200)));
  const cmd = await fold.$eval('#file-cmd', (e) => e.textContent).catch(() => null);
  check('it gives the exact command for this folder (sheet F2)', cmd === `python3 -m http.server --directory '${join(work, 'site')}'`, cmd);
  check('with a Copy button, the address to open, and One file as the way that opens anywhere', !!(await fold.$('#file-copy')) && await fold.evaluate(() => /localhost:8000/.test(document.body.textContent) && /One file/.test(document.body.textContent)));

  // widgets of your own: an SVG one drawn safe, a code one as a value card, and no widget host (nothing runs)
  for (const [d, toml, f, body] of [
    ['dial', 'schema = 1\nname = "Dial"\nkind = "svg"\nentry = "widget.svg"\n', 'widget.svg', '<svg viewBox="0 0 10 10" onload="window.__stolen = 1"><script>window.__stolen = 1</script><text x="1" y="5" font-size="3">{{value}}</text></svg>'],
    ['bar', 'schema = 1\nname = "Bar (code)"\nkind = "code"\nentry = "widget.js"\n', 'widget.js', 'export default { build(el) { window.__stolen = 1 }, update() {} }'],
  ]) {
    mkdirSync(join(work, 'cfg/widgets', d), { recursive: true });
    writeFileSync(join(work, 'cfg/widgets', d, 'widget.toml'), toml);
    writeFileSync(join(work, 'cfg/widgets', d, f), body);
  }
  writeFileSync(join(work, 'wlayout.json'), JSON.stringify({ schema: 1, name: 'Widgets', panes: [{ channels: ['rpm'], view: 'gauges', widgets: { rpm: { type: 'sw_dial' } } }, { channels: ['temp'], view: 'gauges', widgets: { temp: { type: 'sw_bar' } } }] }));
  const wsite = join(work, 'wsite');
  const rwid = spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'wlayout.json'), '--config', join(work, 'cfg'), '--cache', join(work, 'cache'), '--out', wsite, '--viewer', VIEWER], { encoding: 'utf8' });
  check('widgets are exported', rwid.status === 0 && JSON.parse(readFileSync(join(wsite, 'export.json'), 'utf8')).widgets.length === 2, rwid.stderr);
  site = wsite;
  const wp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const errs5 = [];
  wp.on('pageerror', (e) => errs5.push(e.message));
  await wp.goto(url, { waitUntil: 'domcontentloaded' });
  check('both widgets draw as SVG', await (async () => {
    for (let i = 0; i < 150; i++) {
      if (await wp.evaluate(() => document.querySelectorAll('.gsvg').length >= 2).catch(() => false)) return true;
      await wp.waitForTimeout(60);
    }
    return false;
  })(), await wp.evaluate(() => document.querySelectorAll('.gsvg').length));
  check('nothing from the widgets ran, and no widget host is in the page', await wp.evaluate(() => window.__stolen === undefined && !document.querySelector('iframe') && !document.querySelector('.gsvg script')), null);
  check('no page errors with widgets', errs5.length === 0, errs5);

  // a damaged chunk is an error, never data
  const bad1 = join(work, 'damaged');
  spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'layout.json'), '--cache', join(work, 'cache'), '--out', bad1, '--viewer', VIEWER], { encoding: 'utf8' });
  const chunk = join(bad1, 'data/f0/c0.0.bin');
  const buf = readFileSync(chunk);
  buf[buf.length >> 1] ^= 0xff;
  writeFileSync(chunk, buf);
  site = bad1;
  const dp = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  await dp.goto(url, { waitUntil: 'domcontentloaded' });
  await dp.waitForTimeout(2500);
  check('a chunk that doesn\'t match its checksum is refused: the raw samples never finish loading', await dp.evaluate(() => window.__tlmViewerLoaded !== true), null);
  check('and the page says so', await dp.evaluate(() => /doesn.t match the export|damaged/i.test(document.body.textContent ?? '')), await dp.evaluate(() => document.body.textContent?.slice(0, 200)));
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} viewer: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
