// Table column headers drag like channel chips (the prototype's .tth[draggable]; sheet "Other" #1, a blocker): a header
// dropped on another pane moves its channel there, Alt copies it, a header dropped on another header reorders the pane's
// channels, and the column's resize edge still resizes instead of dragging. Sample and Time don't drag.
import { run } from './lib.js';

run('tabledrag', async ({ p, check, until }) => {
  const T = '#stack [data-pane="0"] [data-table]';
  const panes = () => p.evaluate(() => window.__tlmApp.panes());
  const centre = async (sel) => {
    const b = await p.locator(sel).first().boundingBox();
    return { x: b.x + b.width / 2, y: b.y + Math.min(14, b.height / 2) };
  };
  const drag = async (from, to, mods = []) => {
    const a = await centre(from);
    for (const m of mods) await p.keyboard.down(m);
    await p.mouse.move(a.x, a.y);
    await p.mouse.down();
    await p.mouse.move(a.x + 20, a.y + 20, { steps: 4 });
    await p.waitForTimeout(150);
    // measured once the drag is under way: the drop gaps it opens move the panes
    const b = await centre(to);
    await p.mouse.move(b.x, b.y, { steps: 8 });
    await p.mouse.up();
    for (const m of mods) await p.keyboard.up(m);
  };

  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="table"]');
  check('pane 1 is a table of its two channels', await until((t) => document.querySelectorAll(`${t} .thead [data-col]`).length === 4, T));
  const start = await panes();
  const [a, b] = start[0];
  check('channel columns drag; Sample and Time don\'t', await p.evaluate((t) => [...document.querySelectorAll(`${t} .thead .tth`)].map((e) => e.getAttribute('draggable') ?? '-').join(), T) === '-,-,true,true');

  // onto another pane: the channel moves there
  await drag(`${T} .tth[data-col="c${a}"] .tlab`, '#stack [data-pane="1"] .plot');
  check('a header dropped on another pane moves its channel there', await until((ch) => { const ps = window.__tlmApp.panes(); return ps.length === 3 && !ps[0].includes(ch) && ps[1].includes(ch); }, a), await panes());

  // Alt copies
  await drag(`${T} .tth[data-col="c${b}"] .tlab`, '#stack [data-pane="2"] .plot', ['Alt']);
  check('with Alt it copies (the table keeps the column)', await until((ch) => { const ps = window.__tlmApp.panes(); return ps[0].includes(ch) && ps[2].includes(ch); }, b), await panes());

  // back into the table, then reorder by dropping on a header
  await drag(`#stack [data-pane="1"] .chip[data-ch="${a}"]`, `${T} .thead`);
  await until((ch) => window.__tlmApp.panes()[0].includes(ch), a);
  const before = (await panes())[0];
  await drag(`${T} .tth[data-col="c${before[1]}"] .tlab`, `${T} .tth[data-col="c${before[0]}"] .tlab`);
  check('a header dropped on another header reorders the pane\'s channels', await until((want) => window.__tlmApp.panes()[0].join() === want, [before[1], before[0]].join()), [before, (await panes())[0]]);

  // the resize edge resizes, not drags
  const w0 = await p.$eval(`${T} .tth[data-col="c${b}"]`, (e) => e.getBoundingClientRect().width);
  const edge = await p.locator(`${T} .tth[data-col="c${b}"] .trsz`).boundingBox();
  const panesBefore = JSON.stringify(await panes());
  await p.mouse.move(edge.x + edge.width / 2, edge.y + 10);
  await p.mouse.down();
  await p.mouse.move(edge.x + 60, edge.y + 10, { steps: 6 });
  await p.mouse.up();
  const w1 = await p.$eval(`${T} .tth[data-col="c${b}"]`, (e) => e.getBoundingClientRect().width);
  check('the resize edge still resizes the column', w1 > w0 + 30, [w0, w1]);
  check('and moves no channel', JSON.stringify(await panes()) === panesBefore);
});
