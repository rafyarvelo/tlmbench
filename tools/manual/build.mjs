// Builds the User Manual: docs/manual/NN-*.md, in order, into one PDF with a cover (the version), a contents page, page
// numbers and working links between chapters, printed by the Chromium Playwright pins (no LaTeX or Pandoc). Fails on a
// link to a chapter or heading that isn't there, or an image that is missing.
//
//   node tools/manual/build.mjs [--out docs/manual/build/TelemetryBench-manual.pdf] [--html]   (`just manual`)
//   node tools/manual/build.mjs --web DIR                                                       (`just pages`)
//
// --html also writes the single HTML page beside the PDF. --web writes the web copy instead of the PDF: DIR/index.html, a
// page that reads on a screen, and the images it uses in DIR/images/ (relative addresses, so it works from any static host).

import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { Marked } from 'marked';
import { chromium } from 'playwright';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const DIR = join(ROOT, 'docs/manual');
const args = process.argv.slice(2);
const webArg = args.indexOf('--web');
const WEB = webArg >= 0 ? resolve(args[webArg + 1] ?? '') : null;
if (webArg >= 0 && !args[webArg + 1]) throw new Error('--web needs a folder');
const outArg = args.indexOf('--out');
const OUT = resolve(outArg >= 0 ? args[outArg + 1] : join(DIR, 'build/TelemetryBench-manual.pdf'));
const version = JSON.parse(readFileSync(join(ROOT, 'app/src-tauri/tauri.conf.json'), 'utf8')).version;

const chapters = readdirSync(DIR).filter((f) => /^\d\d-.*\.md$/.test(f)).sort();
if (!chapters.length) throw new Error('docs/manual has no chapters (NN-name.md)');

const slug = (s) => s.toLowerCase().replace(/<[^>]+>/g, '').replace(/[^\p{L}\p{N}\s-]/gu, '').trim().replace(/\s+/g, '-');
const problems = [];
const anchors = new Set();
const toc = [];
const wanted = []; // [from chapter, anchor]

