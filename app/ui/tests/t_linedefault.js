// A default line for every channel without one of its own (owner decision 4, sheet B3): Settings › Display has Default line
// width and style; a channel's own width or style wins; the channel editor's first choice names the default. Checked in the
// SVG image export, which draws each trace with the width and dash the screen uses.
import { run } from './lib.js';

run('linedefault', async ({ p, check, until, fake }) => {
  const svg = async () => {
    const n = await fake(() => window.__tlmFake.saved_files.length);
    await p.click('#b-export');
    await p.click('.exportmenu [data-xm="image"]');
    await until(() => !!document.querySelector('#imageDlg'));
    await p.selectOption('#imFormat', 'svg');
    await p.selectOption('#imWhat', 'sel');
    await fake(() => (window.__tlmFake.savePick = '/out/l.svg'));
    await p.click('#imGo');
    await until((k) => window.__tlmFake.saved_files.length === k + 1, n);
    await p.keyboard.press('Escape');
    const t = await fake(() => new TextDecoder().decode(new Uint8Array(window.__tlmFake.saved_files.at(-1).bytes)));
    return [...t.matchAll(/<polyline [^>]*stroke-width="([\d.]+)"([^>]*)/g)].map((m) => ({ w: Number(m[1]), dash: /stroke-dasharray/.test(m[2]) }));
  };
  let lines = await svg();
  check('to start, every line is normal (1.25 px) and solid', lines.length === 2 && lines.every((l) => l.w === 1.25 && !l.dash), lines);

  await p.click('[data-rail="settings"]');
  await until(() => !!document.querySelector('[data-set="lineWidth"]'));
  await p.selectOption('[data-set="lineWidth"]', '3');
  await p.selectOption('[data-set="lineStyle"]', 'dashed');
  lines = await svg();
  check('Default line Heavy and Dashed reach every channel', lines.length === 2 && lines.every((l) => l.w === 3.5 && l.dash), lines);

  // a channel's own width and style win
  await p.click('#stack [data-pane="0"] .chip[data-ch="1"]');
  await until(() => !!document.querySelector('#chEditor'));
  check('the channel editor names the default as its first choice', /Default \(heavy\)/.test(await p.$eval('#chEditor [data-d="width"] option', (o) => o.textContent)) && /Default \(dashed\)/.test(await p.$eval('#chEditor [data-d="dash"] option', (o) => o.textContent)));
  await p.selectOption('#chEditor [data-d="width"]', '1');
  await p.selectOption('#chEditor [data-d="dash"]', 'solid');
  await p.click('#chEditor [data-close]');
  lines = await svg();
  check('a channel\'s own Thin and Solid win over the default', lines.some((l) => l.w === 3.5 && l.dash) && lines.some((l) => l.w === 0.875 && !l.dash), lines);
}, { settings: { settingsAll: true } });
