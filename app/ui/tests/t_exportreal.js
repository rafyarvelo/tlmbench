// Export to HTML… against the real core (the daemon, no fake): a CSV opens, the dialog shows the sizes the core computes, Export
// writes the folder (the data, export.json, and the viewer the app embeds), and that folder opens from a plain static file
// server and draws. Needs the app binary built with the viewer (just viewer, then cargo build); without it the test says so
// and passes.
import { spawn } from 'node:child_process';
import { createReadStream, existsSync, mkdirSync, mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const ROOT = join(DIR, '../../..');
const BIN = [join(ROOT, 'target/release/tlmbench-app'), join(ROOT, 'target/debug/tlmbench-app')].filter(existsSync).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
const PACK = join(ROOT, 'app/viewer/viewer.pack');
const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
if (!BIN || !existsSync(PACK)) {
  console.log('SKIP exportreal: needs the app binary and the viewer (just viewer, then cargo build)');
  console.log('PASS exportreal: 0 ok, 0 failed (skipped)');
  process.exit(0);
}
let ok = 0;
let bad = 0;
const check = (name, cond, got) => {
  if (cond) ok++;
  else bad++;
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${cond ? '' : ` — got ${JSON.stringify(got)}`}`);
};

const home = mkdtempSync(join(tmpdir(), 'tlm-exportreal-'));
const csv = join(home, 'bench run.csv');
writeFileSync(csv, 'time,rpm,temp\n' + Array.from({ length: 5000 }, (_, i) => `${i / 100},${Math.sin(i / 50)},${20 + i / 300}`).join('\n') + '\n');
const dest = join(home, 'out');
mkdirSync(dest);
const daemon = spawn(BIN, ['--daemon', '--ui', join(DIR, '../dist')], { env: { ...process.env, HOME: home }, stdio: ['ignore', 'pipe', 'inherit'] });
const url = await new Promise((res, rej) => {
  daemon.stdout.once('data', (d) => res(String(d).trim()));
  daemon.once('exit', () => rej(new Error('the daemon exited')));
});
const browser = await chromium.launch();
try {
  let answer = csv;
  const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  page.on('dialog', (d) => d.accept(answer));
  const errs = [];
  page.on('pageerror', (e) => errs.push(e.message));
  const until = async (p, fn, ms = 10000) => {
    const t = Date.now();
    while (Date.now() - t < ms) {
      if (await p.evaluate(fn).catch(() => false)) return true;
      await p.waitForTimeout(60);
    }
    return false;
  };
  await page.goto(url);
  await page.waitForSelector('#wsTabs');
  await page.keyboard.press('ControlOrMeta+o');
  // a CSV goes through the import dialog first
  await until(page, () => !!document.getElementById('impGo'));
  await page.click('#impGo');
  check('the CSV opens in the real app', await until(page, () => /bench run/.test(document.querySelector('#wsTabs .wstab')?.textContent ?? '')));
  await page.click('#b-more');
  await page.click('.moremenu [data-more="exporthtml"]');
  check('Export to HTML… shows the core\'s sizes for Full and Light', await until(page, () => /Full \(\d/.test(document.querySelector('#htmlMode')?.textContent ?? '') && /Light \(\d/.test(document.querySelector('#htmlMode')?.textContent ?? '')), await page.$eval('#htmlMode', (e) => e.textContent).catch(() => null));
  answer = dest;
  await page.click('#htmlGo');
  check('Export writes the folder and says so', await until(page, () => /Wrote .* MB \(Full\)/.test(document.querySelector('#htmlDone')?.textContent ?? ''), 20000), await page.$eval('#htmlErr, #htmlDone', (e) => e.textContent).catch(() => null));
  const out = join(dest, 'bench run-export');
  check('the folder has the manifest, the data and the viewer', existsSync(join(out, 'export.json')) && existsSync(join(out, 'data/f0/c0.pyr.bin')) && existsSync(join(out, 'index.html')) && existsSync(join(out, 'tlmbench_web.wasm')), existsSync(out));
  const m = existsSync(join(out, 'export.json')) ? JSON.parse(readFileSync(join(out, 'export.json'), 'utf8')) : null;
  check('the manifest holds the layout in front', m?.layout?.panes?.length > 0 && m?.files?.[0]?.samples === 5000, m?.layout);

  // that folder from a plain static server
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm', '.woff2': 'font/woff2' };
  const server = http.createServer((q, s) => {
    const p = join(out, decodeURIComponent(new URL(q.url, 'http://x').pathname));
    if (!p.startsWith(out) || !existsSync(p) || statSync(p).isDirectory()) return s.writeHead(404).end();
    s.writeHead(200, { 'content-type': types[extname(p)] ?? 'application/octet-stream' });
    createReadStream(p).pipe(s);
  });
  await new Promise((res) => server.listen(0, '127.0.0.1', res));
  const v = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
  const verrs = [];
  v.on('pageerror', (e) => verrs.push(e.message));
  await v.goto(`http://127.0.0.1:${server.address().port}/index.html`, { waitUntil: 'domcontentloaded' });
  check('the written folder opens from a static server and draws its first view', await until(v, () => window.__tlmFirstView != null));
  check('with the layout\'s channels listed', await until(v, () => document.querySelectorAll('#explorer .tree .row.chan').length === 2));
  check('and no page errors in the app or the viewer', errs.length === 0 && verrs.length === 0, [...errs, ...verrs]);
  server.close();
} catch (e) {
  bad++;
  console.log('  FAIL threw', e.message);
}
daemon.kill();
await browser.close();
console.log(`${bad ? 'FAIL' : 'PASS'} exportreal: ${ok} ok, ${bad} failed`);
process.exit(bad ? 1 : 0);
