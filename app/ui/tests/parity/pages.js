// Opens the prototype (prototype/plotter.html) and the app (dist/, fake core) the same way for the parity harness: the
// same viewport, device scale and colour scheme. The prototype gets the real uPlot and TanStack table-core from vendor/,
// and IBM Plex from the app's own font files, so it renders as it does online, with no network.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { open as openApp } from '../lib.js';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const PAGE = join(DIR, '../../../../prototype/plotter.html');
const FONTS = join(DIR, '../../src/fonts');
const { chromium } = createRequire(join(DIR, '../../../../prototype/tests/package.json'))('playwright');

export async function openProto({ viewport = { width: 1440, height: 900 }, dpr = 1, colorScheme = 'dark', settings = {} } = {}) {
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport, deviceScaleFactor: dpr, colorScheme });
  const p = await ctx.newPage();
  const errs = [];
  p.on('pageerror', (e) => errs.push(e.message));
  await p.addInitScript((s) => { if (!sessionStorage.getItem('tlm.seeded')) { sessionStorage.setItem('tlm.seeded', '1'); localStorage.setItem('plotter.settings', s); } }, JSON.stringify(settings));
  await p.route('**/*', (r) => {
    const u = new URL(r.request().url());
    const file = (f, type) => r.fulfill({ body: readFileSync(f), contentType: type });
    if (u.hostname === 'cdn.jsdelivr.net' && u.pathname.includes('uplot')) return file(join(DIR, 'vendor/uPlot.iife.min.js'), 'text/javascript');
    if (u.hostname === 'cdn.jsdelivr.net' && u.pathname.includes('table-core')) return file(join(DIR, 'vendor/table-core.mjs'), 'text/javascript');
    if (u.hostname === 'fonts.googleapis.com') return r.fulfill({ body: readFileSync(join(FONTS, 'fonts.css'), 'utf8').replace(/url\('\.\//g, "url('https://fonts.local/"), contentType: 'text/css' });
    if (u.hostname === 'fonts.local') return file(join(FONTS, u.pathname.slice(1)), 'font/woff2');
    if (u.protocol === 'file:' || u.protocol === 'data:' || u.protocol === 'blob:') return r.continue();
    return r.abort();
  });
  await p.goto('file://' + PAGE);
  await p.evaluate(() => document.fonts.ready);
  return { b, p, errs, close: () => b.close() };
}

export async function openAppPage({ viewport = { width: 1440, height: 900 }, dpr = 1, colorScheme = 'dark', settings } = {}) {
  const env = await openApp({ viewport, dpr, colorScheme, settings, wait: 900 });
  await env.p.evaluate(() => document.fonts.ready);
  return { ...env, close: async () => { await env.b.close(); env.server.close(); } };
}
