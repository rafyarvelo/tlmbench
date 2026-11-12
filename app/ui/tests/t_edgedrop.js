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

  // a pane placed into a row takes its height, and a row holds four
  const gp = await p.locator('#stack [data-pane="0"] .hgrip').boundingBox();
  await p.mouse.move(gp.x + gp.width / 2, gp.y + 3);
  await p.mouse.down();
  await p.mouse.move(gp.x + gp.width / 2, gp.y + 43, { steps: 4 });
  await p.mouse.up();
  const hs = () => p.evaluate(() => [...document.querySelectorAll('#stack .plot')].map((e) => Math.round(e.getBoundingClientRect().height)));
  const taller = await hs();
  check('the row was made taller', taller.slice(0, 3).every((x) => x === 190), taller);
  const last = await p.locator('#stack [data-pane="2"]').boundingBox();
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 20, c.y + 20, { steps: 3 });
  await p.mouse.move(last.x + last.width * 0.95, last.y + last.height / 2, { steps: 6 });
  await p.mouse.up();
  const four = await hs();
  check('a fourth pane fits in the row and takes its height', (await app('layout')).slice(0, 4).join() === 'row,beside,beside,beside' && four.slice(0, 4).every((x) => x === 190), [await app('layout'), four]);
  const mid = await p.locator('#stack [data-pane="1"]').boundingBox();
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 20, c.y + 20, { steps: 3 });
  await p.mouse.move(mid.x + mid.width * 0.5, mid.y + mid.height / 2, { steps: 6 });
  check('a fifth has no room: no edge zone', (await p.getAttribute('#stack [data-pane="1"]', 'data-side')) === null);
  await p.keyboard.press('Escape');
  await p.mouse.up();

  // while a channel is dragged a drop zone shows above the top pane; dropping there makes a pane of its own at the top
  const top0 = (await app('panes'))[0].join();
  const nTop = (await app('panes')).length;
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 20, c.y + 20, { steps: 3 });
  check('the zones show while dragging', (await p.locator('#stack [data-dropgap]').count()) >= 2);
  const gap = await p.locator('#stack [data-dropgap="0"]').boundingBox();
  await p.mouse.move(gap.x + gap.width / 2, gap.y + gap.height / 2, { steps: 6 });
  check('and the one above the top pane lights up', await p.locator('#stack [data-dropgap="0"].over').count() === 1);
  await p.mouse.up();
  const lay2 = await app('layout');
  check('dropping there adds a pane at the top, in a row of its own', (await app('panes')).length === nTop + 1 && lay2[0] === 'row' && lay2[1] === 'row' && (await app('panes'))[1].join() === top0, [await app('panes'), lay2]);
  check('and the zones go away', (await p.locator('#stack [data-dropgap]').count()) === 0);

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
