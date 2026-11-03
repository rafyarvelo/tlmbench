// The chart height grip under a chart: drag to resize (side-by-side panes follow), double-click resets, ↑ ↓ step,
// saved with the layout.
import { run } from './lib.js';

run('height', async ({ p, check, wait, settingsLS, until, paneCount }) => {
  const h = (i) => p.evaluate((i) => Math.round(document.querySelectorAll('#stack .plot')[i].getBoundingClientRect().height), i);
  const grip = (i) => `#stack [data-pane="${i}"] .hgrip`;
  check('plots start at the fixed height', (await h(0)) === 150, await h(0));
  const g = await p.evaluate((s) => { const r = document.querySelector(s).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + 3 }; }, grip(0));
  await p.mouse.move(g.x, g.y);
  await p.mouse.down();
  await p.mouse.move(g.x, g.y - 40, { steps: 4 });
  await p.mouse.up();
  check('dragging the grip up shortens the chart', (await h(0)) === 110, await h(0));
  check('and it is saved with the layout', (await settingsLS()).layouts['/data/flight.tlm'][0].h === 110);
  check('the other panes keep theirs', (await h(1)) === 150);
  await p.focus(grip(0));
  await p.keyboard.press('ArrowDown');
  check('↓ steps it taller', (await h(0)) === 120, await h(0));
  await p.dblclick(grip(0));
  check('double-click resets it', (await h(0)) === 150 && (await settingsLS()).layouts['/data/flight.tlm'][0].h === undefined, await h(0));

  // panes side by side share a height
  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-pmenu]');
  await p.locator('.panemenu .mi', { hasText: /^Split right$/ }).click();
  await until(() => document.querySelectorAll('#stack .plot').length === 4);
  const g2 = await p.evaluate((s) => { const r = document.querySelector(s).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + 3 }; }, grip(0));
  await p.mouse.move(g2.x, g2.y);
  await p.mouse.down();
  await p.mouse.move(g2.x, g2.y - 30, { steps: 4 });
  await p.mouse.up();
  check('a pane beside it follows', (await h(0)) === 120 && (await h(1)) === 120, [await h(0), await h(1)]);
});
