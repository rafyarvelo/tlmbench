// A pane's chips: × takes a channel out, dragging by the name reorders (onto another chip) or moves (onto a pane; Alt,
// or the platform's copy key, ⌘ on the Mac and Ctrl elsewhere, copies), "· L" / "· R" where two units share a pane, and "+N" with the list when the header can't fit them all.
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
  // the platform's copy key too: ⌘ on the Mac, Ctrl elsewhere (owner, Nov 14; sheet C1), with a "+" while it is down
  const KEY = await p.evaluate(() => (/Mac/.test(navigator.platform) ? 'Meta' : 'Control'));
  await p.hover(chip(0, 2));
  await p.click(`${chip(0, 2)} [data-chip-x]`);
  await until(() => !window.__tlmApp.panes()[0].includes(2));
  const from = await p.locator(chip(1, 2)).boundingBox();
  const to = await p.locator('#stack [data-pane="0"] .plot').boundingBox();
  await p.mouse.move(from.x + 8, from.y + 8);
  await p.mouse.down();
  await p.mouse.move(from.x + 30, from.y + 30, { steps: 4 });
  await p.keyboard.down(KEY);
  check(`holding ${KEY} shows a "+" on the drag`, await until(() => document.documentElement.classList.contains('dnd-copy')));
  await p.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 6 });
  await p.mouse.up();
  await p.keyboard.up(KEY);
  check(`${KEY} copies it too`, await until(() => { const ps = window.__tlmApp.panes(); return ps[0].includes(2) && ps[1].includes(2); }), await app('panes'));
  check('the "+" goes when the drag ends', !(await p.evaluate(() => document.documentElement.classList.contains('dnd-copy'))));

  // onto empty space: a new pane (the same view with the setting)
  const n0 = (await app('panes')).length;
  await p.dragAndDrop(chip(1, 3), '#stack', { force: true, targetPosition: { x: 400, y: await p.evaluate(() => document.getElementById('stack').getBoundingClientRect().height - 4) } });
  check('a chip dropped on empty space makes a new pane', (await app('panes')).length === n0 + 1 && !(await app('panes'))[1].includes(3) && JSON.stringify((await app('panes')).at(-1)) === '[3]', await app('panes'));

  await p.click('#stack [data-pane="0"] .phead');

  // two units in one pane: L and R
  check('two units: the chips say which side', (await p.$$eval('#stack [data-pane="0"] [data-side]', (e) => e.map((x) => x.textContent.replace(/\s+/g, ' ').trim()))).join() === '· L,· R', await text('#stack [data-pane="0"] .phead'));

  // overflow
  for (const ch of [3, 4, 5]) await p.click(`#explorer .row.chan[data-ch="${ch}"] .box`);
  await p.setViewportSize({ width: 700, height: 800 });
  await wait(500);
  check('a header that cannot fit them says "+N"', await until(() => !!document.querySelector('#stack [data-pane="0"] [data-more-chips]')), await text('#stack [data-pane="0"] .phead'));
  await p.click('#stack [data-pane="0"] [data-more-chips]');
  check('and lists every channel', (await p.$$('.chanlist .mi')).length === 5, await text('.chanlist'));
  // its channels drag as chips do (owner's V2 note, Nov 16: the prototype's +N list lets them be dragged)
  const before = await p.evaluate(() => window.__tlmApp.panes());
  const moved = await p.evaluate(() => {
    const items = document.querySelectorAll('.chanlist .mi');
    const src = items[items.length - 1];
    const ch = Number(src.dataset.listch);
    const dt = new DataTransfer();
    src.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: dt }));
    return new Promise((res) =>
      setTimeout(() => {
        const faded = document.querySelector('.chanlist')?.classList.contains('listdrag');
        const pane = document.querySelector('#stack section.pane[data-pane="1"]');
        const r = pane.getBoundingClientRect();
        const at = document.elementFromPoint(r.x + r.width / 2, r.y + r.height * 0.6);
        const ev = { bubbles: true, cancelable: true, dataTransfer: dt, clientX: r.x + r.width / 2, clientY: r.y + r.height * 0.6 };
        at.dispatchEvent(new DragEvent('dragover', ev));
        at.dispatchEvent(new DragEvent('drop', ev));
        src.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer: dt }));
        res({ ch, faded });
      }, 80),
    );
  });
  const after = await p.evaluate(() => window.__tlmApp.panes());
  check('a channel of the +N list drags to another pane (the list fades while it does)', moved.faded && after[1].includes(moved.ch) && !after[0].includes(moved.ch) && before[0].includes(moved.ch), [moved, before, after]);
  check('and the list closes when the drag ends', await until(() => !document.querySelector('.chanlist')));
  await p.keyboard.press('Escape');
  await p.setViewportSize({ width: 1440, height: 900 });
  await wait(500);
  check('wide again: no "+N"', !(await visible('#stack [data-pane="0"] [data-more-chips]')));
});
