// A channel dragged from Explorer onto a pane's left or right edge: "Place on the left / right" shows, and the drop
// makes a pane of its own there. A table-only pane has a grip under it that resizes it. The empty stack's button.
import { run } from './lib.js';

const DIR = '/data/flights';
const files = { [`${DIR}/flight.tlm`]: {} };

run('edgedrop', async ({ p, check, until, fake }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  await p.dblclick('#explorer .row.file[data-path$="flight.tlm"]');
  await until(() => document.querySelectorAll('#explorer .row.chan').length === 6);
  const rowBox = await p.locator('#explorer .row.chan').nth(1).locator('.nm').boundingBox();
  const c = { x: rowBox.x + rowBox.width / 2, y: rowBox.y + rowBox.height / 2 };
  const pane = await p.locator('#stack [data-pane="0"]').boundingBox();
  const drag = async (fx, drop) => {
    await p.mouse.move(c.x, c.y);
    await p.mouse.down();
    await p.mouse.move(c.x + 20, c.y + 20, { steps: 3 });
    await p.mouse.move(pane.x + pane.width * fx, pane.y + pane.height / 2, { steps: 6 });
    if (!drop) return;
    await p.mouse.up();
  };
  const n0 = (await app('panes')).length;

  await drag(0.1, false);
  check('over the left edge the pane says where it goes', (await p.getAttribute('#stack [data-pane="0"]', 'data-side')) === 'left');
  check('with "Place on the left"', (await p.evaluate(() => getComputedStyle(document.querySelector('#stack [data-pane="0"]'), '::after').content)).includes('Place on the left'));
  await p.mouse.move(pane.x + pane.width * 0.5, pane.y + pane.height / 2, { steps: 4 });
  check('in the middle it does not', (await p.getAttribute('#stack [data-pane="0"]', 'data-side')) === null);
  await p.mouse.move(pane.x + pane.width * 0.9, pane.y + pane.height / 2, { steps: 4 });
  check('over the right edge it says so', (await p.getAttribute('#stack [data-pane="0"]', 'data-side')) === 'right');
  await p.mouse.up();
  check('the drop makes a pane on the right', (await app('panes')).length === n0 + 1 && (await app('layout'))[1] === 'beside', await app('layout'));

  const pane1 = await p.locator('#stack [data-pane="0"]').boundingBox();
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 20, c.y + 20, { steps: 3 });
  await p.mouse.move(pane1.x + pane1.width * 0.05, pane1.y + pane1.height / 2, { steps: 6 });
  await p.mouse.up();
  const lay = await app('layout');
  check('and on the left a pane before it', (await app('panes')).length === n0 + 2 && lay[0] === 'row' && lay[1] === 'beside' && lay[2] === 'beside', lay);

  // the last channel dragged out of a pane takes the pane with it
  const nBefore = (await app('panes')).length;
  const chip = await p.locator('#stack [data-pane="0"] .chip').first().boundingBox();
  const dst = await p.locator('#stack [data-pane="2"]').boundingBox();
  await p.mouse.move(chip.x + 8, chip.y + chip.height / 2);
  await p.mouse.down();
  await p.mouse.move(chip.x + 30, chip.y + 30, { steps: 3 });
  await p.mouse.move(dst.x + dst.width / 2, dst.y + dst.height / 2, { steps: 6 });
  await p.mouse.up();
  const after = await app('panes');
  check('moving a pane\'s only channel away removes that pane', after.length === nBefore - 1 && after.every((x) => x.length), after);

  // a table-only pane has a grip
  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.locator('.viewmenu .mi', { hasText: /^Table/ }).first().click();
  await until(() => !!document.querySelector('#stack [data-pane="0"] .tgrip'));
  const h = () => p.evaluate(() => Math.round(document.querySelector('#stack [data-pane="0"] .ptable').getBoundingClientRect().height));
  const h0 = await h();
  const g = await p.locator('#stack [data-pane="0"] .tgrip').boundingBox();
  await p.mouse.move(g.x + g.width / 2, g.y + g.height / 2);
  await p.mouse.down();
  await p.mouse.move(g.x + g.width / 2, g.y + g.height / 2 + 60, { steps: 4 });
  await p.mouse.up();
  check('the table pane grows when its grip is dragged down', (await h()) === h0 + 60, [h0, await h()]);
  await p.dblclick('#stack [data-pane="0"] .tgrip');
  check('double-click resets it', (await h()) === h0, await h());
}, { files, launch: null });
