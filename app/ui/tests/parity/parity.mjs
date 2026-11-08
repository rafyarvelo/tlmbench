// Parity harness: opens the prototype and the app side by side, puts each in the same scene (a menu open, a dialog,
// a hover or selected state), saves both screenshots, and compares the computed styles of mapped elements. It reports;
// it doesn't fail. Build the app first (npm run build).
//   node parity/parity.mjs [scene ...] [--dpr 2] [--light]
// Writes out/parity/index.html (side by side, with a table of differences per scene) and prints a summary.
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { openAppPage, openProto } from './pages.js';
import { SCENES } from './scenes.js';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const OUT = join(DIR, '../out/parity');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i < 0 ? null : args.splice(i, 2)[1]; };
const dpr = Number(opt('--dpr') ?? 1);
const light = args.includes('--light') ? args.splice(args.indexOf('--light'), 1) && true : false;
const only = args.length ? SCENES.filter((s) => args.includes(s.name)) : SCENES;
const colorScheme = light ? 'light' : 'dark';
const PROTO_SETTINGS = { startPage: false, fileOpenNew: 'starter', fileOpenNewV: 2, theme: light ? 'light' : 'dark' };
const APP_SETTINGS = { theme: light ? 'light' : 'dark' };

// The styles compared for each pair; lengths within half a pixel count as the same.
const PROPS = ['font-family', 'font-size', 'font-weight', 'line-height', 'letter-spacing', 'text-transform', 'color', 'background-color', 'width', 'height', 'padding', 'gap', 'border-width', 'border-color', 'border-radius', 'box-shadow', 'opacity'];

