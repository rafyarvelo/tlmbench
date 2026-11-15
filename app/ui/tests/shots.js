// Screenshots for the README and docs: the built UI with the fake core. Usage: node shots.js [outdir] (default docs/images)
import { open } from './lib.js';
import { spawnSync } from 'node:child_process';
import { createReadStream, existsSync, mkdtempSync, readFileSync, statSync } from 'node:fs';
import http from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const out = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), '../../../docs/images');
const files = { '/data/flight.tlm': { samples: 600000, rate: 1000 } };
const shot = async (p, name) => (await p.screenshot({ path: join(out, name) }), console.log(name));

for (const scheme of ['dark', 'light']) {
  const { b, p, server } = await open({ files, colorScheme: scheme, viewport: { width: 1440, height: 720 }, wait: 1200 });
  const at = (i, f) => p.evaluate(([i, f]) => { const r = document.querySelectorAll('#stack section.pane .plot')[i].getBoundingClientRect(); return { x: r.left + 60 + (r.width - 76) * f, y: r.top + r.height / 2 }; }, [i, f]);
  if (scheme === 'dark') {
    await p.click('#m-cursor');
    for (const f of [0.35, 0.62]) { const c = await at(0, f); await p.mouse.click(c.x, c.y); }
    await p.waitForTimeout(500);
    await shot(p, 'app-cursors.png');
    await p.keyboard.press('Escape');
    await p.click('#m-box');
    await p.click('[data-vmenu]');
    await p.waitForTimeout(300);
    await shot(p, 'app-view-menu.png');
    await p.keyboard.press('Escape');
    await p.click('#b-more');
    await p.waitForTimeout(300);
    await shot(p, 'app-more-menu.png');
    await p.keyboard.press('Escape');
    await p.click('#b-export');
    await p.click('.exportmenu [data-xm="html"]');
    await p.waitForTimeout(500);
    await shot(p, 'app-export-html.png');
    await p.keyboard.press('Escape');
    await p.click('#b-settings');
    await p.waitForTimeout(300);
    await shot(p, 'app-settings.png');
    await p.click('[data-rail="explorer"]');
  }
  await p.mouse.move(700, 700);
  await p.waitForTimeout(300);
  await shot(p, scheme === 'dark' ? 'app.png' : 'app-light.png');
  await b.close();
  server.close();
}

// live: the Connect dialog on file playback, and a live tab with its Sources card
{
  const { b, p, server } = await open({ files, viewport: { width: 1440, height: 760 }, launch: null, wait: 800 });
  await p.click('[data-rail="live"]');
  await p.fill('#lvRecording', '/data/flight.tlm');
  await p.fill('#lvName', 'Bench demo');
  await p.waitForTimeout(500);
  await shot(p, 'app-connect.png');
  await p.click('#lvConnect');
  await p.waitForTimeout(3500);
  await p.click('#lvSrcBtn');
  await p.mouse.move(5, 740);
  await p.waitForTimeout(800);
  await shot(p, 'app-live.png');
  await b.close();
  server.close();
}

// several files in one tab: slots in Explorer and file 2's Alignment section after a cross-correlation
{
  const gps = { channels: ['lat', 'lon', 'accel_x'], units: ['deg', 'deg', 'g'], samples: 60000, rate: 100 };
  const two = { '/data/runs/flight.tlm': { samples: 600000, rate: 1000 }, '/data/runs/gps.tlm': gps };
  const { b, p, server } = await open({ files: two, launch: null, viewport: { width: 1440, height: 760 }, wait: 800 });
  await p.evaluate(() => (window.__tlmFake.pickFolder = '/data/runs'));
  await p.click('#exFolder');
  await p.dblclick('#explorer .row.file[data-path$="flight.tlm"]');
  await p.waitForTimeout(800);
  await p.click('#explorer .row.file[data-path$="gps.tlm"]');
  await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
  await p.waitForTimeout(800);
  await p.click('#explorer .row.chan[data-ch="6"] .box');
  await p.evaluate(() => (window.__tlmFake.alignResults.xcorr = { knots: [['0', '-3330000000']], residuals: [['1000000000', 420000], ['9000000000', -310000], ['17000000000', 150000], ['25000000000', -90000]], quality: 0.97, note: 'Matched on accel_x: −3.33 s, r = 0.97', channel: 'accel_x' }));
  await p.click('#explorer .row.slot[data-slot="2"]');
  await p.selectOption('[data-falign="2"]', 'xcorr');
  await p.waitForTimeout(900);
  await p.mouse.move(5, 740);
  await shot(p, 'app-slots.png');
  await b.close();
  server.close();
}

