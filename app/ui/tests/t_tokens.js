// The type scale (CLAUDE.md's tokens: 11, 11.5, 12, 13, 15, 26 px) on the screens with the most small print, and
// Settings' rows keeping their text readable (audit Nov 22): a <small> left to the browser's "smaller" fell between the
// tokens (10 px in the widget editor's hints, 10.8 px in Settings), and the Plugins row's sentence was squeezed into a
// column a word wide beside its two buttons.
import { run } from './lib.js';

/** In the page: every visible element with text of its own whose size isn't a token (SVG drawings aside). */
const offScale = () => {
  const ok = new Set(['11px', '11.5px', '12px', '13px', '15px', '26px']);
  const bad = [];
  for (const e of document.querySelectorAll('body *')) {
    if (e.closest('svg')) continue;
    if (![...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) continue;
    const r = e.getBoundingClientRect();
    const cs = getComputedStyle(e);
    if (!r.width || !r.height || cs.visibility === 'hidden') continue;
    if (!ok.has(cs.fontSize)) bad.push(`${cs.fontSize} «${e.textContent.trim().slice(0, 30)}»`);
  }
  return bad;
};

run(
  'tokens',
  async ({ p, check, until }) => {
    await p.click('#b-settings');
    await until(() => !!document.querySelector('#iconSet'));
    const s = await p.evaluate(offScale);
    check("Settings' text is all on the type scale", s.length === 0, s);
    const row = await p.$eval('#pluginsRescan', (b) => { const r = b.closest('.urow'); const t = r.querySelector(':scope > span:first-child'); return { text: t.getBoundingClientRect().width, under: b.getBoundingClientRect().top >= t.getBoundingClientRect().bottom - 1 }; });
    check("Plugins' sentence keeps its width; the buttons go under it", row.text >= 200 && row.under, row);
    const cache = await p.$eval('#cacheClear', (b) => b.getBoundingClientRect().top === document.getElementById('cacheOpen').getBoundingClientRect().top);
    check("the cache's two buttons stay side by side", cache);

    // the widget editor's hints (Bindings, Inputs, Parameters, Preview)
    await p.click('[data-rail="explorer"]');
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const idx = await p.evaluate(() => window.__tlmApp.sel()); // the new pane, below the selected one
    await p.click(`#stack [data-pane="${idx}"] [data-vmenu]`);
    await p.click('.menu [data-vm="gauges"]');
    await until(() => document.querySelectorAll('#stack [data-gw] svg').length >= 3);
    await p.hover('#stack [data-gw="current_a"]');
    await p.click('#stack [data-gw="current_a"] [data-gwt]');
    await p.click('.widgetmenu [data-wt="yourlist"]');
    await p.click('[data-wnew="svg"]');
    await until(() => !!document.querySelector('[data-wsvg]'));
    const w1 = await p.evaluate(offScale);
    check("an SVG widget's form is on the type scale", w1.length === 0, w1);
    await p.click('[data-wnew="code"]');
    await until(() => !!document.querySelector('[data-wcode]'));
    const w2 = await p.evaluate(offScale);
    check("a code widget's form is on the type scale", w2.length === 0, w2);
  },
  {
    files: { '/data/flight.tlm': { channels: ['battery_v', 'current_a', 'throttle_pct'], units: ['V', 'A', '%'] } },
    launch: '/data/flight.tlm',
    settings: { customWidgets: [], trustedWidgetSources: [] },
  },
);
