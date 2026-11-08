// Compares the icons of the controls the prototype and the app share: each side's SVG markup per control (matched by
// id, data attribute or label), normalised, and the size each draws at (an SVG keeps its aspect, so the smaller side). Prints the controls whose icons differ, with the prototype's markup.
//   node parity/icons.mjs [--all]
import { openAppPage, openProto } from './pages.js';

/** In the page: control key → { svg, size } for every visible control with an SVG icon. */
function icons() {
  const key = (e) => {
    if (e.id) return '#' + e.id;
    const d = [...e.attributes].find((a) => /^data-(rail|pmenu|vmenu|viewmenu|addpm|tabm|tabx)$/.test(a.name));
    if (d) return `[${d.name}]`;
    const l = e.getAttribute('aria-label');
    return l ? `[aria-label="${l}"]` : null;
  };
  const norm = (s) => s.outerHTML.replace(/ (class|aria-hidden|style|width|height)="[^"]*"/g, '').replace(/\s+/g, ' ').replace(/> </g, '><');
  const out = {};
  for (const e of document.querySelectorAll('button, [role=button], .tm, .tx')) {
    const s = e.querySelector('svg');
    const r = e.getBoundingClientRect();
    if (!r.width) continue;
    const k = key(e);
    if (!k || out[k]) continue;
    const sr = s?.getBoundingClientRect();
    out[k] = { svg: s ? norm(s) : null, size: s ? `${Math.round(Math.min(sr.width, sr.height) * 10) / 10}` : null, text: s ? '' : (e.innerText || '').trim().slice(0, 12) };
  }
  return out;
}

const all = process.argv.includes('--all');
const pr = await openProto({ settings: { startPage: false, fileOpenNew: 'starter', fileOpenNewV: 2, theme: 'dark' } });
await pr.p.waitForTimeout(1500);
await pr.p.locator('#stack section.pane .phead').first().hover();
const P = await pr.p.evaluate(icons);
await pr.close();
const ap = await openAppPage();
await ap.p.locator('#stack section.pane .phead').first().hover();
const A = await ap.p.evaluate(icons);
await ap.close();
// the app's names for the prototype's controls where they differ
const ALIAS = { '[data-viewmenu]': '[data-vmenu]', '#railHome': '#b-start', '[data-addpm]': '#addPaneKinds', '[data-tabm]': '[aria-label="Menu for flight.tlm"]', '[data-tabx]': '[aria-label="Close flight.tlm"]', '#exExpand': '#treeExpand', '#exCollapse': '#treeCollapse', '#exFolder': '#exFolderMenu' };
for (const [k, pv] of Object.entries(P)) {
  const a = A[ALIAS[k] ?? k];
  if (!a) { if (all) console.log(`only in prototype  ${k}`); continue; }
  const same = pv.svg === a.svg && pv.size === a.size;
  if (same && !all) continue;
  console.log(`${same ? 'same' : 'DIFF'}  ${k}  proto ${pv.size ?? JSON.stringify(pv.text)}  app ${a.size ?? JSON.stringify(a.text)}`);
  if (!same) console.log(`   proto: ${pv.svg ?? '(text)'}\n   app:   ${a.svg ?? '(text)'}`);
}
