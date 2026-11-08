// Table rows: hovering a row draws a hover line on the charts, right-click sets cursor A or B or clears them or
// centres the charts, rows between the cursors are shaded, the keys move the window (arrows a row, Page a page,
// Home and End), and the status line says what a row is.
import { run } from './lib.js';

run('tablerows', async ({ p, check, text, until, wait }) => {
  const T = '#stack [data-pane="0"] [data-table]';
  const hover = () => p.evaluate(() => window.__tlmApp.hoverT());
  const cursors = () => p.evaluate(() => window.__tlmApp.cursors());
  const top = () => p.evaluate((t) => document.querySelector(`${t} .tscroll`).scrollTop, T);

  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="table"]');
  await until(() => document.querySelectorAll('[data-table] [data-row]').length > 3);

  check('the status line says what a row is', / in view · one per accel_x sample \(1000\.000 Hz\)/.test(await text(`${T} [data-tstat]`)), await text(`${T} [data-tstat]`));

  // hover
  const t2 = await p.getAttribute(`${T} [data-row="2"]`, 'data-t');
  await p.hover(`${T} [data-row="2"]`);
  check('hovering a row hovers the charts at its time', await until((t) => window.__tlmApp.hoverT() === t, t2), [await hover(), t2]);
  await p.mouse.move(5, 5);
  check('leaving the table clears it', await until(() => window.__tlmApp.hoverT() === null), await hover());

  // right-click
  await p.click(`${T} [data-row="2"]`, { button: 'right' });
  check('right-click opens the row menu', !!(await p.$('.rowmenu')));
  check('with the clear item off', (await p.getAttribute('.rowmenu [data-rm="clear"]', 'disabled')) !== null);
  await p.click('.rowmenu [data-rm="A"]');
  check('Set cursor A here', await until((t) => window.__tlmApp.cursors().A === t, t2), await cursors());
  const t6 = await p.getAttribute(`${T} [data-row="6"]`, 'data-t');
  await p.click(`${T} [data-row="6"]`, { button: 'right' });
  await p.click('.rowmenu [data-rm="B"]');
  check('Set cursor B here', await until((t) => window.__tlmApp.cursors().B === t, t6), await cursors());
  check('the table bar reads A, B and Δt', await until(() => /A .* · B .* · Δt/.test(document.querySelector('[data-table] [data-tcur]')?.textContent ?? '')), await text(`${T} [data-tcur]`));
  check('the rows between are shaded', await until(() => ['4', '5'].every((r) => document.querySelector(`[data-table] [data-row="${r}"]`)?.classList.contains('cab')) && !document.querySelector('[data-table] [data-row="6"]').classList.contains('cab')), await p.evaluate(() => [...document.querySelectorAll('[data-table] [data-row]')].map((e) => e.dataset.row + ':' + e.className)));
  // zoom in on the second chart (the table's own pane is first) so there is room to centre
  const c = await p.evaluate(() => { const r = document.querySelectorAll('#stack section.pane .plot')[1].getBoundingClientRect(); return { x: r.left + 200, y: r.top + r.height / 2 }; });
  await p.mouse.move(c.x, c.y);
  for (let k = 0; k < 6; k++) await p.mouse.wheel(0, -300);
  await until(() => BigInt(window.__tlmApp.view().t1) - BigInt(window.__tlmApp.view().t0) < 100000000000n);
  await wait(600);
  await p.click(`${T} [data-row="4"]`, { button: 'right' });
  await p.click('.rowmenu [data-rm="centre"]');
  await p.click(`${T} [data-row="4"]`, { button: 'right' });
  await p.click('.rowmenu [data-rm="centre"]');
  const mid = await p.getAttribute(`${T} [data-row="4"]`, 'data-t');
  check('Centre the charts on this row', await until((t) => { const v = window.__tlmApp.view(); return Math.abs(Number((BigInt(v.t0) + BigInt(v.t1)) / 2n - BigInt(t))) < 5000000; }, mid), [await p.evaluate(() => window.__tlmApp.view()), mid]);
  await p.click(`${T} [data-row="4"]`, { button: 'right' });
  await p.click('.rowmenu [data-rm="clear"]');
  check('Clear cursors', await until(() => window.__tlmApp.cursors().A === null && window.__tlmApp.cursors().B === null), await cursors());
  check('and the readout goes', await until(() => !document.querySelector('[data-table] [data-tcur]')));

  // keys
  await p.click(`${T} [data-row="3"]`);
  await p.focus(`${T} .tscroll`);
  await p.keyboard.press('Home');
  await until(() => document.querySelector('[data-table] .tscroll').scrollTop === 0);
  await p.keyboard.press('ArrowDown');
  check('ArrowDown moves a row', (await top()) === 26, await top());
  await p.keyboard.press('PageDown');
  const page = (await top()) - 26;
  check('Page moves a window of rows', page > 26 && page % 26 === 0, page);
  await p.keyboard.press('End');
  check('End goes to the last row', (await top()) > 100000, await top());
  await p.keyboard.press('Home');
  check('Home goes back', (await top()) === 0, await top());
});
