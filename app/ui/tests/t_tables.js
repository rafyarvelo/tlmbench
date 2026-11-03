// Table panes: the view menu's Table and Chart and table; rows read a window at a time as the table scrolls, the
// view moving them; a row click placing cursor A (Shift: B), double-click centring the view, Go to A; column widths
// and the table's height under a chart saved with the layout.
import { run } from './lib.js';

const T0 = 1767225600000000000n; // the fake core's first sample; 1000 Hz
const timeOf = (n) => String(T0 + BigInt(n) * 1000000n);

run('tables', async ({ p, check, text, until, settingsLS, wait, fake }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const viewMenu = async (i) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-vmenu]`);
  };
  const T = '#stack [data-pane="0"] [data-table]';
  const rowsInView = async () => Number((await text(`${T} [data-tstat]`)).replace(/\D/g, ''));
  const cell = (r, c) => text(`${T} [data-row="${r}"] span:nth-child(${c})`);

  await viewMenu(0);
  const items = await p.$$eval('.viewmenu [data-vm]', (b) => b.map((x) => x.dataset.vm));
  check('the view menu', items.join() === 'chart,table,both,values', items);
  await p.click('.viewmenu [data-vm="table"]');
  check('pane 1 is a table, no plot', (await app('modes'))[0] === 'table' && !!(await p.$(T)) && !(await p.$('#stack [data-pane="0"] .plot')));
  const chans = (await app('panes'))[0];
  const heads = await p.$$eval(`${T} .tth`, (h) => h.map((x) => x.textContent.trim()));
  check('columns: sample, time, a channel each', heads.length === chans.length + 2 && heads[0] === 'Sample' && heads[1] === 'Time', heads);

  // the whole recording is in view: 200,000 rows, of which only a window is read and drawn
  check('every sample in view is a row', await until(() => /200,000 rows/.test(document.querySelector('[data-table] [data-tstat]').textContent)), await text(`${T} [data-tstat]`));
  check('the first row', await until(() => document.querySelector('[data-table] [data-row="0"] span')?.textContent === '0'), await cell(0, 1));
  const drawnRows = (await p.$$(`${T} [data-row]`)).length;
  check('only the rows on screen are drawn', drawnRows > 3 && drawnRows < 80, drawnRows);
  check('only a window is read', Math.max(...(await fake(() => window.__tlmFake.rowsAsked))) < 400, await fake(() => window.__tlmFake.rowsAsked));
  check('the other panes still draw', await until(() => window.__tlmApp.drawn().slice(1).some((d) => d && d.length)));

  // scrolling reads the rows it reaches
  await p.$eval(`${T} .tscroll`, (e) => (e.scrollTop = 100000 * 26));
  check('scrolled to the middle', await until(() => [...document.querySelectorAll('[data-table] [data-row]')].some((r) => r.querySelector('span').textContent === '100,000')));
  const row = await p.$$eval(`${T} [data-row]`, (rs) => Number(rs.find((r) => r.querySelector('span').textContent === '100,000').dataset.row));
  check('its value matches the channel', (await cell(row, 3)) !== '' && (await cell(row, 3)) !== '—', await cell(row, 3));

  // a click places cursor A, Shift-click B
  await p.click(`${T} [data-row="${row}"]`);
  check('click places A on the row', (await app('cursors')).A === timeOf(100000), await app('cursors'));
  check('and marks it', await until((r) => document.querySelector(`[data-table] [data-row="${r}"]`)?.classList.contains('ca'), row));
  await p.click(`${T} [data-row="${row + 5}"]`, { modifiers: ['Shift'] });
  check('Shift-click places B', (await app('cursors')).B === timeOf(100005), await app('cursors'));

  // double-click centres the view on the row; the table follows the view
  await p.dblclick(`${T} [data-row="${row + 2}"]`);
  const v = await app('view');
  const mid = (BigInt(v.t0) + BigInt(v.t1)) / 2n;
  check('double-click centres the view', mid - BigInt(timeOf(100002)) < 2n && BigInt(timeOf(100002)) - mid < 2n, [v, timeOf(100002)]);
  const r1 = await p.$eval('#stack [data-pane="1"] .plot', (e) => { const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; });
  await p.mouse.move(r1.x, r1.y);
  for (let k = 0; k < 6; k++) await p.mouse.wheel(0, -200);
  check('zooming in leaves fewer rows', await until(() => Number(document.querySelector('[data-table] [data-tstat]').textContent.replace(/\D/g, '')) < 200000), await rowsInView());
  const n = await rowsInView();
  const vz = await app('view');
  const expect = Math.ceil(Number(BigInt(vz.t1) - T0) / 1e6) - Math.ceil(Number(BigInt(vz.t0) - T0) / 1e6);
  check('the rows are the samples in [t0, t1)', n === expect, [n, expect]);

  // Go to A brings A's row into sight
  await p.$eval(`${T} .tscroll`, (e) => (e.scrollTop = 0));
  await p.click(`${T} [data-go="A"]`);
  check('Go to A', await until((t) => !!document.querySelector('[data-table] .trw.ca'), null), await app('cursors'));

  // a column's width is saved with the layout
  const h = await p.$eval(`${T} .tth[data-col="t"] .trsz`, (e) => { const b = e.getBoundingClientRect(); return { x: b.x + 3, y: b.y + b.height / 2 }; });
  const w0 = await p.$eval(`${T} .tth[data-col="t"]`, (e) => e.getBoundingClientRect().width);
  await p.mouse.move(h.x, h.y);
  await p.mouse.down();
  await p.mouse.move(h.x + 60, h.y, { steps: 4 });
  await p.mouse.up();
  const w1 = await p.$eval(`${T} .tth[data-col="t"]`, (e) => e.getBoundingClientRect().width);
  check('dragging a header edge resizes the column', w1 > w0 + 40, [w0, w1]);
  check('the width is saved', (await settingsLS()).layouts['/data/flight.tlm'][0].widths?.t > 150, (await settingsLS()).layouts['/data/flight.tlm'][0]);

  // Chart and table: the chart keeps drawing, its table below at 210 px (less when the pane is short), taller by
  // dragging the grip
  await viewMenu(1);
  await p.click('.viewmenu [data-vm="both"]');
  const B = '#stack [data-pane="1"] [data-table]';
  check('chart and table', !!(await p.$('#stack [data-pane="1"] .plot')) && !!(await p.$(B)));
  check('the chart still draws', await until(() => { const d = window.__tlmApp.drawn()[1]; return d && d.length; }));
  check('the table is 210 px', Math.round(await p.$eval(B, (e) => e.getBoundingClientRect().height)) === 210, await p.$eval(B, (e) => e.getBoundingClientRect().height));
  const g = await p.$eval(`${B} .tgrip`, (e) => { const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; });
  await p.mouse.move(g.x, g.y);
  await p.mouse.down();
  await p.mouse.move(g.x, g.y - 50, { steps: 4 });
  await p.mouse.up();
  check('dragging the grip up makes it taller', (await settingsLS()).layouts['/data/flight.tlm'][1].tableH === 260, (await settingsLS()).layouts['/data/flight.tlm'][1]);

  await p.reload();
  await wait(800);
  check('views come back after a reload', JSON.stringify((await app('modes')).slice(0, 2)) === '["table","both"]', await app('modes'));
  check('with the width and height', (await p.$eval(B, (e) => e.style.flexBasis)) === '260px' && (await p.$eval(`${T} .tth[data-col="t"]`, (e) => e.getBoundingClientRect().width)) > w0 + 40);
});
