// Renders an SVG to a square PNG with a transparent background, in the Playwright Chromium the prototype tests
// pin: node render.mjs <in.svg> <out.png> [size]. Run it from prototype/tests so `playwright` resolves.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(`${process.cwd()}/`)('playwright');
const [src, out, size = '1024'] = process.argv.slice(2);
const px = Number(size);
const svg = readFileSync(src, 'utf8');
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: px, height: px } });
await page.setContent(
  `<style>html,body{margin:0;background:transparent}svg{display:block;width:${px}px;height:${px}px}</style>${svg}`,
);
await page.screenshot({ path: out, omitBackground: true });
await browser.close();
