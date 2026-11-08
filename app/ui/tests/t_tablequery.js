// Table sort and filter: the header sorts (click: ascending, descending, off; Shift adds a column), the filter row
// takes the core's filter language, both run in the core over every row in view and the table pages through the
// result; unreadable filters are marked and ignored; Clear sort and filters; sort and filters saved with the layout.
import { run } from './lib.js';

run('tablequery', async ({ p, check, text, until, settingsLS, wait, fake }) => {
  const T = '#stack [data-pane="0"] [data-table]';
  const stat = () => text(`${T} [data-tstat]`);
  const cell = (r, c) => text(`${T} [data-row="${r}"] span:nth-child(${c})`);
  // the fake core's accel_x (channel 0) at sample i, 1000 Hz
  const x = (i) => Math.fround(Math.sin((i / 1000) * 0.05) + 0.3 * Math.sin((i / 1000) * 1.7));
  const N = 200000;
  let over = 0, max = -Infinity, argmax = 0, min = Infinity;
  for (let i = 0; i < N; i++) {
    const v = x(i);
    if (v > 1.2) over++;
    if (v > max) [max, argmax] = [v, i];
    min = Math.min(min, v);
  }

  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="table"]');
  check('unsorted: every row', await until(() => /200,000 rows in view/.test(document.querySelector('[data-table] [data-tstat]').textContent)), await stat());
  check('the rows on screen show as a band on the charts', await until(() => window.__tlmApp.bands().length === 1), await p.evaluate(() => window.__tlmApp.bands()));
  const band = await p.evaluate(() => window.__tlmApp.bands()[0]);
  check('from the first row to the last one showing', band[0] === 0 && BigInt(band[2]) > BigInt(band[1]), band);
  check('a filter box under each name but the sample column', (await p.$$(`${T} .tfin`)).length >= 2 && !(await p.$(`${T} [data-col="n"] .tfin`)));
  check('no Clear button yet', !(await p.$(`${T} [data-clear]`)));

  // filter
  await p.fill(`${T} [data-filter="c0"]`, '> 1.2');
  check('the filter leaves the matching rows', await until((n) => document.querySelector('[data-table] [data-tstat]').textContent.split(' · ')[0].replace(/\D/g, '') === n, `${over}200000`), [await stat(), over]);
  check('it says how many of the rows in view', new RegExp(`^${over.toLocaleString()} of 200,000 rows in view`).test(await stat()), await stat());
  check('every row shown passes', await until(() => { const r = [...document.querySelectorAll('[data-table] [data-row] span:nth-child(3)')]; return r.length > 3 && r.every((c) => parseFloat(c.textContent) > 1.19); }));
  check('and the core was asked once for it', (await fake(() => window.__tlmFake.queriesRun.at(-1))).filters[0].expr === '> 1.2');
  check('Clear sort and filters appears', !!(await p.$(`${T} [data-clear]`)));

  // an unreadable filter is marked and ignored
  await p.fill(`${T} [data-filter="c1"]`, 'abc');
  check('an unreadable filter is marked', await until(() => document.querySelector('[data-table] [data-filter="c1"]').classList.contains('bad')));
  check('with the reason', (await p.getAttribute(`${T} [data-filter="c1"]`, 'data-tip')) === 'can\'t read "abc"', await p.getAttribute(`${T} [data-filter="c1"]`, 'data-tip'));
  check('the rows stay as the good filter left them', /of 200,000 rows in view/.test(await stat()), await stat());
  await p.fill(`${T} [data-filter="c1"]`, '');

  // sort: ascending, descending, off
  await p.fill(`${T} [data-filter="c0"]`, '');
  check('no filter: every row again', await until(() => /^200,000 rows in view/.test(document.querySelector('[data-table] [data-tstat]').textContent)), await stat());
  const head = `${T} [data-col="c0"]`;
  await p.click(`${head} .thtop`);
  check('click sorts ascending', (await p.getAttribute(head, 'aria-sort')) === 'ascending');
  check('the smallest value is first', await until((m) => Math.abs(parseFloat(document.querySelector('[data-table] [data-row="0"] span:nth-child(3)')?.textContent) - m) < 0.01, min), await cell(0, 3));
  await p.click(`${head} .thtop`);
  check('again, descending', (await p.getAttribute(head, 'aria-sort')) === 'descending');
  check('the largest is first, with its sample number', await until((a) => document.querySelector('[data-table] [data-row="0"] span')?.textContent === a, argmax.toLocaleString()), [await cell(0, 1), argmax]);
  check('the status line counts all rows', /^200,000 rows in view/.test(await stat()) || /of 200,000/.test(await stat()), await stat());
  const ls = (await settingsLS()).layouts['/data/flight.tlm'][0];
  check('the sort is saved with the layout', JSON.stringify(ls.sort) === '[{"col":"c0","desc":true}]', ls);
  await p.click(`${T} [data-col="c1"] .thtop`, { modifiers: ['Shift'] });
  check('Shift adds a second key, numbered', (await p.$$eval(`${T} .tsort`, (e) => e.map((s) => s.textContent.trim()))).join() === '▼1,▲2', await p.$$eval(`${T} .tsort`, (e) => e.map((s) => s.textContent)));
  await p.click(`${head} .thtop`, { modifiers: ['Shift'] });
  check('Shift-click again clears that key only', (await p.getAttribute(head, 'aria-sort')) === 'none' && (await p.getAttribute(`${T} [data-col="c1"]`, 'aria-sort')) === 'ascending');

  // the sample column can't sort; time sorts descending to put the last sample first
  await p.click(`${T} [data-col="n"] .thtop`);
  check('the sample column has no sort', (await p.getAttribute(`${T} [data-col="n"]`, 'aria-sort')) === 'none');

  // reload keeps both; Clear removes both
  await p.fill(`${T} [data-filter="c0"]`, '1..1.3');
  await wait(600);
  await p.reload();
  await wait(1200);
  check('sort and filter come back after a reload', (await p.inputValue(`${T} [data-filter="c0"]`)) === '1..1.3' && (await p.getAttribute(`${T} [data-col="c1"]`, 'aria-sort')) === 'ascending');
  check('and the rows are queried again', await until(() => /of 200,000 rows/.test(document.querySelector('[data-table] [data-tstat]').textContent)), await stat());
  await p.click(`${T} [data-clear]`);
  check('Clear sort and filters', await until(() => /^200,000 rows in view/.test(document.querySelector('[data-table] [data-tstat]').textContent)) && (await p.inputValue(`${T} [data-filter="c0"]`)) === '' && !(await p.$(`${T} [data-clear]`)), await stat());
  const after = (await settingsLS()).layouts['/data/flight.tlm'][0];
  check('nothing left in the layout', !after.sort && !after.filters, after);
});
