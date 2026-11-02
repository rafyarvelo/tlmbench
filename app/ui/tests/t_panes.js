// The pane stack: the ⋯ menu (and right-click) closes, duplicates and splits panes, with Undo; panes split into
// columns side by side and each draws; a pane moves by dragging its number; the layout comes back after a reload.
import { run } from './lib.js';

run('panes', async ({ p, check, text, visible, paneCount, until, settingsLS, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const menu = async (i) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-pmenu]`);
    return p.$$eval('.panemenu .mi', (b) => b.map((x) => (x.disabled ? '(' + x.textContent + ')' : x.textContent)));
  };
  const pick = (label) => p.locator('.panemenu .mi', { hasText: new RegExp(`^${label}$`) }).click();
  const rect = (i) => p.evaluate((i) => { const r = document.querySelector(`#stack [data-pane="${i}"] .plot`).getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; }, i);

  check('three panes', (await paneCount()) === 3);
  const items = await menu(0);
  check('the ⋯ menu', items.join('|') === 'Close others|Close panes below|Close pane|Duplicate|Split right|Split down|Add a pane below', items);
  await p.keyboard.press('Escape');
  check('Esc closes it', !(await visible('.panemenu')));

  // split right: two columns in the first row, each drawn
  await menu(0);
  await pick('Split right');
  check('split right adds a pane beside', (await paneCount()) === 4 && JSON.stringify(await app('layout')) === '["row","beside","row","row"]', await app('layout'));
  const [a, b] = [await rect(0), await rect(1)];
  check('side by side', Math.abs(a.y - b.y) < 2 && b.x > a.x + a.w, [a, b]);
  check('the new pane is selected and empty', (await app('sel')) === 1 && /Tick channels/.test(await text('#stack [data-pane="1"] .phead')));
  await p.click('#chans .chan[data-ch="4"]');
  check('a channel goes to the selected pane', JSON.stringify((await app('panes'))[1]) === '[4]', await app('panes'));
  check('every pane draws', await until(() => window.__tlmApp.drawn().filter((d, i) => window.__tlmApp.panes()[i].length).every((d) => d && d.length)));

  // split down within the column, duplicate
  await menu(1);
  await pick('Split down');
  const [c1, c2] = [await rect(1), await rect(2)];
  check('split down stacks in the column', (await paneCount()) === 5 && Math.abs(c1.x - c2.x) < 2 && c2.y > c1.y, [c1, c2]);
  await menu(0);
  await pick('Duplicate');
  check('duplicate copies the channels', JSON.stringify((await app('panes'))[1]) === JSON.stringify((await app('panes'))[0]), await app('panes'));

  // close pane, with Undo
  const before = JSON.stringify(await app('panes'));
  await menu(1);
  await pick('Close pane');
  check('close pane', (await paneCount()) === 5 && /Closed 1 pane/.test(await text('#wsNote')), await text('#wsNote'));
  await p.click('#wsNote button:has-text("Undo")');
  check('undo brings it back', JSON.stringify(await app('panes')) === before, await app('panes'));

  // right-click on a pane's title (not a chip) opens its menu; close others, then undo
  const head = await p.$eval('#stack [data-pane="0"] .phead', (e) => e.getBoundingClientRect().width);
  await p.click('#stack [data-pane="0"] .phead', { button: 'right', position: { x: head - 80, y: 10 } });
  check('right-click opens the pane menu', await until(() => !!document.querySelector('.panemenu'), null, 2000));
  await pick('Close others');
  check('close others', (await paneCount()) === 1 && /Closed 5 panes/.test(await text('#wsNote')), await text('#wsNote'));
  await p.click('#wsNote button:has-text("Undo")');
  check('undone', (await paneCount()) === 6);
  check('a chip right-click is still the channel menu', await (async () => {
    await p.click('#stack [data-pane="0"] .chip[data-ch="0"]', { button: 'right' });
    const ok = (await visible('.menu [data-gap]')) && !(await visible('.panemenu'));
    await p.keyboard.press('Escape');
    return ok;
  })());

  // drag pane 6 by its number onto pane 1: it moves first
  const last = JSON.stringify((await app('panes'))[5]);
  await p.locator('#stack [data-pane="5"] .pnum').dragTo(p.locator('#stack [data-pane="0"] .phead'));
  await wait(200);
  check('dragging a pane number moves it', JSON.stringify((await app('panes'))[0]) === last, await app('panes'));

  // time labels on each column's bottom pane of the last row
  const pb = await p.$$eval('#stack .plot', (els) => els.map((e) => getComputedStyle(e).getPropertyValue('--pb').trim()));
  check('time labels under the bottom panes', pb[pb.length - 1] !== '4px' && pb[0] === '4px', pb);

  // the layout is saved, columns included, and comes back
  const layout = JSON.stringify(await app('layout'));
  check('saved', (await settingsLS()).layouts['/data/flight.tlm'].length === 6);
  await p.reload();
  await wait(800);
  check('it comes back after a reload', JSON.stringify(await app('layout')) === layout && (await paneCount()) === 6, await app('layout'));
});
