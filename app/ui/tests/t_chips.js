// A pane's chips: × takes a channel out, dragging by the name reorders (onto another chip) or moves (onto a pane; Alt
// copies), "· L" / "· R" where two units share a pane, and "+N" with the list when the header can't fit them all.
import { run } from './lib.js';

run('chips', async ({ p, check, wait, until, text, visible }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const chip = (i, ch) => `#stack [data-pane="${i}"] .chip[data-ch="${ch}"]`;

  // ×
  await p.hover(chip(0, 1));
  await p.click(`${chip(0, 1)} [data-chip-x]`);
  check('× takes a channel out of the pane', JSON.stringify((await app('panes'))[0]) === '[0]', await app('panes'));
  // put it back, then reorder by dragging onto another chip
  await p.click('#explorer .row.chan[data-ch="1"] .box');
  check('it can go back', JSON.stringify((await app('panes'))[0]) === '[0,1]', await app('panes'));
  await p.dragAndDrop(chip(0, 1), chip(0, 0), { targetPosition: { x: 4, y: 8 } });
  check('dropping a chip on another reorders the pane', JSON.stringify((await app('panes'))[0]) === '[1,0]', await app('panes'));

  // move to another pane, copy with Alt
  await p.dragAndDrop(chip(0, 0), '#stack [data-pane="1"] .plot');
  check('dropping a chip on a pane moves the channel', JSON.stringify((await app('panes')).slice(0, 2)) === '[[1],[2,3,0]]', await app('panes'));
  await p.keyboard.down('Alt');
  await p.dragAndDrop(chip(1, 2), '#stack [data-pane="0"] .plot');
  await p.keyboard.up('Alt');
  check('Alt copies it', JSON.stringify((await app('panes'))[0]) === '[1,2]' && (await app('panes'))[1].includes(2), await app('panes'));

  // two units in one pane: L and R
  check('two units: the chips say which side', (await p.$$eval('#stack [data-pane="0"] [data-side]', (e) => e.map((x) => x.textContent.replace(/\s+/g, ' ').trim()))).join() === '· L,· R', await text('#stack [data-pane="0"] .phead'));

  // overflow
  for (const ch of [3, 4, 5]) await p.click(`#explorer .row.chan[data-ch="${ch}"] .box`);
  await p.setViewportSize({ width: 700, height: 800 });
  await wait(500);
  check('a header that cannot fit them says "+N"', await until(() => !!document.querySelector('#stack [data-pane="0"] [data-more-chips]')), await text('#stack [data-pane="0"] .phead'));
  await p.click('#stack [data-pane="0"] [data-more-chips]');
  check('and lists every channel', (await p.$$('.chanlist .mi')).length === 5, await text('.chanlist'));
  await p.keyboard.press('Escape');
  await p.setViewportSize({ width: 1440, height: 900 });
  await wait(500);
  check('wide again: no "+N"', !(await visible('#stack [data-pane="0"] [data-more-chips]')));
});
