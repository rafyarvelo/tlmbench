// Explorer's channels: the swatch checkbox (check in the selected pane, colour wherever it's plotted, "pane N" for
// other panes), multi-select (Ctrl/Cmd-click, Shift-click, Ctrl+A, Esc) and the selection bar (Add to pane N, Chart,
// Table, Remove), dragging channels onto a pane or onto empty space, the file right-click menu, Show only these, and
// Undo on Close all files.
import { run } from './lib.js';

const DIR = '/data/flights';
const files = { [`${DIR}/flight.tlm`]: {}, [`${DIR}/hover.csv`]: { channels: ['volts', 'amps'], units: ['V', 'A'] } };

run('channels', async ({ p, check, text, visible, fake, paneCount, until, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const ch = (n) => `#explorer .row.chan[data-ch="${n}"]`;
  await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  await p.dblclick(`#explorer .row.file[data-path$="flight.tlm"]`);
  check('flight.tlm opens with three panes', await until(() => document.querySelectorAll('#explorer .row.chan').length === 6) && (await paneCount()) === 3);

  // categories: channels grouped by the start of their names, with counts and hover actions
  check('channels are grouped by name prefix', (await p.$$eval('#explorer .row.cat .nm', (e) => e.map((x) => x.textContent))).join() === 'accel,motor,other');
  check('a category counts what is plotted', (await p.$$eval('#explorer .row.cat [data-count]', (e) => e.map((x) => x.textContent.trim()))).join() === '2 / 2,2 / 2,2 / 2', await p.$$eval('#explorer .row.cat [data-count]', (e) => e.map((x) => x.textContent)));
  await p.click('#explorer .row.cat[data-cat="motor"] .nm');
  check('clicking a category folds it, saved', (await p.$$('#explorer .row.chan')).length === 4 && (await p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.settings')).exCatClosed)).join() === 'motor');
  await p.click('#explorer .row.cat[data-cat="motor"] .nm');
  await p.hover('#explorer .row.cat[data-cat="other"]');
  const np = await paneCount();
  await p.click('#explorer .row.cat[data-cat="other"] [data-cat-act="chart"]');
  check('a category\'s Chart opens the group in a new pane', (await paneCount()) === np + 1 && (await app('panes')).at(-1).join() === '2,3', await app('panes'));
  await p.keyboard.press('ControlOrMeta+z').catch(() => {});
  await p.click('#stack [data-pane="3"] [data-pmenu]');
  await p.locator('.panemenu .mi', { hasText: /^Close pane$/ }).click();
  check('back to three panes', (await paneCount()) === 3);

  // swatches
  check('a channel in the selected pane has a check', await p.isVisible(`${ch(0)} .box svg`));
  await p.click('#stack [data-pane="1"] .phead');
  check('one in another pane keeps its colour and says where', !(await p.isVisible(`${ch(0)} .box svg`)) && (await p.evaluate((s) => getComputedStyle(document.querySelector(s)).backgroundColor !== 'rgba(0, 0, 0, 0)', `${ch(0)} .box`)) && /pane 1/.test(await text(`${ch(0)} [data-where]`)), await text(ch(0)));

  // drag a channel onto a pane, and onto empty space for a new one
  await p.click('#explorer .row.chan[data-ch="4"] .nm');
  await p.dragAndDrop(`${ch(4)}`, '#stack [data-pane="0"] .plot');
  check('dropping a channel on a pane adds it there', (await app('panes'))[0].includes(4), await app('panes'));
  await p.click('#explorer .row.chan[data-ch="1"] .nm');
  const before = await paneCount();
  await p.dragAndDrop(`${ch(1)}`, '#stack', { targetPosition: { x: 400, y: await p.evaluate(() => document.getElementById('stack').getBoundingClientRect().height - 4) } });
  check('dropping on empty space makes a new pane', (await paneCount()) === before + 1, [before, await paneCount()]);

  // a selected channel gets the bar; Remove takes it out of every pane
  await p.click(`${ch(0)} .nm`);
  check('one channel selected shows its actions', await visible('#selBar') && /accel_x/i.test(await text('#selBar')));
  await p.click('#selBar [data-sel="add"]');
  check('Add to pane N plots it in the selected pane', (await app('panes'))[await app('sel')].includes(0), await app('panes'));
  await p.click('#selBar [data-sel="remove"]');
  check('Remove takes it out of every pane', (await app('panes')).every((c) => !c.includes(0)), await app('panes'));

  // multi-select
  await p.click(`${ch(1)} .nm`);
  await p.click(`${ch(3)} .nm`, { modifiers: ['ControlOrMeta'] });
  check('Ctrl/Cmd-click adds', /2 selected/i.test(await text('#selBar')) && (await p.$$('#explorer .row.chan.sel')).length === 2, await text('#selBar'));
  await p.click(`${ch(5)} .nm`, { modifiers: ['Shift'] });
  check('Shift-click selects the range from the last click', /[345] selected/i.test(await text('#selBar')), await text('#selBar'));
  await p.keyboard.press('Escape');
  check('Esc clears the picked channels', !/selected/i.test(await text('#selBar').catch(() => '') ?? ''));
  await p.focus(ch(2));
  await p.keyboard.press('ControlOrMeta+a');
  check('Ctrl+A picks every channel', /6 selected/i.test(await text('#selBar')), await text('#selBar'));
  const n0 = await paneCount();
  await p.click('#selBar [data-sel="chart"]');
  check('Chart opens them in a new pane', (await paneCount()) === n0 + 1 && (await app('panes')).at(-1).length === 6, await app('panes'));
  await p.click('#selBar [data-sel="table"]');
  check('Table opens them in a new table pane', (await paneCount()) === n0 + 2 && (await p.$$('#stack [data-table]')).length >= 1);
  await p.keyboard.press('Escape');

  // files: right-click, Show only these, Close all with Undo
  await p.click('#explorer .row.file[data-path$="hover.csv"]', { button: 'right' });
  check('a file has a right-click menu', (await p.$$eval('.filemenu .mi', (e) => e.map((x) => x.textContent))).join() === 'Open,Close this file,Close all files');
  await p.click('.filemenu [data-fm="open"]');
  check('Open opens it', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'hover.csv'));
  await p.click('#onlyOpen');
  check('Show only these lists only the open files', (await p.$$('#explorer .row.file')).length === 2 && (await p.getAttribute('#onlyOpen', 'aria-pressed')) === 'true');
  await p.click('#explorer .row.file[data-path$="hover.csv"]', { button: 'right' });
  await p.click('.filemenu [data-fm="closeall"]');
  check('Close all files closes every tab and says so', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 0) && /Closed 2 files/.test(await text('#wsNote')), await text('#wsNote'));
  await p.click('#wsNote button:has-text("Undo")');
  check('Undo reopens them', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2), await text('#wsTabs'));
}, { files, launch: null });
