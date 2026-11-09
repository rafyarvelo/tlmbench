// Shared harness for the app's UI tests: the built UI (dist/) in headless Chromium, with the fake core
// (fakecore.js) answering the Tauri commands.
//   import { run } from './lib.js';
//   run('name', async ({ p, check, fake }) => { ... }, { files: { '/data/a.tlm': {} }, launch: '/data/a.tlm', settings: {...} });
// A test fails (exit code 1) when a check fails, the page throws, or the script throws.
import { readFileSync, existsSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const DIST = join(DIR, '../dist');
// Playwright comes from the prototype's tests (one install, one Chromium)
const { chromium } = createRequire(join(DIR, '../../../prototype/tests/package.json'))('playwright');
const FAKE = readFileSync(join(DIR, 'fakecore.js'), 'utf8');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png' };
export const DEFAULT_FILE = '/data/flight.tlm';

/** Serves dist/ on a free port. */
function serve() {
  if (!existsSync(join(DIST, 'index.html'))) throw new Error('no app/ui/dist: run npm run build first');
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^(\.\.[/\\])+/, '');
    let f = join(DIST, path);
    if (!existsSync(f) || statSync(f).isDirectory()) f = join(DIST, 'index.html');
    res.writeHead(200, { 'content-type': TYPES[extname(f)] ?? 'application/octet-stream' });
    res.end(readFileSync(f));
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

export async function open(opts = {}) {
  const server = await serve();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: opts.viewport ?? { width: 1440, height: 900 }, deviceScaleFactor: opts.dpr ?? 1, colorScheme: opts.colorScheme ?? 'dark' });
  const p = await ctx.newPage();
  const files = opts.files ?? { [DEFAULT_FILE]: {} };
  const launch = opts.launch === undefined ? Object.keys(files)[0] ?? null : opts.launch;
  await p.addInitScript(FAKE);
  await p.addInitScript(([files, launch]) => Object.assign(window.__tlmFake.files, files) && (window.__tlmFake.launch = launch), [files, launch]);
  // what the fake core holds from the start: `setup(fake)` runs before the page does
  if (opts.setup) await p.addInitScript(([src, arg]) => (0, eval)(`(${src})`)(window.__tlmFake, arg), [opts.setup.toString(), opts.setupArg ?? null]);
  // first-launch settings, once per test: a reload finds what the page saved
  if (opts.settings) await p.addInitScript((s) => { if (!sessionStorage.getItem('tlm.seeded')) { sessionStorage.setItem('tlm.seeded', '1'); localStorage.setItem('tlmbench.settings', s); } }, JSON.stringify(opts.settings));
  const errs = [];
  p.on('pageerror', (e) => errs.push(e.message + ' @ ' + String(e.stack || '').split('\n')[1]));
  p.on('console', (m) => m.type() === 'error' && errs.push('console: ' + m.text()));
  await p.goto(`http://127.0.0.1:${server.address().port}/`);
  await p.waitForTimeout(opts.wait ?? 600);
  return { b, p, errs, server };
}

export async function run(name, body, opts = {}) {
  let failed = 0;
  let passed = 0;
  let env = null;
  const check = (label, cond, detail) => {
    if (cond) {
      passed++;
      console.log(`  ok   ${label}`);
    } else {
      failed++;
      console.log(`  FAIL ${label}${detail !== undefined ? ' — got ' + (typeof detail === 'string' ? detail : JSON.stringify(detail)) : ''}`);
    }
    return !!cond;
  };
  try {
    env = await open(opts);
    const { p } = env;
    const h = {
      p,
      check,
      errs: env.errs,
      fake: (fn, arg) => p.evaluate(fn, arg),
      text: (sel) => p.evaluate((s) => { const e = document.querySelector(s); return e ? e.innerText.replace(/\s+/g, ' ').trim() : null; }, sel),
      visible: (sel) => p.evaluate((s) => { const e = document.querySelector(s); return !!e && !e.hidden && !!e.offsetParent; }, sel),
      panes: () => p.evaluate(() => [...document.querySelectorAll('#stack section.pane .phead')].map((h) => h.innerText.replace(/\s+/g, ' ').trim())),
      paneCount: () => p.locator('#stack section.pane').count(),
      /** The centre of pane `i`'s plot, at fraction `fx` across it. */
      plotAt: (i, fx = 0.5) => p.evaluate(([i, fx]) => { const r = document.querySelectorAll('#stack section.pane .plot')[i].getBoundingClientRect(); return { x: r.left + 60 + (r.width - 76) * fx, y: r.top + r.height / 2 }; }, [i, fx]),
      settingsLS: () => p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.settings') || '{}')),
      wait: (ms) => p.waitForTimeout(ms),
      // poll a page condition until it's true (or the time runs out); returns its last value
      until: async (fn, arg, ms = 5000) => {
        const t = Date.now();
        let v;
        while (Date.now() - t < ms) {
          try { v = await p.evaluate(fn, arg); } catch { v = undefined; }
          if (v) return v;
          await p.waitForTimeout(100);
        }
        return v;
      },
    };
    await body(h);
  } catch (e) {
    failed++;
    console.log(`  FAIL script: ${String(e.message || e).split('\n').slice(0, 3).join(' / ')}`);
    try {
      await env.p.screenshot({ path: join(DIR, 'out', `fail_${name}.png`) });
      console.log(`  (screenshot: out/fail_${name}.png)`);
    } catch { /* no page */ }
  }
  if (env) {
    if (env.errs.length) {
      failed++;
      console.log(`  FAIL page errors: ${env.errs.slice(0, 3).join(' | ')}`);
    }
    await env.b.close();
    env.server.close();
  }
  console.log(`${failed ? 'FAIL' : 'PASS'} ${name}: ${passed} ok, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}
