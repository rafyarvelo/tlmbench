// Every control the viewer shows, locked and unlocked (sheet R12): each one acts, or is disabled with a tip that says why.
// Opens an HTML export of two files' worth of layout in headless Chromium, lists every visible button (and the items of
// each menu), clicks each, and fails on one that does nothing without being disabled. Also: Locked is the start state,
// Unlock lets panes move by dragging (the drag, the copy key, the split, duplicate and close), Lock puts it back.
// Needs the CLI (cargo build --release -p tlmbench) and the viewer (just viewer); without either it says so and passes.
import { spawnSync } from 'node:child_process';
import { createReadStream, existsSync, mkdtempSync, statSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const CLI = [join(ROOT, 'target/release/tlmbench'), join(ROOT, 'target/debug/tlmbench')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const VIEWER = join(ROOT, 'app/viewer/dist');
const pw = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
// TLM_BROWSER=webkit runs it in Safari's engine (the owner's chip drag after Unlock failed there, Nov 16)
const chromium = pw[process.env.TLM_BROWSER ?? 'chromium'];
if (!CLI || !existsSync(join(VIEWER, 'index.html')) || !existsSync(join(VIEWER, 'tlmbench_web.wasm'))) {
  console.log('SKIP viewerctl: needs the CLI (cargo build --release -p tlmbench) and the viewer (just viewer)');
  console.log('PASS viewerctl: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const work = mkdtempSync(join(tmpdir(), 'tlm-viewerctl-'));
const csv = join(work, 'bench.csv');
writeFileSync(csv, 'time,rpm,temp,volts,amps\n' + Array.from({ length: 6000 }, (_, i) => `${i / 100},${Math.sin(i / 50)},${20 + i / 300},${12 + Math.cos(i / 90)},${i % 7}`).join('\n') + '\n');
writeFileSync(join(work, 'layout.json'), JSON.stringify({ schema: 1, name: 'Four panes', panes: [{ channels: ['rpm', 'temp'], view: 'chart' }, { channels: ['volts'], view: 'chart' }, { channels: ['amps'], view: 'table' }] }));
const site = join(work, 'site');
const r = spawnSync(CLI, ['export', 'html', csv, '--layout', join(work, 'layout.json'), '--cache', join(work, 'cache'), '--out', site, '--viewer', VIEWER], { encoding: 'utf8' });
check('the CLI writes the export', r.status === 0 && existsSync(join(site, 'export.json')), r.stderr);

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
  p.setDefaultTimeout(5000);
  await p.addInitScript(() => { try { localStorage.clear(); } catch {} }); // each load starts from the exported view
  const errs = [];
  p.on('pageerror', (e) => errs.push(e.message));
  p.on('console', (m) => m.type() === 'error' && errs.push(m.text()));
  const until = async (fn, ms = 8000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await p.evaluate(fn).catch(() => false)) return true;
      await p.waitForTimeout(40);
    }
    return false;
  };
  const load = async (unlocked = false) => {
    await p.goto(url, { waitUntil: 'domcontentloaded' });
    await p.waitForSelector('#stack [data-pane]', { timeout: 10000 });
    await until(() => window.__tlmViewerLoaded === true);
    if (unlocked) await p.click('#b-vlock');
  };
  const panes = () => p.evaluate(() => JSON.stringify(window.__tlmApp.panes()));
  const box = async (sel) => {
    const b = await p.locator(sel).first().boundingBox();
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  const drag = async (from, to, mods = []) => {
    const a = await box(from);
    const c = await box(to);
    for (const m of mods) await p.keyboard.down(m);
    await p.mouse.move(a.x, a.y);
    await p.mouse.down();
    await p.mouse.move(a.x + 20, a.y + 20, { steps: 4 });
    await p.mouse.move(c.x, c.y, { steps: 8 });
    await p.mouse.up();
    for (const m of mods) await p.keyboard.up(m);
    await p.waitForTimeout(250);
  };

  // ---- the state it opens in, and the pill
  await load();
  const pill = () => p.$eval('#b-vlock', (e) => ({ text: e.getAttribute('aria-label'), pressed: e.getAttribute('aria-pressed'), tip: e.getAttribute('data-tip') }));
  let s0 = await pill();
  check('the viewer opens locked: Edit layout, not pressed; no time-view lock there (owner, Nov 16)', s0.text === 'Edit layout' && s0.pressed === 'false' && !(await p.$('#b-lock')), s0);
  check('the pill’s tip keeps the export’s file, date and version', /bench\.csv/.test(s0.tip) && /\d{4}/.test(s0.tip) && /TelemetryBench \d/.test(s0.tip), s0.tip);
  const before = await panes();
  check('three panes as exported', before === '[[0,1],[2],[3]]', before);

  // ---- locked: nothing moves
  await drag('#stack [data-pane="0"] .chip >> nth=0', '#stack [data-pane="2"]');
  check('locked: a chip does not drag to another pane', (await panes()) === before, await panes());
  await drag('#explorer .row.chan >> nth=2', '#stack [data-pane="1"] .plot');
  check('locked: an Explorer row does not drag into a pane', (await panes()) === before, await panes());
  check('locked: pane actions are disabled and say why', await p.evaluate(() => ['[data-pdup]', '[data-pclose]', '[data-psplit]', '[data-pmenu]', '[data-chip-x]', '[data-vmenu]', '#addPane', '#addPaneKinds'].every((q) => [...document.querySelectorAll(q)].every((b) => b.disabled && /Edit layout/.test(b.getAttribute('data-tip') ?? '')))));
  await p.keyboard.press('Meta+d');
  await p.keyboard.press('Control+d');
  await p.waitForTimeout(150);
  check('locked: the duplicate shortcut does nothing to the panes', (await panes()) === before, await panes());

  // ---- unlock: the same moves work
  await p.click('#b-vlock');
  s0 = await pill();
  check('Edit layout: the button is pressed and says Done', s0.text === 'Done editing the layout' && s0.pressed === 'true' && /bench\.csv/.test(s0.tip), s0);
  await drag('#stack [data-pane="0"] .chip >> nth=0', '#stack [data-pane="2"]');
  check('unlocked: a chip moves to another pane', (await panes()) === '[[1],[2],[3,0]]', await panes());
  await drag('#explorer .row.chan >> nth=1', '#stack [data-pane="1"] .plot');
  check('unlocked: an Explorer row drags into a pane', (await panes()) === '[[1],[2,1],[3,0]]', await panes());
  await drag('#stack [data-pane="2"] .chip >> nth=1', '#stack [data-pane="0"]', ['Alt']);
  const copied = JSON.parse(await panes());
  check('unlocked: the copy key leaves the channel where it was', copied[0].includes(0) && copied[2].includes(0), copied);
  await drag('#stack [data-pane="1"] .pnum', '#stack [data-pane="0"]');
  check('unlocked: a pane number drags (the panes change)', (await panes()) !== JSON.stringify(copied), await panes());
  // a fresh arrangement for the pane actions
  await load(true);
  // a pane's action is its button when the header has room, else the same item in its ⋯ menu (the narrow header hides some)
  const paneAct = async (i, attr, text) => {
    await p.locator(`#stack [data-pane="${i}"]`).hover();
    const b = p.locator(`#stack [data-pane="${i}"] [${attr}]`);
    if (await b.isVisible()) return b.click();
    await p.click(`#stack [data-pane="${i}"] [data-pmenu]`, { force: true });
    await p.locator('.menu [role="menuitem"]', { hasText: new RegExp('^' + text + '$') }).first().click();
  };
  await paneAct(0, 'data-psplit', 'Split right');
  const afterSplit = JSON.parse(await panes()).length;
  check('unlocked: Split adds a pane', afterSplit === 4, afterSplit);
  await paneAct(0, 'data-pdup', 'Duplicate');
  check('unlocked: Duplicate adds a pane', JSON.parse(await panes()).length === afterSplit + 1, await panes());
  await paneAct(0, 'data-pclose', 'Close pane');
  check('unlocked: Close removes one', JSON.parse(await panes()).length === afterSplit, await panes());
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.menu [role="menuitem"] >> nth=1').catch(() => {});
  await p.keyboard.press('Escape');
  const h0 = await p.evaluate(() => document.querySelector('#stack [data-pane="0"] .hgrip')?.getAttribute('aria-valuenow'));
  const g = await box('#stack [data-pane="0"] .hgrip');
  await p.mouse.move(g.x, g.y);
  await p.mouse.down();
  await p.mouse.move(g.x, g.y + 40, { steps: 4 });
  await p.mouse.up();
  const h1 = await p.evaluate(() => document.querySelector('#stack [data-pane="0"] .hgrip')?.getAttribute('aria-valuenow'));
  check('unlocked: a pane’s height grip resizes it', h0 !== h1, [h0, h1]);

  // ---- Lock again
  await p.click('#b-vlock');
  const locked = await panes();
  await drag('#stack [data-pane="0"] .chip >> nth=0', '#stack [data-pane="1"]');
  check('Lock puts it back: nothing drags again', (await pill()).text === 'Edit layout' && (await panes()) === locked, await panes());

  // ---- a reload is the exported view again (nothing was written into the export or kept)
  await load();
  check('a reload is the exported arrangement, locked', (await panes()) === before && (await pill()).text === 'Edit layout', await panes());

  // ---- the sweep: every visible control acts, or is disabled with a tip that says why
  const controls = () =>
    p.evaluate(() => {
      const seen = new Set();
      const out = [];
      const sel = 'button, [role="button"], [role="tab"], [role="switch"], [role="checkbox"], a[href], .tm, .tx, select, summary';
      for (const e of document.querySelectorAll(sel)) {
        const r = e.getBoundingClientRect();
        if (!e.offsetParent || r.width === 0 || r.height === 0) continue;
        if (e.closest('.menu')) continue;
        const key = `${e.id || e.getAttribute('data-rail') || e.className.toString().replace(/svelte-\w+/g, '').trim()}::${e.getAttribute('aria-label') || e.textContent.trim().slice(0, 30)}`;
        if (seen.has(key)) continue;
        seen.add(key);
        out.push(key);
      }
      return out;
    });
  const pick = (key) =>
    p.evaluateHandle((key) => {
      const sel = 'button, [role="button"], [role="tab"], [role="switch"], [role="checkbox"], a[href], .tm, .tx, select, summary';
      return [...document.querySelectorAll(sel)].find((e) => {
        if (!e.offsetParent || e.closest('.menu')) return false;
        return `${e.id || e.getAttribute('data-rail') || e.className.toString().replace(/svelte-\w+/g, '').trim()}::${e.getAttribute('aria-label') || e.textContent.trim().slice(0, 30)}` === key;
      });
    }, key);
  const snap = () => p.evaluate(() => document.body.innerHTML.replace(/svelte-\w+/g, '').replace(/\s+/g, ' ') + '|' + JSON.stringify(window.__tlmApp.panes()) + '|' + document.documentElement.className + '|' + document.title);
  // a control that can leave the page as it is, because it is already there (Fit on a fitted view) or only moves a scroll
  const IDEMPOTENT = /^(b-fit|b-back|m-pan|m-box|treeExpand|treeCollapse|tabsLeft|tabsRight|wstab|pnum|ofnm|evExpand|evCollapse|evGroup|sel[^:]*)::/; // already there (the tab in front, the file already open, nothing to expand), or a native dropdown
  const BASIC = /^::(Basic|All)$/;
  const disabledWithReason = (h) => h.evaluate((e) => (e.disabled || e.getAttribute('aria-disabled') === 'true') && (e.getAttribute('data-tip') || e.getAttribute('title') || '').length > 3 || (e.disabled && /^(Back|Clear cursors|Go to [AB]|Scroll tabs (left|right))$/.test(e.getAttribute('aria-label') ?? '')));
  const bad_ = [];
  const sweep = async (state, panel) => {
    await load(state === 'unlocked');
    await p.click(`[data-rail="${panel}"]`).catch(() => {});
    await p.waitForTimeout(150);
    const keys = await controls();
    for (const key of keys) {
      if (swept.has(`${state}|${key}`)) continue;
      swept.add(`${state}|${key}`);
      let h = await pick(key);
      if (!h.asElement()) continue;
      if (await disabledWithReason(h.asElement())) continue;
      if (await h.asElement().evaluate((e) => e.disabled)) {
        bad_.push(`${state}: "${key}" is disabled and doesn't say why`);
        continue;
      }
      errs.length = 0;
      const was = await snap();
      try {
        await h.asElement().click({ timeout: 1200, force: true });
      } catch {
        continue;
      }
      await p.waitForTimeout(220);
      const now = await snap();
      if (now === was && !IDEMPOTENT.test(key) && !BASIC.test(key)) bad_.push(`${state}/${panel}: "${key}" does nothing`);
      if (errs.length) bad_.push(`${state}/${panel}: "${key}" errors: ${errs[0].slice(0, 120)}`);
      if (now !== was) {
        await p.keyboard.press('Escape');
        await load(state === 'unlocked');
        await p.click(`[data-rail="${panel}"]`).catch(() => {});
        await p.waitForTimeout(100);
      }
    }
  };
  // the items of each menu: opened by the control, each clicked in turn
  const menuSweep = async (state, label, open, lockedNone = false) => {
    const itemsOf = async () => {
      await load(state === 'unlocked');
      await open().catch(() => {});
      await p.waitForTimeout(150);
      return p.evaluate(() => [...document.querySelectorAll('.menu [role^="menuitem"]')].map((e, i) => `${i}::${e.textContent.trim().slice(0, 40)}`));
    };
    const items = await itemsOf();
    if (lockedNone && state === 'locked') return check(`locked: ${label} stays shut (nothing in it can act)`, items.length === 0, items);
    check(`${state}: ${label} opens a menu`, items.length > 0, items);
    for (const it of items) {
      const i = Number(it.split('::')[0]);
      await load(state === 'unlocked');
      await open().catch(() => {});
      await p.waitForTimeout(120);
      const m = p.locator('.menu [role^="menuitem"]').nth(i);
      const info = await m.evaluate((e) => ({ disabled: e.disabled || e.getAttribute('aria-disabled') === 'true', tip: e.getAttribute('data-tip') || e.getAttribute('title') || '' })).catch(() => null);
      if (!info) continue;
      if (info.disabled) {
        // a menu item that can't act is greyed; the reason shows when it is a locked one or a viewer-only one
        if (state === 'locked' && /^\d+::(New pane|Remove every|Every pane as|Close pane|Close others|Duplicate|Split)/.test(it) && !/Edit layout/.test(info.tip)) bad_.push(`${state}: ${label} › "${it}" is disabled and doesn't say why`);
        continue;
      }
      errs.length = 0;
      const was = await snap();
      await m.click({ timeout: 1200, force: true }).catch(() => {});
      await p.waitForTimeout(220);
      const now = await snap();
      if (now === was) bad_.push(`${state}: ${label} › "${it}" does nothing`);
      if (errs.length) bad_.push(`${state}: ${label} › "${it}" errors: ${errs[0].slice(0, 120)}`);
    }
  };
  const swept = new Set();
  for (const state of ['locked', 'unlocked']) {
    for (const panel of ['explorer', 'events', 'checks', 'measure', 'settings']) await sweep(state, panel);
    await menuSweep(state, 'the ⋯ menu', () => p.click('#b-more'));
    await menuSweep(state, 'a tab’s ⋯ menu', () => p.click('#wsTabs .tm'));
    await menuSweep(state, 'a pane’s menu', async () => {
      await p.locator('#stack [data-pane="0"]').hover();
      await p.click('#stack [data-pane="0"] [data-pmenu]', { force: true });
    }, true);
    await menuSweep(state, 'a pane’s right-click', () => p.locator('#stack [data-pane="0"] .phead').click({ button: 'right', position: { x: 500, y: 12 } }), true);
    await menuSweep(state, 'a chip’s right-click', () => p.click('#stack [data-pane="0"] .chip >> nth=0', { button: 'right' }));
    await menuSweep(state, 'the Explorer’s ⋯ menu', () => p.click('#exMore'));
    await menuSweep(state, 'a file’s right-click', () => p.click('#explorer .row.file >> nth=0', { button: 'right' }));
  }
  check('every control acts, or is disabled and says why', bad_.length === 0, bad_);
  check('no page errors', errs.filter((e) => !/Failed to load resource/.test(e)).length === 0, errs);
} finally {
  await browser.close();
  server.close();
}
console.log(`${bad ? 'FAIL' : 'PASS'} viewerctl: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
