// The owner's two blockers (Nov 22): a chip dropped on its own pane's left or right edge moves to a new pane there (it
// leaves the old one; the copy key copies), and closing one of two panes side by side leaves the other in its row, wider,
// whichever of the pair closes and however it goes (Close pane, its last chip dragged away, dropped onto another pane).
import { run } from './lib.js';

run('rowkeep', async ({ p, check, until }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const set = (ps) => p.evaluate((ps) => window.__tlmApp.setPanes(ps), ps);
  const rect = (i) => p.evaluate((i) => { const r = document.querySelector(`#stack [data-pane="${i}"]`).getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width }; }, i);
  const chipDrag = async (pane, k, to, fx, copy = false) => {
    const chip = await p.locator(`#stack [data-pane="${pane}"] .chip`).nth(k).boundingBox();
    await p.mouse.move(chip.x + 8, chip.y + chip.height / 2);
    await p.mouse.down();
    await p.mouse.move(chip.x + 30, chip.y + 30, { steps: 3 });
    // the drop gaps that show while dragging move the panes: measure the target now
    const dst = await p.locator(`#stack [data-pane="${to}"]`).boundingBox();
    if (copy) await p.keyboard.down('Alt');
    await p.mouse.move(dst.x + dst.width * fx, dst.y + dst.height / 2, { steps: 6 });
    await p.mouse.up();
    if (copy) await p.keyboard.up('Alt');
  };
  const menuClose = async (i) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-pmenu]`);
    await p.locator('.panemenu .mi', { hasText: /^Close pane$/ }).click();
  };
  const fourRows = [{ channels: [0] }, { channels: [1] }, { channels: [2], beside: true }, { channels: [3] }];

  // blocker 1: a pane of two channels, a chip dropped on its own right edge, then its own left edge
  await set([{ channels: [0, 1] }, { channels: [2] }]);
  await until(() => document.querySelectorAll('#stack [data-pane="0"] .chip').length === 2);
  await chipDrag(0, 1, 0, 0.92);
  check('dropped on its own pane\'s right edge, the channel moves to a new pane there', JSON.stringify(await app('panes')) === '[[0],[1],[2]]' && (await app('layout')).join() === 'row,beside,row', [await app('panes'), await app('layout')]);
  await set([{ channels: [0, 1] }, { channels: [2] }]);
  await until(() => document.querySelectorAll('#stack [data-pane="0"] .chip').length === 2);
  await chipDrag(0, 0, 0, 0.06);
  check('on its left edge too: a new pane before it, the channel gone from the old one', JSON.stringify(await app('panes')) === '[[0],[1],[2]]' && (await app('layout')).join() === 'row,beside,row', [await app('panes'), await app('layout')]);
  await set([{ channels: [0, 1] }, { channels: [2] }]);
  await until(() => document.querySelectorAll('#stack [data-pane="0"] .chip').length === 2);
  await chipDrag(0, 1, 0, 0.92, true);
  check('with the copy key it stays in the old pane as well', JSON.stringify(await app('panes')) === '[[0,1],[1],[2]]', await app('panes'));
  await set([{ channels: [0, 1] }, { channels: [2] }]);
  await until(() => document.querySelectorAll('#stack [data-pane="1"] .chip').length === 1);
  await chipDrag(1, 0, 0, 0.92);
  check('a pane\'s only chip dropped on another pane\'s edge moves there and takes its pane with it', JSON.stringify(await app('panes')) === '[[0,1],[2]]' && (await app('layout')).join() === 'row,beside', [await app('panes'), await app('layout')]);

  // blocker 2: one on top, two side by side, one at the bottom; close either of the pair
  for (const [k, left] of [[1, 2], [2, 1]]) {
    await set(fourRows);
    await until(() => document.querySelectorAll('#stack [data-pane]').length === 4);
    const top = await rect(0);
    await menuClose(k);
    check(`closing pane ${k + 1} of the pair keeps the other in its row`, (await app('layout')).join() === 'row,row,row' && JSON.stringify((await app('panes'))[1]) === `[${left}]`, [await app('layout'), await app('panes')]);
    const r = await rect(1);
    check('and it grows across the row', r.y > top.y + 20 && Math.abs(r.w - top.w) < 2, [r, top]);
    // the owner's Y1 note (Nov 23): Undo after closing the left one put the pair back as two rows
    await p.mouse.click(5, 5);
    await p.keyboard.press('ControlOrMeta+z');
    check(`⌘Z after closing pane ${k + 1} puts the pair back side by side`, (await app('layout')).join() === 'row,row,beside,row' && JSON.stringify(await app('panes')) === '[[0],[1],[2],[3]]', [await app('layout'), await app('panes')]);
    await p.keyboard.press('ControlOrMeta+Shift+z');
    check('and ⇧⌘Z closes it again, the other still in its row', (await app('layout')).join() === 'row,row,row', await app('layout'));
  }
  // the same when the left one goes because its only chip was dragged onto the bottom pane
  await set(fourRows);
  await until(() => document.querySelectorAll('#stack [data-pane]').length === 4);
  await chipDrag(1, 0, 3, 0.5);
  check('dragging away the only chip of the left one keeps the right one in its row', (await app('layout')).join() === 'row,row,row' && JSON.stringify(await app('panes')) === '[[0],[2],[3,1]]', [await app('layout'), await app('panes')]);
  // and a column under a closed pane's place keeps it
  await set([{ channels: [0] }, { channels: [1] }, { channels: [4], under: true }, { channels: [2], beside: true }, { channels: [3] }]);
  await until(() => document.querySelectorAll('#stack [data-pane]').length === 5);
  await menuClose(1);
  check('closing a column\'s top pane: the one under it takes its place, beside the pane next to it', (await app('layout')).join() === 'row,row,beside,row', await app('layout'));
  await menuClose(1);
  check('then closing that: the right one widens in its row', (await app('layout')).join() === 'row,row,row', await app('layout'));
});
