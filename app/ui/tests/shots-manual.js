// The User Manual's screenshots (one per `<!-- shot: id — … -->` in docs/manual): the built UI with the fake core, over files
// shaped like tools/testdata/make.py's flights (altitude, speed, battery_v, temp; 60 s at 100 Hz), the same every run.
// Called by shots.js: `node shots.js OUTDIR --manual [id ...]`. Every scene writes OUTDIR/<id>.png, light (the manual is
// printed), and OUTDIR/<id>-dark.png where `dark: true`. `OUTDIR/manual-images.txt` lists which chapter uses which image.
import { open } from './lib.js';
import { spawnSync } from 'node:child_process';
import { createReadStream, existsSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../../..');

/** make.py's flight: the same channels, units, length and rate (the fake core draws the waveforms). */
const FLIGHT = { channels: ['altitude', 'speed', 'battery_v', 'temp'], units: ['m', 'm/s', 'V', '°C'], samples: 6000, rate: 100 };
const DIR = '/home/you/flights';
const FILES = {
  [`${DIR}/flight-1.csv`]: FLIGHT,
  [`${DIR}/flight-2.csv`]: FLIGHT,
  [`${DIR}/flight-bad.csv`]: FLIGHT,
  [`${DIR}/flight-gap.csv`]: FLIGHT,
};
const WIDE = { width: 1440, height: 800 };

const hoverHead = async (p, i) => {
  await p.hover(`#stack [data-pane="${i}"] .phead`);
};
const viewMenu = async (p, i, vm) => {
  await hoverHead(p, i);
  await p.click(`#stack [data-pane="${i}"] [data-vmenu]`);
  if (vm) await p.click(`.viewmenu [data-vm="${vm}"]`);
};
/** Opens the folder in Explorer and flight-1 in a tab. */
const withFolder = async (p) => {
  await p.evaluate((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  await p.dblclick('#explorer .row.file[data-path$="flight-1.csv"]');
  await p.waitForTimeout(900);
};
/** A numbered badge on each element: the labelled shots (the numbers are explained in the chapter's text). */
const badges = (p, items) =>
  p.evaluate((items) => {
    items.forEach(([sel, label, where], n) => {
      const e = document.querySelector(sel);
      if (!e) return;
      const r = e.getBoundingClientRect();
      const b = document.createElement('div');
      b.textContent = label ?? String(n + 1);
      const [dx, dy] = where ?? [4, 4];
      b.style.cssText = `position:fixed;z-index:99999;left:${r.left + dx}px;top:${r.top + dy}px;min-width:20px;height:20px;padding:0 6px;border-radius:10px;background:#d4351c;color:#fff;font:600 12px/20px system-ui;text-align:center;box-shadow:0 1px 4px #0006;pointer-events:none`;
      document.body.appendChild(b);
    });
  }, items);
const park = (p) => p.mouse.move(5, 780);

export const scenes = [];
const scene = (id, o, fn) => scenes.push({ id, o, fn });

scene('start-page', { dark: true, launch: null, viewport: WIDE }, async (p) => {
  await withFolder(p);
  for (const f of ['flight-bad', 'flight-gap']) {
    await p.evaluate((x) => (window.__tlmFake.pick = x), `${DIR}/${f}.csv`);
    await p.click('#b-start');
    await p.click('[data-start="open"] >> visible=true');
    await p.waitForTimeout(500);
  }
  await p.click('#b-start');
  await p.waitForSelector('.start.overlay');
  await p.waitForTimeout(400);
});
scene('first-look', { dark: true, viewport: WIDE }, async (p) => {
  await park(p);
});
scene('window-parts', { dark: true, viewport: WIDE }, async (p) => {
  await withFolder(p);
  await p.click('[data-rail="measure"]');
  await p.waitForTimeout(500);
  await park(p);
  await badges(p, [['[data-rail="settings"]', '1', [14, 44]], ['#explorer', '2', [140, 260]], ['#wsTabs', '3', [150, 24]], ['#spanTxt', '4', [70, 26]], ['#stack', '5', [500, 150]], ['.ovplot', '6', [300, -26]], ['#measure', '7', [120, 300]]]);
});
scene('explorer', { viewport: WIDE }, async (p) => {
  await withFolder(p);
  await p.click('#explorer .row.file[data-path$="flight-1.csv"]').catch(() => {});
  await park(p);
});
scene('panes-grid', { viewport: WIDE }, async (p) => {
  await park(p);
});
scene('top-bar', { dark: true, viewport: WIDE, clip: { x: 0, y: 0, width: 1440, height: 76 } }, async (p) => {
  await park(p);
  await badges(p, [['#wsTabs', '1', [60, 34]], ['#rangeTxt', '2', [60, 34]], ['#m-box', '3', [8, 34]], ['#b-clear', '4', [8, 34]], ['[data-allmode="chart"]', '5', [8, 34]], ['#b-export', '6', [2, 34]], ['#b-more', '7', [2, 34]]]);
});
scene('pointer-values', { viewport: WIDE }, async (p) => {
  const r = await p.evaluate(() => { const b = document.querySelectorAll('#stack section.pane .plot')[1].getBoundingClientRect(); return { x: b.left + 60 + (b.width - 76) * 0.45, y: b.top + b.height / 2 }; });
  await p.mouse.move(r.x, r.y);
  await p.waitForTimeout(500);
});
scene('cursors-measure', { viewport: WIDE }, async (p) => {
  await p.click('#m-cursor');
  for (const f of [0.3, 0.62]) {
    const c = await p.evaluate((f) => { const r = document.querySelectorAll('#stack section.pane .plot')[0].getBoundingClientRect(); return { x: r.left + 60 + (r.width - 76) * f, y: r.top + r.height / 2 }; }, f);
    await p.mouse.click(c.x, c.y);
  }
  await p.click('[data-rail="measure"]').catch(() => {});
  await p.waitForTimeout(500);
  await park(p);
});
scene('table-pane', { viewport: WIDE }, async (p) => {
  await viewMenu(p, 2, 'table');
  await p.waitForTimeout(700);
  await p.fill('#stack [data-pane="2"] [data-table] input[placeholder^="e.g."]', '> 1.5');
  await p.waitForSelector('#stack [data-pane="2"] [data-tfilt]');
  await p.click('#stack [data-pane="2"] [data-fapply]');
  await p.waitForTimeout(700);
  await park(p);
});
scene('transport', { viewport: WIDE }, async (p) => {
  await p.click('#b-play');
  await p.waitForTimeout(1500);
  await park(p);
});


// ---- shared fixtures for the dialogs ----
const BENCH = {
  schema: 1, name: 'Bench records', files: [], endian: 'little', header_size: 0, record_size: 12,
  time: { source: 'field', field: 't', unit: 'us' },
  fields: [
    { name: 't', type: 'u32', offset: 0 }, { name: 'altitude', type: 'f32', offset: 4, unit: 'm' },
    { name: 'battery', type: 'i16', offset: 8, unit: 'V', scale: 0.001 }, { name: 'armed', type: 'u8', offset: 10 },
  ],
};
/** make.py's flight-limits check set. */
const LIMITS = {
  schema: 1, name: 'Flight limits', runs_on_any_file: true, info_groups: [],
  check: [
    { name: 'Battery above 12 V', kind: 'check', group: 'Power', expect: 'battery_v > 12', severity: 'fail', grace: 0, settle: 0, enabled: true },
    { name: 'Temperature under 50', kind: 'check', group: 'Thermal', expect: 'temp < 50', severity: 'warn', grace: 0, settle: 0, enabled: true },
  ],
};
const RULES = {
  schema: 1, name: 'Event rules', runs_on_any_file: true, info_groups: [],
  check: [
    { name: 'Low battery', kind: 'rule', group: 'Alarm', expect: 'battery_v < 12.5', category: 'alarm', severity: 'fail', grace: 2, settle: 0, enabled: true },
  ],
};
const day = (d) => String(BigInt(Date.UTC(2026, 10, d)) * 1000000n);
const folder = async (p) => {
  await p.evaluate((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  await p.waitForTimeout(500);
};

scene('import-dialog', { launch: null, viewport: WIDE, files: { ...FILES, [`${DIR}/flight-new.csv`]: FLIGHT } }, async (p) => {
  await p.evaluate((d) => window.__tlmFake.csv(`${d}/flight-new.csv`, ['time', 'altitude', 'speed', 'battery_v', 'temp'], { unit: 's', rate: 100, units: { altitude: 'm', speed: 'm/s', battery_v: 'V', temp: '°C' }, rows: 6000 }), DIR);
  await folder(p);
  await p.dblclick('#explorer .row.file[data-path$="flight-new.csv"]');
  await p.waitForSelector('#importDlg');
  await p.waitForTimeout(500);
});
scene('descriptor-form', { launch: null, viewport: { width: 1440, height: 1300 } }, async (p) => {
  await p.evaluate(([d, bench]) => {
    const f = window.__tlmFake;
    const out = new Uint8Array(12 * 400);
    const dv = new DataView(out.buffer);
    for (let i = 0; i < 400; i++) { const t = i / 1000; dv.setUint32(i * 12, Math.round(t * 1e6), true); dv.setFloat32(i * 12 + 4, 120 + 80 * Math.sin(t / 9), true); dv.setInt16(i * 12 + 8, 12600 - 40 * t, true); out[i * 12 + 10] = t > 5 ? 1 : 0; }
    f.bin(`${d}/other/bench.bin`, { bytes: out, spec: { channels: ['altitude', 'battery', 'armed'], samples: 60000, rate: 1000 } });
    f.decoders['bench-records'] = bench;
  }, [DIR, BENCH]);
  await p.evaluate((d) => (window.__tlmFake.pickFolder = `${d}/other`), DIR);
  await p.click('#exFolder');
  await p.dblclick('#explorer .row.file[data-path$="bench.bin"]');
  await p.waitForSelector('#dscDlg');
  await p.selectOption('#dscPick', '/cache/decoders/bench-records.toml');
  await p.addStyleTag({ content: '.dscdlg{max-height:none!important}' });
  await p.waitForTimeout(800);
});
scene('export-data', { viewport: WIDE }, async (p) => {
  await p.click('#b-export');
  await p.click('.exportmenu [data-xm="data"]');
  await p.waitForSelector('#exportDlg');
  await p.waitForTimeout(500);
});
scene('connect-dialog', { launch: null, viewport: WIDE }, async (p) => {
  await p.click('[data-rail="live"]');
  await p.waitForSelector('#liveDlg, #lvHost, #lvPort');
  await p.click('#liveDlg [data-kind="udp"]').catch(() => {});
  await p.waitForTimeout(500);
});
scene('events-panel', { viewport: WIDE }, async (p) => {
  await p.click('[data-rail="events"]');
  await p.click('#m-cursor');
  for (const f of [0.25, 0.4]) { const c = await p.evaluate((f) => { const r = document.querySelectorAll('#stack section.pane .plot')[0].getBoundingClientRect(); return { x: r.left + 60 + (r.width - 76) * f, y: r.top + r.height / 2 }; }, f); await p.mouse.click(c.x, c.y); }
  await p.click('#evMarker');
  await p.fill('#eventsPanel [data-evf="name"]', 'Liftoff');
  await p.dispatchEvent('#eventsPanel [data-evf="name"]', 'change');
  await p.click('#eventsPanel [data-evdone]');
  await p.click('#evRange');
  await p.waitForTimeout(500);
});
scene('check-editor', { viewport: WIDE, open: { setup: (f, [a, b]) => { f.checkSets = [a, b]; }, setupArg: [LIMITS, RULES] } }, async (p) => {
  await p.click('[data-rail="checks"]');
  await p.waitForSelector('#checksBody');
  await p.click('#checksBody [data-cedit="Flight limits"]');
  await p.waitForSelector('#recDlg');
  await p.locator('#recDlg [data-xm]').nth(1).click();
  await p.click('#recDlg [data-kaddr]');
  await p.waitForTimeout(600);
});
scene('report-tab', { viewport: WIDE, open: { setupArg: [LIMITS, DIR, [day(1), day(2), day(3), day(4)]], setup: (f, [SET, DIR, days]) => {
  f.checkSets = [SET];
  f.checkOutcomes = {};
  const names = ['flight-1', 'flight-2', 'flight-bad', 'flight-gap'];
  f.checkFileTimes = Object.fromEntries(names.map((n, i) => [`${DIR}/${n}.csv`, days[i]]));
  f.checkFileOutcomes = {
    [`${DIR}/flight-1.csv`]: { 'Battery above 12 V': { value: 13.1 }, 'Temperature under 50': { value: 33.8 } },
    [`${DIR}/flight-2.csv`]: { 'Battery above 12 V': { value: 13.0 }, 'Temperature under 50': { value: 34.2 } },
    [`${DIR}/flight-bad.csv`]: { 'Battery above 12 V': { verdict: 'fail', runs: [[40, 50]], value: 11.4 }, 'Temperature under 50': { verdict: 'warn', runs: [[30, 35]], value: 52 } },
    [`${DIR}/flight-gap.csv`]: { 'Battery above 12 V': { value: 12.9 }, 'Temperature under 50': { value: 35.1 } },
  };
} } }, async (p) => {
  await folder(p);
  await p.click('[data-rail="checks"]');
  await p.click('#checksBody [data-cfolder]');
  await p.waitForSelector('#reportTab');
  await p.click('#reportTab [data-frun]');
  await p.waitForFunction(() => document.querySelector('#reportTab [data-frun]')?.textContent === 'Run again', null, { timeout: 10000 });
  await p.click('#reportTab [data-ftab="report"]');
  await p.waitForTimeout(600);
});
scene('settings-all', { viewport: WIDE }, async (p) => {
  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]').catch(() => {});
  await p.waitForTimeout(500);
});
scene('reset-app', { viewport: WIDE }, async (p) => {
  await p.click('#b-settings');
  await p.click('#setResetApp');
  await p.waitForSelector('#resetDlg');
  await p.waitForTimeout(300);
});
scene('channel-editor', { viewport: WIDE }, async (p) => {
  await p.click('#explorer .row.chan[data-ch="0"]', { button: 'right' });
  await p.click('.menu [data-cm="display"]');
  await p.waitForSelector('#chEditor');
  await p.waitForTimeout(300);
});
scene('derive-dialog', { viewport: WIDE }, async (p) => {
  await p.click('#exDerive');
  await p.waitForSelector('#deriveDlg');
  await p.waitForTimeout(500);
});
scene('share-dialog', { viewport: WIDE, open: { setupArg: [DIR], setup: (f, [d]) => {
  f.pickFolder = d;
  f.cfgRows = [
    { kind: 'views', key: 'views/altitude-and-battery.tlmview.json', name: 'Altitude and battery', state: 'new', detail: null },
    { kind: 'checks', key: 'checks/flight-limits.toml', name: 'Flight limits', state: 'changed', detail: null },
    { kind: 'recipes', key: 'recipes/power.toml', name: 'Power', state: 'same', detail: null },
    { kind: 'events', key: 'events/flight-1-csv.toml', name: 'flight-1.csv', state: 'changed', detail: '1 of yours not shared, 2 of theirs not yours' },
    { kind: 'categories', key: 'categories.toml', name: 'Event categories', state: 'folder', detail: null },
    { kind: 'widgets', key: 'widgets/level-bar', name: 'Level bar (code)', state: 'folder', detail: null },
  ];
  f.cfgRecipes = ['Power'];
} } }, async (p) => {
  await p.click('#exFolder');
  await p.waitForTimeout(400);
  await p.click('#exFolder');
  await p.click('.exmenu [data-ex="share"]');
  await p.waitForSelector('#shareDlg');
  await p.waitForTimeout(500);
});
scene('shortcuts-menu', { viewport: WIDE }, async (p) => {
  await p.click('#b-more');
  await p.waitForTimeout(400);
});
scene('browser-access', { viewport: WIDE }, async (p) => {
  await p.click('#b-settings');
  await p.click('#browserOn').catch(() => {});
  await p.waitForTimeout(500);
});


// ---- slots, alignment, widgets, tabs ----
const twoFiles = async (p) => {
  await p.evaluate((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  await p.dblclick('#explorer .row.file[data-path$="flight-1.csv"]');
  await p.waitForTimeout(700);
  await p.click('#explorer .row.file[data-path$="flight-2.csv"]');
  await p.click('#explorer .row.file.sel', { button: 'right' });
  await p.click('.filemenu [data-fm="add"]');
  await p.waitForTimeout(800);
};
scene('slots', { launch: null, viewport: WIDE }, async (p) => {
  await twoFiles(p);
  await p.click('#explorer .row.chan[data-ch="4"] .box');
  await park(p);
});
scene('alignment-section', { launch: null, viewport: { width: 1440, height: 900 } }, async (p) => {
  await twoFiles(p);
  await p.click('#explorer .row.chan[data-ch="4"] .box');
  await p.evaluate(() => (window.__tlmFake.alignResults.xcorr = { knots: [['0', '-3330000000']], residuals: [['1000000000', 420000], ['9000000000', -310000], ['17000000000', 150000], ['25000000000', -90000]], quality: 0.97, note: 'Matched on altitude: −3.33 s, r = 0.97', channel: 'altitude' }));
  await p.click('#explorer .row.slot[data-slot="2"]');
  await p.selectOption('[data-falign="2"]', 'xcorr');
  await p.waitForTimeout(900);
  await park(p);
});
const gaugePane = async (p) => {
  await p.focus('#explorer .row.chan[data-ch="0"]').catch(() => {});
  await p.keyboard.press('ControlOrMeta+a');
  await p.click('#selBar [data-sel="chart"]');
  const idx = (await p.locator('#stack section.pane').count()) - 1;
  await viewMenu(p, idx, 'gauges');
  await p.waitForTimeout(800);
  return idx;
};
const LEVEL = { id: 'sw_levelbar', name: 'Level bar (code)', shared: '.tlmbench/widgets/level-bar', by: 'j.kim', code: `{\n  build(el, info) { el.innerHTML = '<b class="t"></b><i class="n"></i>'; el.querySelector('.n').textContent = info.name; },\n  update(el, x) { el.querySelector('.t').textContent = x.txt; }\n}` };
scene('trust-widget', { viewport: WIDE, open: { settings: { customWidgets: [LEVEL], trustedWidgetSources: [] } } }, async (p) => {
  await gaugePane(p);
  const card = '#stack [data-gw="battery_v"]';
  await p.hover(card);
  await p.click(`${card} [data-gwt]`);
  await p.click('.widgetmenu [data-wt="yourlist"]');
  await p.click('.widgetmenu [data-wt="sw_levelbar"]');
  await p.waitForTimeout(500);
  await p.click(`${card} [data-trustw]`);
  await p.waitForSelector('#trustDlg');
  await p.click('#trustDlg summary');
  await p.waitForTimeout(400);
});
scene('widget-editor', { viewport: { width: 1440, height: 900 } }, async (p) => {
  await gaugePane(p);
  const card = '#stack [data-gw="battery_v"]';
  await p.hover(card);
  await p.click(`${card} [data-gwt]`);
  await p.click('.widgetmenu [data-wt="yourlist"]');
  await p.waitForSelector('#widgetDlg');
  await p.waitForTimeout(300);
  await p.waitForTimeout(500);
});
scene('tab-strip', { viewport: WIDE }, async (p) => {
  await p.evaluate((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('[data-rail="layouts"]');
  await p.click('#lyNew');
  await p.fill('#lyName', 'Altitude and battery');
  await p.press('#lyName', 'Enter');
  await p.waitForTimeout(500);
  await p.click('#addPane').catch(() => {});
  await p.click('#wsTabs .wstab:first-child').catch(() => {});
  await p.click('#wsTabs .wstab:first-child', { button: 'right' }).catch(() => {});
  await p.waitForTimeout(400);
});


scene('gauges-pane', { viewport: WIDE, files: { [`${DIR}/flight-1.csv`]: { channels: ['altitude', 'speed', 'battery_v', 'temp', 'heading', 'armed'], units: ['m', 'm/s', 'V', '°C', 'deg', 'state'], samples: 6000, rate: 100 } } }, async (p) => {
  await gaugePane(p);
  const types = { altitude: 'arc', speed: 'ring', battery_v: 'bar', temp: 'tank', armed: 'state', heading: 'compass' };
  for (const [ch, t] of Object.entries(types)) {
    const card = `#stack [data-gw="${ch}"]`;
    if (!(await p.$(card))) continue;
    await p.hover(card);
    await p.click(`${card} [data-gwt]`);
    await p.click(`.widgetmenu [data-wt="${t}"]`).catch(() => p.keyboard.press('Escape'));
    await p.waitForTimeout(150);
  }
  await park(p);
  await p.waitForTimeout(500);
});


// ---- scenes that are not the fake core: real builds over make.py's files, composites, a terminal ----
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.json': 'application/json', '.woff2': 'font/woff2' };
const serve = (dir) => new Promise((res) => {
  const srv = http.createServer((q, r) => {
    const f = join(dir, decodeURIComponent(new URL(q.url, 'http://x').pathname));
    if (!f.startsWith(dir) || !existsSync(f) || statSync(f).isDirectory()) return r.writeHead(404).end();
    r.writeHead(200, { 'content-type': TYPES[extname(f)] ?? 'application/octet-stream' });
    createReadStream(f).pipe(r);
  });
  srv.listen(0, '127.0.0.1', () => res(srv));
});
let testdata = null;
/** make.py's files, written once per run. */
const td = () => {
  if (!testdata) {
    testdata = mkdtempSync(join(tmpdir(), 'tlm-manual-shots-'));
    spawnSync('python3', [join(ROOT, 'tools/testdata/make.py'), join(testdata, 'td')], { stdio: 'ignore' });
  }
  return join(testdata, 'td');
};
/** Pictures in one page (a row or a column), as one PNG. */
const compose = async (file, pngs, { column = false, pad = 14, bg = '#e8ebef' } = {}) => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1700, height: 1000 } });
  await p.setContent(`<body style="margin:0;background:${bg}"><div id=c style="display:inline-flex;flex-direction:${column ? 'column' : 'row'};gap:${pad}px;padding:${pad}px;align-items:flex-start">${pngs.map((x) => `<img style="border:1px solid #9aa4af;border-radius:8px;display:block" src="data:image/png;base64,${x.toString('base64')}">`).join('')}</div></body>`);
  await p.locator('#c').screenshot({ path: file });
  await b.close();
};
const custom = (id, fn) => scenes.push({ id, custom: fn, o: {} });

custom('viewer-unlock', async (out) => {
  const CLI = join(ROOT, 'target/release/tlmbench');
  const VIEWER = join(ROOT, 'app/viewer/dist');
  if (!existsSync(CLI) || !existsSync(join(VIEWER, 'index.html'))) throw new Error('needs just viewer and the release CLI');
  const flights = join(td(), 'flights');
  const site = join(testdata, 'site');
  const r = spawnSync(CLI, ['export', 'html', join(flights, 'flight-bad.csv'), '--layout', 'Altitude and battery', '--checks', 'Flight limits', '--config', join(flights, '.tlmbench'), '--cache', join(testdata, 'cache'), '--viewer', VIEWER, '--out', site], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`export failed: ${r.stderr}`);
  const srv = await serve(site);
  const b = await chromium.launch();
  const p = await (await b.newContext({ viewport: { width: 1100, height: 520 }, colorScheme: 'light' })).newPage();
  await p.goto(`http://127.0.0.1:${srv.address().port}/index.html`);
  await p.waitForFunction(() => window.__tlmViewerLoaded === true, null, { timeout: 20000 });
  await p.mouse.move(5, 500);
  const head = { x: 0, y: 0, width: 1100, height: 46 };
  const locked = await p.screenshot({ clip: head });
  await p.click('#b-lock');
  await p.mouse.move(5, 500);
  await p.waitForTimeout(400);
  const unlocked = await p.screenshot({ clip: head });
  await b.close();
  srv.close();
  await compose(join(out, 'viewer-unlock.png'), [locked, unlocked], { column: true });
});
custom('browser-folder', async (out) => {
  const SITE = join(ROOT, 'app/browser/dist');
  if (!existsSync(join(SITE, 'tlmbench_web.wasm'))) throw new Error('needs just viewer (app/browser/dist)');
  const flights = join(td(), 'flights');
  const csvs = Object.fromEntries(['flight-1.csv', 'flight-2.csv', 'flight-bad.csv', 'flight-gap.csv'].map((n) => [n, readFileSync(join(flights, n), 'utf8')]));
  const srv = await serve(SITE);
  const b = await chromium.launch();
  const p = await (await b.newContext({ viewport: { width: 1280, height: 720 }, colorScheme: 'light' })).newPage();
  await p.addInitScript(() => {
    window.showDirectoryPicker = async () => {
      const root = await navigator.storage.getDirectory();
      const dir = await root.getDirectoryHandle('flights', { create: true });
      for (const [n, t] of Object.entries(window.__csvs)) {
        const w = await (await dir.getFileHandle(n, { create: true })).createWritable();
        await w.write(t);
        await w.close();
      }
      return dir;
    };
  });
  await p.goto(`http://127.0.0.1:${srv.address().port}/index.html`, { waitUntil: 'domcontentloaded' });
  await p.evaluate((c) => (window.__csvs = c), csvs);
  await p.click('#startPage [data-start="folder"]');
  await p.waitForSelector('#explorer .row.file[data-path$="flight-1.csv"]');
  await p.dblclick('#explorer .row.file[data-path$="flight-1.csv"]');
  await p.waitForFunction(() => document.querySelectorAll('#stack section.pane').length > 0, null, { timeout: 20000 });
  await p.waitForTimeout(1500);
  await p.mouse.move(5, 700);
  await p.screenshot({ path: join(out, 'browser-folder.png') });
  await b.close();
  srv.close();
});
custom('windows-two', async (out) => {
  const shots = [];
  for (const [w, view] of [[0, 'chart'], [1, 'table']]) {
    const { b, p, server } = await open({ files: FILES, launch: `${DIR}/flight-1.csv`, colorScheme: 'light', viewport: { width: 820, height: 640 }, wait: 1200 });
    if (view === 'table') await viewMenu(p, 2, 'table');
    await p.click('[data-rail="explorer"]').catch(() => {});
    await park(p);
    await p.waitForTimeout(500);
    shots.push(await p.screenshot());
    await b.close();
    server.close();
  }
  await compose(join(out, 'windows-two.png'), shots);
});
custom('cli-check', async (out) => {
  const CLI = join(ROOT, 'target/release/tlmbench');
  if (!existsSync(CLI)) throw new Error('needs the release CLI (cargo build --release -p tlmbench)');
  td();
  const args = ['check', 'flights/flight-bad.csv', 'flights/flight-1.csv', '--set', 'Flight limits', '--config', 'flights/.tlmbench', '--cache', join(testdata, 'cache')];
  const r = spawnSync(CLI, args, { cwd: td(), encoding: 'utf8' });
  const esc = (t) => t.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const cmd = 'tlmbench check flights/flight-bad.csv flights/flight-1.csv --set "Flight limits" --config flights/.tlmbench';
  const body = r.stdout.replace(/(Fail)/g, '<b style="color:#ff7b72">$1</b>').replace(/(Warn)/g, '<b style="color:#e3b341">$1</b>').replace(/(Pass)/g, '<b style="color:#56d364">$1</b>');
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1000, height: 400 } });
  await p.setContent(`<body style="margin:0;background:#e8ebef"><pre id=t style="display:inline-block;margin:14px;padding:16px 20px;border-radius:10px;background:#0d1117;color:#e6edf3;font:13px/1.55 ui-monospace,Menlo,Consolas,monospace;box-shadow:0 1px 6px #0004"><span style="color:#8b949e">$</span> ${esc(cmd)}\n${body.replace(/&/g, '&amp;').replace(/&amp;lt;/g, '&lt;')}<span style="color:#8b949e">$</span> echo $?\n${r.status}\n</pre></body>`);
  await p.locator('#t').screenshot({ path: join(out, 'cli-check.png') });
  await b.close();
});

/** Runs the scenes (all, or the ids given) into `out`. */
export async function manualShots(out, ids) {
  for (const s of scenes) {
    if (ids.length && !ids.includes(s.id)) continue;
    if (s.custom) {
      try {
        await s.custom(out);
        console.log(`${s.id}.png`);
      } catch (e) {
        console.log(`${s.id}: FAILED ${String(e.message || e).split('\n')[0]}`);
        process.exitCode = 1;
      }
      continue;
    }
    for (const scheme of s.o.dark ? ['light', 'dark'] : ['light']) {
      const { b, p, server, errs } = await open({ files: s.o.files ?? FILES, launch: s.o.launch === undefined ? `${DIR}/flight-1.csv` : s.o.launch, colorScheme: scheme, viewport: s.o.viewport ?? WIDE, wait: 1200, ...(s.o.open ?? {}) });
      p.setDefaultTimeout(8000);
      try {
        await s.fn(p, scheme);
        await p.waitForTimeout(300);
        const name = scheme === 'dark' ? `${s.id}-dark.png` : `${s.id}.png`;
        await p.screenshot({ path: join(out, name), ...(s.o.clip ? { clip: s.o.clip } : {}) });
        console.log(name, errs.length ? `(page errors: ${errs.slice(0, 2).join(' | ')})` : '');
      } catch (e) {
        console.log(`${s.id}: FAILED ${String(e.message || e).split('\n')[0]}`);
        await p.screenshot({ path: join(out, `fail-${s.id}.png`) }).catch(() => {});
        process.exitCode = 1;
      }
      await b.close();
      server.close();
    }
  }
  // which chapter uses which image (the list kept beside the images)
  const dir = join(ROOT, 'docs/manual');
  const rows = [];
  for (const f of readdirSync(dir).filter((x) => /^\d\d-.*\.md$/.test(x)).sort())
    for (const m of readFileSync(join(dir, f), 'utf8').matchAll(/!\[[^\]]*\]\(\.\.\/images\/manual\/([^)]+)\)/g)) rows.push(`| [${m[1]}](${m[1]}) | [${f}](../../manual/${f}) |`);
  writeFileSync(join(out, 'README.md'), `# Manual screenshots\n\nWritten by \`node app/ui/tests/shots.js OUTDIR --manual\` (\`app/ui/tests/shots-manual.js\`): the built UI over files shaped like \`tools/testdata/make.py\`'s flights, the real viewer, browser-only site and CLI over make.py's own files. The same data every time. Light, as the manual is printed. The gauges shot's file adds two channels (heading, armed) to the flight's four.\n\n| Image | Used by |\n| --- | --- |\n${rows.join('\n')}\n`);
  return existsSync(out);
}
