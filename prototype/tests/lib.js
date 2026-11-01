// Shared harness for the prototype's smoke tests.
//   const T = require('./lib');
//   T.run('name', async ({ p, check, tabs, panes, drop }) => { ... }, { settings: {...}, phone: true });
// A test fails (exit code 1) when a check fails, the page throws, or the script throws.
const { chromium, devices } = require('playwright');
const fs = require('fs');
const path = require('path');

const DIR = __dirname;
const PAGE = path.resolve(DIR, '../plotter.html');
const URL = 'file://' + PAGE;
const BASE_SETTINGS = { startPage: false, fileOpenNew: 'starter', fileOpenNewV: 2, theme: 'dark' };

async function open(opts = {}) {
  const b = await chromium.launch();
  const ctx = await b.newContext(opts.phone ? { ...devices['iPhone 13'], viewport: { width: 390, height: 844 } } : { viewport: opts.viewport || { width: 1440, height: 900 } });
  const p = await ctx.newPage();
  const settings = Object.assign({}, BASE_SETTINGS, opts.settings || {});
  // seed first-launch settings once per test: on a reload the page must find what it saved, not the seed
  if (!opts.noSettings) await p.addInitScript(s => { try { if (window !== window.top) return; if (sessionStorage.getItem('tlm.seeded')) return; sessionStorage.setItem('tlm.seeded', '1'); if (!localStorage.getItem('plotter.settings')) localStorage.setItem('plotter.settings', s); } catch (_) { /* sandboxed frames (code widgets) have no storage */ } }, JSON.stringify(settings));
  const errs = [];
  p.on('pageerror', e => errs.push(e.message + ' @ ' + String(e.stack || '').split('\n')[1]));
  await p.route('**/*', r => {
    const u = r.request().url();
    if (u.includes('uplot')) return r.fulfill({ body: fs.readFileSync(path.join(DIR, 'stub.js'), 'utf8'), contentType: 'text/javascript' });
    if (u.includes('table-core')) return r.fulfill({ body: fs.readFileSync(path.join(DIR, 'tsstub.mjs'), 'utf8'), contentType: 'text/javascript' });
    // a phone needs the viewport meta the published page gets from its wrapper
    if (opts.phone && u.endsWith('plotter.html')) return r.fulfill({ body: '<meta name="viewport" content="width=device-width, initial-scale=1">' + fs.readFileSync(PAGE, 'utf8'), contentType: 'text/html' });
    if (u.startsWith('file:') || u.startsWith('data:')) return r.continue();
    return r.abort();
  });
  await p.goto(URL);
  await p.waitForTimeout(opts.wait == null ? 1500 : opts.wait);
  return { b, ctx, p, errs };
}

async function run(name, body, opts = {}) {
  let failed = 0, passed = 0, env = null;
  const check = (label, cond, detail) => {
    if (cond) { passed++; console.log(`  ok   ${label}`); }
    else { failed++; console.log(`  FAIL ${label}${detail !== undefined ? ' — got ' + (typeof detail === 'string' ? detail : JSON.stringify(detail)) : ''}`); }
    return !!cond;
  };
  try {
    env = await open(opts);
    const { p } = env;
    const h = {
      p, check, errs: env.errs,
      tabs: () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab')].map(e => e.innerText.replace(/\s+/g, ' ').trim() + (e.getAttribute('aria-selected') === 'true' ? '*' : ''))),
      panes: () => p.evaluate(() => [...document.querySelectorAll('#stack section.pane .phead')].map(h => h.innerText.replace(/\s+/g, ' ').trim())),
      paneCount: () => p.locator('#stack section.pane').count(),
      text: sel => p.evaluate(s => { const e = document.querySelector(s); return e ? e.innerText.replace(/\s+/g, ' ').trim() : null; }, sel),
      visible: sel => p.evaluate(s => { const e = document.querySelector(s); return !!e && !e.hidden && !!e.offsetParent; }, sel),
      csv: n => [n, fs.readFileSync(path.join(DIR, 'csv', n), 'utf8')],
      drop: async files => { await p.evaluate(list => { const dt = new DataTransfer(); list.forEach(([n, t]) => dt.items.add(new File([t], n, { type: 'text/csv' }))); window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true })); }, files); await p.waitForTimeout(800); },
      menu: () => p.evaluate(() => [...document.querySelectorAll('.catmenu .mi')].map(e => e.textContent)),
      pick: async re => { const ok = await p.evaluate(src => { const r = new RegExp(src); const b = [...document.querySelectorAll('.catmenu .mi, .panemenu .mi, .moremenu .mi')].find(x => r.test(x.textContent)); if (b) b.click(); return !!b; }, re.source); await p.waitForTimeout(300); return ok; },
      expandTree: async () => { await p.evaluate(() => { const b = document.getElementById('treeExpand'); if (b) b.click(); }); await p.waitForTimeout(200); },
      wait: ms => p.waitForTimeout(ms),
      // poll a page condition until it's true (or the time runs out); returns its last value
      until: async (fn, arg, ms = 10000) => { const t = Date.now(); let v; while (Date.now() - t < ms) { try { v = await p.evaluate(fn, arg); } catch (_) { v = undefined; } if (v) return v; await p.waitForTimeout(150); } return v; },
      settingsLS: () => p.evaluate(() => JSON.parse(localStorage.getItem('plotter.settings') || '{}')),
    };
    await body(h);
  } catch (e) {
    failed++; console.log(`  FAIL script: ${String(e.message || e).split('\n').slice(0, 3).join(' / ')}`);
    try { await env.p.screenshot({ path: path.join(DIR, `fail_${name}.png`) }); console.log(`  (screenshot: fail_${name}.png)`); } catch (_) { /* no page */ }
  }
  if (env) {
    if (env.errs.length) { failed++; console.log(`  FAIL page errors: ${env.errs.slice(0, 3).join(' | ')}`); }
    await env.b.close();
  }
  console.log(`${failed ? 'FAIL' : 'PASS'} ${name}: ${passed} ok, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

module.exports = { run, open, URL, DIR };
