// Table columns: the Columns list (tick to show or hide, Show all / Hide all, a name goes to its column), Autofit
// (to contents, to headers, reset), the ▾ menu on each header (sort, autofit, select, hide), selecting columns (Chart,
// Table, Hide), and Sample and Time pinned while the others scroll. Hidden columns and widths are saved with the layout.
import { run } from './lib.js';

run('tablecols', async ({ p, check, text, until, settingsLS }) => {
  const T = '#stack [data-pane="0"] [data-table]';
  const heads = () => p.evaluate((t) => [...document.querySelectorAll(`${t} .thead [data-col]`)].map((e) => e.dataset.col), T);
  const cellsOf = () => p.evaluate((t) => document.querySelectorAll(`${t} [data-row] > span`).length / Math.max(1, document.querySelectorAll(`${t} [data-row]`).length), T);
  const layout = async () => (await settingsLS()).layouts?.['/data/flight.tlm']?.[0] ?? {};
  const nHeads = () => document.querySelectorAll('[data-table] .thead [data-col]').length;

  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="table"]');
  check('every column to start with', await until(() => document.querySelectorAll('[data-table] .thead [data-col]').length === 4), await heads());

  // pinned
  const pins = await p.evaluate((t) => [...document.querySelectorAll(`${t} .thead .tth.pin`)].map((e) => getComputedStyle(e).position), T);
  check('Sample and Time are pinned', pins.length === 2 && pins.every((x) => x === 'sticky'), pins);

  // the Columns list
  await p.click(`${T} [data-cols]`);
  check('the list names the channels', (await p.$$('.colpop [data-colrow]')).length === 2);
  await p.click('.colpop [data-col="1"]');
  check('unticking a channel takes its column out', await until(() => document.querySelectorAll('[data-table] .thead [data-col]').length === 3), await heads());
  check('the rows lose the cell too', await until(() => document.querySelectorAll('[data-table] [data-row] > span').length % 3 === 0), await cellsOf());
  check('the button counts what is hidden', (await text(`${T} [data-cols]`)).includes('1 hidden'), await text(`${T} [data-cols]`));
  check('the chart still has both channels', (await p.evaluate(() => window.__tlmApp.panes()[0].length)) === 2);
  check('saved with the layout', JSON.stringify((await layout()).hideCols) === '[1]', await layout());
  await p.fill('.colpop [data-q]', 'accel_x');
  check('the filter narrows the list', (await p.$$('.colpop [data-colrow]')).length === 1);
  await p.fill('.colpop [data-q]', '');
  await p.click('.colpop [data-none]');
  check('Hide all leaves Sample and Time', await until(() => document.querySelectorAll('[data-table] .thead [data-col]').length === 2), await heads());
  await p.click('.colpop [data-colgo="1"]');
  check('a name shows its column again', await until(() => document.querySelectorAll('[data-table] .thead [data-col]').length === 3), await heads());
  await p.click('.colpop [data-all]');
  check('Show all', (await until(() => document.querySelectorAll('[data-table] .thead [data-col]').length === 4)) && !(await layout()).hideCols, await heads());
  await p.click('.colpop [data-done]');
  check('Done closes it', !(await p.$('.colpop')));

  // Autofit
  const widths = async () => (await layout()).widths ?? null;
  await p.click(`${T} [data-autofit]`);
  await p.click('.colmenu [data-af="headers"]');
  check('Fit all to headers saves a width for each column', await until(async () => true) && Object.keys((await widths()) ?? {}).length === 4, await widths());
  await p.click(`${T} [data-autofit]`);
  await p.click('.colmenu [data-af="reset"]');
  check('Reset widths forgets them', (await widths()) == null, await widths());
  await p.click(`${T} [data-autofit]`);
  await p.click('.colmenu [data-af="contents"]');
  const body = await widths();
  check('Fit all to contents too', body && body.c0 >= 60 && body.c0 <= 480, body);

  // the header ▾ menu
  await p.hover(`${T} [data-col="c0"]`);
  await p.click(`${T} [data-colmenu="c0"]`);
  await p.click('.colmenu [data-hm="desc"]');
  check('Sort descending sorts it', await until(() => document.querySelector('[data-table] [data-col="c0"]')?.getAttribute('aria-sort') === 'descending'));
  await p.click(`${T} [data-colmenu="c0"]`);
  check('and ticks the sort it is in', (await p.getAttribute('.colmenu [data-hm="desc"]', 'aria-checked')) === 'true');
  await p.click('.colmenu [data-hm="clear"]');
  check('Clear sort', await until(() => document.querySelector('[data-table] [data-col="c0"]')?.getAttribute('aria-sort') === 'none'));

  // selecting columns
  check('no selection bar yet', !(await p.$(`${T} [data-selbar]`)));
  await p.click(`${T} [data-colmenu="c0"]`);
  await p.click('.colmenu [data-hm="select"]');
  check('Select this column shows the bar', (await text(`${T} [data-selbar]`)).includes('1 selected'), await text(`${T} [data-selbar]`));
  check('the column is marked', !!(await p.$(`${T} .thead .tth.sel[data-col="c0"]`)));
  await p.click(`${T} [data-sc="table"]`);
  check('Table opens the column in a new table pane', await until(() => document.querySelectorAll('#stack section.pane').length === 4) && (await p.evaluate(() => window.__tlmApp.modes().at(-1))) === 'table', await p.evaluate(() => window.__tlmApp.modes()));
  check('and the selection is done', !(await p.$(`${T} [data-selbar]`)));
  await p.click(`${T} [data-col="c1"] .thtop`, { modifiers: [process.platform === 'darwin' ? 'Meta' : 'Control'] });
  check('Ctrl/⌘-click on a header selects too', (await text(`${T} [data-selbar]`)).includes('1 selected'));
  await p.click(`${T} [data-sc="hide"]`);
  check('Hide takes the selected columns out', (await until(() => document.querySelectorAll('#stack [data-pane="0"] [data-table] .thead [data-col]').length === 3)) && JSON.stringify((await layout()).hideCols) === '[1]', await layout());
  await p.click(`${T} [data-colmenu="c0"]`);
  await p.click('.colmenu [data-hm="hide"]');
  check('Hide this column', await until(() => document.querySelectorAll('#stack [data-pane="0"] [data-table] .thead [data-col]').length === 2));

  // Click a column header to sort: off, a click selects (Shift a range, Ctrl/⌘ adds) and sorting is in the ▾ menu
  await p.click(`${T} [data-cols]`);
  await p.click('.colpop [data-all]');
  await p.click('.colpop [data-done]');
  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]');
  await p.uncheck('[data-set="tableClickSort"]');
  await p.click('#b-settings');
  check('the setting is saved', (await settingsLS()).tableClickSort === false);
  await p.click(`${T} [data-col="c0"] .thtop`);
  check('a click selects the column and does not sort', (await text(`${T} [data-selbar]`)).includes('1 selected') && (await p.getAttribute(`${T} [data-col="c0"]`, 'aria-sort')) === 'none');
  await p.click(`${T} [data-col="c1"] .thtop`, { modifiers: ['Shift'] });
  check('Shift-click selects a range', (await text(`${T} [data-selbar]`)).includes('2 selected'), await text(`${T} [data-selbar]`));
  await p.click(`${T} [data-sc="clear"]`);
  check('× clears the selection', !(await p.$(`${T} [data-selbar]`)));
});
