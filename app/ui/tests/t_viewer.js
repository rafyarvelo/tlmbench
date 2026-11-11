// The viewer of an HTML export, from a plain static file server (no server code): `tlmbench export html` writes the folder
// (a layout, a check set's summary), a headless browser opens it. The UI is the app's over the core compiled to WASM:
// the layout's panes draw, channels list, the checks summary shows with the violations marked, and nothing edits.
// Needs the CLI (cargo build -p tlmbench) and the built viewer (just viewer); without either the test says so and passes.
import { spawnSync } from 'node:child_process';
import { createReadStream, existsSync, mkdirSync, mkdtempSync, statSync, writeFileSync } from 'node:fs';
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
const site = join(work, 'site');
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
  check('Explorer lists the exported channels, with the layout\'s panes', await until(() => document.querySelectorAll('#explorer .tree .row.chan').length === 2 && /pane 2/.test(document.querySelector('#explorer .tree')?.textContent ?? '')), await p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.className)));
  check('nothing edits: no new tab, no layouts, no live, no start page', !(await p.$('#wsTabNew')) && !(await p.$('[data-rail="layouts"]')) && !(await p.$('[data-rail="live"]')) && !(await p.$('#b-start')));
  check('Export to HTML… is off in a viewer', await (async () => {
    await p.click('#b-more');
    const off = await p.$eval('.moremenu [data-more="exporthtml"]', (b) => b.disabled || b.getAttribute('aria-disabled') === 'true').catch(() => true);
    await p.keyboard.press('Escape');
    return off;
  })());
  await p.click('[data-rail="checks"]');
  check('the checks summary shows the set and its failure', await until(() => /Test limits/.test(document.querySelector('#checksBody')?.textContent ?? '') && /1 failed/.test(document.querySelector('#checksBody')?.textContent ?? '')), await p.$eval('#checksBody', (b) => b.textContent).catch(() => null));
  check('without the buttons that make sets', !(await p.isVisible('[data-cnew]')));
  check('no page errors', errs.length === 0, errs);
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
await browser.close();
server.close();
console.log(`${bad ? 'FAIL' : 'PASS'} viewer: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
