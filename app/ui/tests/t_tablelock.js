// The view lock and tables: with "Table sort and filter still work while locked" (the default) a locked view,
// layout lock included, still sorts and filters; with it off, the table keeps its sort and filters (the boxes are
// disabled, a header click explains), and widths and columns follow the layout lock.
import { run } from './lib.js';

run('tablelock', async ({ p, check, until, settingsLS, text }) => {
  const T = '#stack [data-pane="0"] [data-table]';
  const sortOf = (c) => p.getAttribute(`${T} [data-col="${c}"]`, 'aria-sort');

  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="table"]');
  await until(() => document.querySelectorAll('[data-table] [data-row]').length > 3);

  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]');
  await p.click('[data-set="locked"]');
  await p.click('[data-set="lockLayout"]');
  const s = await settingsLS();
  check('locked, with the layout locked too', s.locked === true && s.lockLayout === true, s);
  check('sort and filter still work is on by default', s.lockSortFilter === undefined || s.lockSortFilter === true);
  await p.click('#b-settings');

  await p.click(`${T} [data-col="c0"] .thtop`);
  check('a locked view still sorts', await until(() => document.querySelector('[data-table] [data-col="c0"]')?.getAttribute('aria-sort') === 'ascending'), await sortOf('c0'));
  await p.fill(`${T} [data-filter="c0"]`, '> 1.2');
  check('and filters', await until(() => /of 200,000 rows/.test(document.querySelector('[data-table] [data-tstat]').textContent)), await text(`${T} [data-tstat]`));

  // the layout lock still holds the columns
  await p.hover(`${T} [data-col="c0"]`);
  await p.click(`${T} [data-colmenu="c0"]`);
  await p.click('.colmenu [data-hm="hide"]');
  check('the layout lock keeps the columns', (await p.$$(`${T} .thead [data-col]`)).length === 4);

  // sort and filter off while locked
  await p.click('#b-settings');
  await p.click('[data-set="lockSortFilter"]');
  check('the setting is off', (await settingsLS()).lockSortFilter === false, await settingsLS());
  await p.click('#b-settings');
  check('the filter boxes are disabled', await p.isDisabled(`${T} [data-filter="c0"]`));
  await p.click(`${T} [data-col="c0"] .thtop`);
  check('a header click leaves the sort as it was', (await sortOf('c0')) === 'ascending', await sortOf('c0'));
  check('and says why', await until(() => /sort and filters are locked/i.test(document.body.innerText)));
});
