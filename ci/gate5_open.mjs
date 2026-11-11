// Gate 5's browser half: opens an export from a static host and reports when its first chart view arrived.
// node ci/gate5_open.mjs <url>   (prints one JSON line)
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..');
const { chromium } = createRequire(join(root, 'prototype/tests/package.json'))('playwright');
const url = process.argv[2];
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto(url, { waitUntil: 'domcontentloaded' });
// the viewer notes when its first view reply has been drawn from (time since the navigation began)
await page.waitForFunction(() => window.__tlmFirstView != null, null, { timeout: 60000 });
const first = await page.evaluate(() => window.__tlmFirstView);
// then the raw samples finish arriving and the exact view replaces the coarse one
await page.waitForFunction(() => window.__tlmViewerLoaded === true, null, { timeout: 120000 }).catch(() => {});
const loaded = await page.evaluate(() => window.__tlmViewerLoaded ?? null);
const tabs = await page.$$eval('#wsTabs .wstab', (t) => t.length);
await browser.close();
console.log(JSON.stringify({ firstViewMs: Math.round(first), loaded, tabs, errors }));