/** In the page: the computed styles and box of the last visible element matching `sel`, or null. */
function measure([sel, props]) {
  const els = [...document.querySelectorAll(sel)].filter((e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== 'hidden'; });
  const e = els[els.length - 1];
  if (!e) return null;
  const cs = getComputedStyle(e);
  const v = {};
  for (const p of props) {
    if (p === 'padding') v[p] = ['top', 'right', 'bottom', 'left'].map((s) => cs.getPropertyValue('padding-' + s)).join(' ');
    else if (p === 'border-width') v[p] = ['top', 'right', 'bottom', 'left'].map((s) => cs.getPropertyValue(`border-${s}-width`)).join(' ');
    else if (p === 'border-color') v[p] = cs.getPropertyValue('border-top-width') === '0px' && cs.getPropertyValue('border-bottom-width') === '0px' ? 'none' : cs.getPropertyValue('border-top-color');
    else if (p === 'width' || p === 'height') v[p] = Math.round(e.getBoundingClientRect()[p] * 10) / 10 + 'px';
    else if (p === 'font-family') v[p] = cs.fontFamily.split(',')[0].replace(/["']/g, '').trim();
    else v[p] = cs.getPropertyValue(p);
  }
  const r = e.getBoundingClientRect();
  v.count = els.length;
  v.text = (e.innerText || '').replace(/\s+/g, ' ').trim().slice(0, 60);
  return { v, rect: { x: r.x, y: r.y, w: r.width, h: r.height } };
}

const same = (a, b) => {
  if (a === b) return true;
  const n = (s) => String(s).split(' ').map((x) => (/^-?[\d.]+px$/.test(x) ? parseFloat(x) : x));
  const [x, y] = [n(a), n(b)];
  return x.length === y.length && x.every((v, i) => (typeof v === 'number' && typeof y[i] === 'number' ? Math.abs(v - y[i]) <= 0.5 : v === y[i]));
};

async function shoot(env, scene, side, dir) {
  const steps = scene[side];
  try {
    if (steps) await steps(env.p);
    await env.p.waitForTimeout(scene.settle ?? 350);
  } catch (e) {
    return { error: String(e.message).split('\n')[0] };
  }
  const pairs = [];
  for (const [label, ps, as, props] of scene.pairs ?? []) pairs.push(await env.p.evaluate(measure, [side === 'proto' ? ps : as, props ?? PROPS]));
  const focus = scene.focus ? await env.p.evaluate(measure, [side === 'proto' ? scene.focus[0] : scene.focus[1], []]) : null;
  const shot = `${scene.name}_${side}.png`;
  await env.p.screenshot({ path: join(dir, shot) });
  let crop = null;
  if (focus) {
    const vp = env.p.viewportSize();
    const pad = 24;
    const x = Math.max(0, focus.rect.x - pad), y = Math.max(0, focus.rect.y - pad);
    const clip = { x, y, width: Math.min(vp.width - x, focus.rect.w + 2 * pad), height: Math.min(vp.height - y, focus.rect.h + 2 * pad) };
    crop = `${scene.name}_${side}_crop.png`;
    await env.p.screenshot({ path: join(dir, crop), clip });
  }
  return { shot, crop, pairs };
}

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

mkdirSync(OUT, { recursive: true });
const results = [];
for (const scene of only) {
  const proto = await openProto({ dpr, colorScheme, settings: { ...PROTO_SETTINGS, ...scene.protoSettings } });
  await proto.p.waitForTimeout(1500);
  const app = await openAppPage({ dpr, colorScheme, settings: { ...APP_SETTINGS, ...scene.appSettings } });
  const [p, a] = [await shoot(proto, scene, 'proto', OUT), await shoot(app, scene, 'app', OUT)];
  await proto.close();
  await app.close();
  const rows = (scene.pairs ?? []).map(([label, , , props], k) => {
    const [pv, av] = [p.pairs?.[k]?.v, a.pairs?.[k]?.v];
    const diffs = !pv || !av ? [['(element)', pv ? 'found' : 'missing', av ? 'found' : 'missing']] : [...(props ?? PROPS), 'count'].filter((q) => !same(pv[q], av[q])).map((q) => [q, pv[q], av[q]]);
    return { label, diffs, ptext: pv?.text, atext: av?.text };
  });
  const n = rows.reduce((s, r) => s + r.diffs.length, 0);
  results.push({ scene, p, a, rows, n });
  console.log(`${String(n).padStart(3)} differences  ${scene.name}${p.error ? '  (prototype: ' + p.error + ')' : ''}${a.error ? '  (app: ' + a.error + ')' : ''}`);
}

const html = `<!doctype html><meta charset="utf-8"><title>Parity report</title>
<style>
body { margin: 0; padding: 16px 24px; background: #0f1115; color: #e6e8ec; font: 13px/1.45 system-ui, sans-serif; }
h1 { font-size: 20px; margin: 0 0 4px; } h2 { font-size: 15px; margin: 32px 0 6px; } p { color: #9aa1ad; margin: 0 0 8px; }
.pair { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: start; }
.pair img { max-width: 100%; border: 1px solid #2a2f38; border-radius: 4px; display: block; }
.pair figcaption { color: #9aa1ad; font-size: 12px; margin-bottom: 4px; }
table { border-collapse: collapse; margin-top: 8px; font-size: 12px; } td, th { border-bottom: 1px solid #2a2f38; padding: 3px 10px 3px 0; text-align: left; vertical-align: top; }
td.v { font-family: ui-monospace, monospace; } .ok { color: #5fc58a; } .err { color: #f0706a; }
nav a { color: #6cb6ff; margin-right: 10px; }
</style>
<h1>Parity report</h1>
<p>Prototype (left) and app (right), ${dpr}x, ${colorScheme}. Styles of mapped elements are compared; lengths within 0.5 px count as the same.</p>
<nav>${results.map((r) => `<a href="#${r.scene.name}">${r.scene.name} (${r.n})</a>`).join('')}</nav>
${results.map(({ scene, p, a, rows }) => `<h2 id="${scene.name}">${esc(scene.name)}</h2><p>${esc(scene.about ?? '')}${p.error ? ` <span class="err">Prototype: ${esc(p.error)}</span>` : ''}${a.error ? ` <span class="err">App: ${esc(a.error)}</span>` : ''}</p>
<div class="pair"><figure><figcaption>Prototype</figcaption><img src="${p.crop ?? p.shot}"></figure><figure><figcaption>App</figcaption><img src="${a.crop ?? a.shot}"></figure></div>
<table><tr><th>Element</th><th>Property</th><th>Prototype</th><th>App</th></tr>
${rows.map((r) => (r.diffs.length ? r.diffs.map(([q, pv, av], i) => `<tr>${i ? '<td></td>' : `<td rowspan="1">${esc(r.label)}</td>`}<td>${esc(q)}</td><td class="v">${esc(pv)}</td><td class="v">${esc(av)}</td></tr>`).join('') : `<tr><td>${esc(r.label)}</td><td class="ok" colspan="3">same</td></tr>`)).join('')}
</table>`).join('')}`;
writeFileSync(join(OUT, 'index.html'), html);
writeFileSync(join(OUT, 'report.json'), JSON.stringify(results.map(({ scene, rows, p, a }) => ({ scene: scene.name, errors: [p.error, a.error].filter(Boolean), rows })), null, 1));
console.log(`${results.reduce((s, r) => s + r.n, 0)} differences in ${results.length} scenes: ${join(OUT, 'index.html')}`);
