// A pane's view: the view menu switches a pane between a chart and value tiles; tiles show the value at the pointer,
// else at cursor A, else at the end of the view; the choice is saved with the layout and kept by Duplicate.
import { run } from './lib.js';

run('modes', async ({ p, check, text, visible, until, settingsLS, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const viewMenu = async (i) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-vmenu]`);
  };
  const pick = (mode) => p.click(`.viewmenu [data-vm="${mode}"]`);
  const tile = (ch) => text(`#stack [data-tile="${ch}"] .tval`);

  check('panes start as charts', (await app('modes')).every((m) => m === 'chart'), await app('modes'));
  await viewMenu(0);
  const items = await p.$$eval('.viewmenu [data-vm]', (b) => b.map((x) => x.dataset.vm + ':' + x.getAttribute('aria-checked')));
  check('the view menu', items.join() === 'chart:true,table:false,both:false,values:false,spectrum:false,checks:false', items);
  await pick('values');
  check('pane 1 shows values', JSON.stringify(await app('modes')).startsWith('["values"'), await app('modes'));
  const chans = (await app('panes'))[0];
  check('a tile per channel, no plot', (await p.$$('#stack [data-pane="0"] [data-tile]')).length === chans.length && !(await p.$('#stack [data-pane="0"] .plot')));
  check('at rest the tiles hold the end of the view', await until(() => /\d/.test(document.querySelector('#stack [data-tile] .tval').textContent)), await tile(chans[0]));
  check('the other panes still draw', await until(() => window.__tlmApp.drawn().slice(1).some((d) => d && d.length)));

  // the pointer on another pane's chart drives the tiles
  const r = await p.$eval('#stack [data-pane="1"] .plot', (e) => { const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; });
  const rest = await tile(chans[0]);
  await p.mouse.move(r.x + r.w * 0.25 + 60, r.y + r.h / 2);
  check('hover moves them', await until((c) => document.querySelector(`[data-tile="${c}"] .tval`).textContent !== '–', chans[0]) && (await tile(chans[0])) !== rest, [rest, await tile(chans[0])]);

  // saved with the layout, and kept by Duplicate
  check('saved', (await settingsLS()).layouts['/data/flight.tlm'][0].mode === 'values', (await settingsLS()).layouts);
  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-pmenu]');
  await p.locator('.panemenu .mi', { hasText: /^Duplicate$/ }).click();
  check('duplicate keeps the view', JSON.stringify((await app('modes')).slice(0, 2)) === '["values","values"]', await app('modes'));
  await p.reload();
  await wait(800);
  check('it comes back after a reload', JSON.stringify((await app('modes')).slice(0, 2)) === '["values","values"]', await app('modes'));

  // back to a chart
  await viewMenu(0);
  await pick('chart');
  check('chart again', (await app('modes'))[0] === 'chart' && (await settingsLS()).layouts['/data/flight.tlm'][0].mode === undefined, await app('modes'));
  check('and it draws', await until(() => { const d = window.__tlmApp.drawn()[0]; return d && d.length; }));
});
