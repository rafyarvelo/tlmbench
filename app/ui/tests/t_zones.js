// Moving a pane: dropping its number on another pane's edges (beside it, above or below it) or middle (the
// channels merge; Alt copies), the "+" between rows, "+ Add pane ▾" (kinds), Duplicate beside, and the empty states.
import { run } from './lib.js';

run('zones', async ({ p, check, wait, until, paneCount, visible, text }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const layout = () => p.evaluate(() => [...document.querySelectorAll('#stack .prow')].map((r) => [...r.querySelectorAll('section.pane')].map((s) => s.dataset.pane).join('/')).join(' | '));
  const num = (i) => `#stack [data-pane="${i}"] .pnum`;
  const drop = (from, to, x, y) => p.dragAndDrop(num(from), `#stack [data-pane="${to}"]`, { force: true, targetPosition: { x, y } });
  const size = (i) => p.evaluate((i) => { const r = document.querySelector(`#stack [data-pane="${i}"]`).getBoundingClientRect(); return { w: r.width, h: r.height }; }, i);

  check('three rows at first', (await layout()) === '0 | 1 | 2', await layout());
  let s = await size(0);
  await drop(2, 0, s.w * 0.92, s.h * 0.5);
  check('the right edge puts it beside', (await layout()) === '0/1 | 2' && (await paneCount()) === 3, await layout());
  s = await size(0);
  const p2 = (await app('panes'))[2];
  await drop(2, 0, s.w * 0.06, s.h * 0.5);
  check('the left edge puts it first in the row', (await layout()) === '0/1/2' && JSON.stringify((await app('panes'))[0]) === JSON.stringify(p2), [await layout(), await app('panes')]);
  const ch0 = JSON.stringify(await app('panes'));
  await p.click('#b-more');
  await p.click('.moremenu [data-more="nopanes"]');
  await p.click('#wsNote button:has-text("Undo")');
  check('Undo restores them', JSON.stringify(await app('panes')) === ch0);

  // from a clean three
  await p.evaluate(() => localStorage.clear());
  await p.reload();
  await wait(1000);
  const orig = await app('panes');
  s = await size(1);
  await drop(2, 1, s.w * 0.5, s.h * 0.08);
  check('the top edge makes a row above', (await layout()) === '0 | 1 | 2' && JSON.stringify(await app('panes')) === JSON.stringify([orig[0], orig[2], orig[1]]), [await layout(), await app('panes')]);
  const before = await app('panes');
  s = await size(1);
  await drop(0, 1, s.w * 0.5, s.h * 0.5);
  check('the middle merges the channels', (await paneCount()) === 2 && (await app('panes')).flat().length >= before.flat().length - 0, await app('panes'));

  // the + between rows, and the kinds menu
  await p.click('#addPane');
  const n = await paneCount();
  await p.hover('#stack .rgap', { force: true });
  await p.click('#stack [data-gap="0"]');
  check('the + between rows adds a pane there', (await paneCount()) === n + 1);
  await p.click('#addPaneKinds');
  await p.click('.addkinds [data-addkind="table"]');
  check('Add pane ▾ → Table adds a table pane', (await paneCount()) === n + 2 && (await p.$$('#stack [data-table]')).length >= 1);

  // duplicate beside
  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-pmenu]');
  await p.locator('.panemenu .mi', { hasText: /^Duplicate beside$/ }).click();
  check('Duplicate beside puts the copy right of it', /^0\/1/.test(await layout()), await layout());

  // empty states
  await p.click('#b-more');
  await p.click('.moremenu [data-more="nopanes"]');
  check('no panes: a line of help and a big + Add pane', await visible('[data-nopanes]'));
  await p.click('#addFirst');
  check('+ Add pane adds an empty one that says so', (await paneCount()) === 1 && /Empty pane/.test(await text('#stack [data-pane="0"] .phead')), await text('#stack'));
});