// The viewer of an HTML export and browser-only mode: real builds over the owner test data (tools/testdata/make.py), each
// served by a plain file server. Skipped, with a note, when `just viewer` or the release CLI hasn't been built.
{
  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../../..');
  const CLI = join(ROOT, 'target/release/tlmbench');
  const VIEWER = join(ROOT, 'app/viewer/dist');
  const SITE = join(ROOT, 'app/browser/dist');
  const { chromium } = createRequire(join(ROOT, 'prototype/tests/package.json'))('playwright');
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.json': 'application/json', '.woff2': 'font/woff2' };
  const serve = (dir) => new Promise((res) => {
    const srv = http.createServer((q, r) => {
      const f = join(dir, decodeURIComponent(new URL(q.url, 'http://x').pathname));
      if (!f.startsWith(dir) || !existsSync(f) || statSync(f).isDirectory()) return r.writeHead(404).end();
      r.writeHead(200, { 'content-type': types[extname(f)] ?? 'application/octet-stream' });
      createReadStream(f).pipe(r);
    });
    srv.listen(0, '127.0.0.1', () => res(srv));
  });
  const work = mkdtempSync(join(tmpdir(), 'tlm-shots-'));
  spawnSync('python3', [join(ROOT, 'tools/testdata/make.py'), join(work, 'td')], { stdio: 'inherit' });
  const flights = join(work, 'td', 'flights');
  if (existsSync(CLI) && existsSync(join(VIEWER, 'index.html'))) {
    const out = join(work, 'site');
    const r = spawnSync(CLI, ['export', 'html', join(flights, 'flight-bad.csv'), '--layout', 'Altitude and battery', '--checks', 'Flight limits', '--config', join(flights, '.tlmbench'), '--cache', join(work, 'cache'), '--viewer', VIEWER, '--out', out], { encoding: 'utf8' });
    if (r.status !== 0) console.log('viewer shot: export failed', r.stderr);
    else {
      const srv = await serve(out);
      const b = await chromium.launch();
      const p = await (await b.newContext({ viewport: { width: 1440, height: 760 }, colorScheme: 'dark' })).newPage();
      await p.goto(`http://127.0.0.1:${srv.address().port}/index.html`);
      await p.waitForFunction(() => window.__tlmFirstView != null, null, { timeout: 20000 });
      await p.waitForTimeout(1500);
      await p.mouse.move(5, 740);
      await shot(p, 'app-viewer.png');
      await b.close();
      srv.close();
    }
  } else console.log('viewer shot skipped: needs just viewer and the release CLI');
  if (existsSync(join(SITE, 'tlmbench_web.wasm'))) {
    const srv = await serve(SITE);
    const b = await chromium.launch();
    const p = await (await b.newContext({ viewport: { width: 1440, height: 760 }, colorScheme: 'dark' })).newPage();
    await p.goto(`http://127.0.0.1:${srv.address().port}/index.html`);
    await p.waitForSelector('#explorer .exnone button');
    const chooser = p.waitForEvent('filechooser');
    await p.click('#explorer .exnone button:last-of-type');
    await (await chooser).setFiles({ name: 'flight-1.csv', mimeType: 'text/csv', buffer: readFileSync(join(flights, 'flight-1.csv')) });
    await p.waitForFunction(() => document.querySelectorAll('#stack section.pane').length > 0, null, { timeout: 20000 });
    await p.waitForTimeout(1500);
    await p.mouse.move(5, 740);
    await shot(p, 'app-browser.png');
    await b.close();
    srv.close();
  } else console.log('browser-only shot skipped: needs just browser-site');
}