const body = chapters
  .map((file) => {
    const id = file.replace(/\.md$/, '');
    const md = new Marked();
    md.use({
      renderer: {
        heading({ tokens, depth }) {
          const text = this.parser.parseInline(tokens);
          const a = `${id}--${slug(text)}`;
          anchors.add(a);
          if (depth === 1) anchors.add(id);
          if (depth <= 2) toc.push({ depth, text, a: depth === 1 ? id : a });
          return `<h${depth} id="${depth === 1 ? id : a}">${text}</h${depth}>\n`;
        },
        link({ href, tokens }) {
          const text = this.parser.parseInline(tokens);
          if (/^[a-z]+:/i.test(href)) return `<a href="${href}">${text}</a>`;
          // a chapter (`05-channels.md`, `05-channels.md#limits`) or a heading here (`#limits`)
          const m = href.match(/^(?:(\d\d-[\w-]+)\.md)?(?:#(.*))?$/);
          if (m && (m[1] || m[2])) {
            const ch = m[1] ?? id;
            const target = m[2] ? `${ch}--${m[2]}` : ch;
            wanted.push([file, target]);
            return `<a href="#${target}">${text}</a>`;
          }
          // anything else in the repository: a link to it on GitHub
          const path = resolve(DIR, href.split('#')[0]);
          if (!existsSync(path)) problems.push(`${file}: a link to ${href}, which isn't there`);
          const rel = path.slice(ROOT.length + 1);
          return `<a href="https://github.com/rafyarvelo/tlmbench/blob/main/${rel}">${text}</a>`;
        },
        image({ href, text }) {
          const path = resolve(DIR, href);
          if (!existsSync(path)) problems.push(`${file}: an image ${href}, which isn't there`);
          let src = pathToFileURL(path).href;
          if (WEB && existsSync(path)) {
            // kept under images/, with the same folders as docs/images
            const rel = path.slice(join(ROOT, 'docs').length + 1).replace(/\\/g, '/');
            mkdirSync(dirname(join(WEB, rel)), { recursive: true });
            copyFileSync(path, join(WEB, rel));
            src = rel;
          }
          return `<figure><img src="${src}" alt="${text}" loading="lazy"><figcaption>${text}</figcaption></figure>`;
        },
      },
    });
    return `<section class="chapter">${md.parse(readFileSync(join(DIR, file), 'utf8'))}</section>`;
  })
  .join('\n');

for (const [file, a] of wanted) if (!anchors.has(a)) problems.push(`${file}: a link to #${a.replace('--', ' › ')}, which isn't there`);
if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}

const contents = toc
  .map((t) => `<li class="d${t.depth}"><a href="#${t.a}">${t.text}</a></li>`)
  .join('\n');

const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>TelemetryBench ${version}: User Manual</title>
<style>
  @page { size: A4; margin: 20mm 18mm 22mm; }
  body { font: 10.5pt/1.5 -apple-system, "Segoe UI", Helvetica, Arial, sans-serif; color: #1b1f24; }
  h1, h2, h3 { line-height: 1.25; break-after: avoid; }
  h1 { font-size: 22pt; margin: 0 0 12pt; }
  h2 { font-size: 14pt; margin: 18pt 0 6pt; }
  h3 { font-size: 11.5pt; margin: 14pt 0 4pt; }
  .chapter { break-before: page; }
  code, pre { font: 9pt/1.45 ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
  code { background: #f1f3f5; padding: 0 3px; border-radius: 4px; }
  pre { background: #f1f3f5; padding: 8pt 10pt; border-radius: 8px; white-space: pre-wrap; break-inside: avoid; }
  pre code { background: none; padding: 0; }
  table { border-collapse: collapse; width: 100%; margin: 8pt 0; font-size: 9.5pt; break-inside: auto; }
  th, td { border-bottom: 1px solid #d8dde3; padding: 4pt 6pt; text-align: left; vertical-align: top; }
  tr { break-inside: avoid; }
  img { max-width: 100%; border: 1px solid #d8dde3; border-radius: 8px; }
  figure { margin: 10pt 0; break-inside: avoid; } figcaption { font-size: 9pt; color: #57606a; }
  a { color: #0a63c9; text-decoration: none; }
  .cover { height: 230mm; display: flex; flex-direction: column; justify-content: center; }
  .cover h1 { font-size: 30pt; margin: 0; } .cover p { font-size: 13pt; color: #57606a; margin: 6pt 0 0; }
  .toc { break-before: page; } .toc ol { list-style: none; padding: 0; } .toc .d1 { font-weight: 600; margin-top: 6pt; }
  .toc .d2 { margin-left: 14pt; font-size: 9.5pt; }
  ${WEB ? `@media screen {
    body { max-width: 58rem; margin: 0 auto; padding: 1.5rem 1rem 4rem; font-size: 16px; }
    .chapter { break-before: auto; margin-top: 3rem; padding-top: 1.5rem; border-top: 1px solid #d8dde3; }
    .cover { height: auto; padding: 2rem 0 1rem; } .toc { margin-top: 2rem; }
    pre { overflow-x: auto; white-space: pre; } table { display: block; overflow-x: auto; }
    :target { scroll-margin-top: 1rem; }
  }
  @media (prefers-color-scheme: dark) {
    body { background: #111418; color: #e3e6ea; } code, pre { background: #1c2128; } th, td { border-color: #30363d; }
    .chapter { border-color: #30363d; } a { color: #58a6ff; } figcaption { color: #9aa4af; } img { border-color: #30363d; }
  }` : ''}
</style></head><body>
<div class="cover"><h1>TelemetryBench</h1><p>User Manual, version ${version}</p>
<p>The code is licensed under the MIT License or the Apache License 2.0; this manual and its screenshots under CC BY 4.0.</p></div>
<nav class="toc"><h1>Contents</h1><ol>${contents}</ol></nav>
${body}
</body></html>`;

if (WEB) {
  mkdirSync(WEB, { recursive: true });
  writeFileSync(join(WEB, 'index.html'), html.replace('<head>', '<head><meta name="viewport" content="width=device-width, initial-scale=1">'));
  console.log(`${WEB}: ${chapters.length} chapters, version ${version}`);
  process.exit(0);
}
mkdirSync(dirname(OUT), { recursive: true });
const htmlPath = OUT.replace(/\.pdf$/, '.html');
writeFileSync(htmlPath, html);
const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(pathToFileURL(htmlPath).href, { waitUntil: 'load' });
await page.pdf({
  path: OUT,
  format: 'A4',
  printBackground: true,
  displayHeaderFooter: true,
  headerTemplate: '<span></span>',
  footerTemplate: `<div style="font: 8pt sans-serif; color: #888; width: 100%; padding: 0 18mm; display: flex; justify-content: space-between">
    <span>TelemetryBench ${version}</span><span><span class="pageNumber"></span> / <span class="totalPages"></span></span></div>`,
  margin: { top: '20mm', bottom: '22mm', left: '18mm', right: '18mm' },
  outline: true,
  tagged: true,
});
await browser.close();
if (!args.includes('--html')) (await import('node:fs')).rmSync(htmlPath);
console.log(`${OUT.slice(ROOT.length + 1)}: ${chapters.length} chapters, version ${version}`);
