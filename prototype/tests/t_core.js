// The everyday flow on one file: panes and undo, views, cursors + Measure, events and rules, layouts,
// limits, XY, lock, and the session coming back after a reload.
const T = require('./lib');
T.run('core', async ({ p, check, tabs, paneCount, text, pick, menu, wait, settingsLS }) => {
  check('one tab for the flight log', (await tabs()).join() === 'synthetic_flight.bin*', await tabs());
  check('starter panes', (await paneCount()) === 3, await paneCount());
  check('Explorer header', await p.isVisible('#exHead'));

  // panes: split, close, undo
  await p.evaluate(() => document.querySelector('#stack section.pane [data-pmenu]').click()); await wait(200);
  await pick(/^Split down$/); await wait(500);
  check('split down adds a pane', (await paneCount()) === 4, await paneCount());
  await p.evaluate(() => document.querySelector('#stack section.pane [data-pmenu]').click()); await wait(200);
  await pick(/^Close pane$/); await wait(500);
  check('close pane', (await paneCount()) === 3, await paneCount());
  await p.evaluate(() => { const b = [...document.querySelectorAll('#wsNote button')].find(b => /undo/i.test(b.textContent)); b && b.click(); }); await wait(500);
  check('undo restores it', (await paneCount()) === 4, await paneCount());

  // views: table, then back to chart
  await p.evaluate(() => document.querySelector('#stack section.pane [data-viewmenu]').click()); await wait(200);
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="both"]').click()); await wait(1200);
  check('chart and table', /rows in view/.test(await text('#stack .tstat')));

  // cursors from a table row, read in Measure
  await p.click('#stack .trw:not([hidden]) >> nth=1', { button: 'right' }); await wait(150);
  await pick(/^Set cursor A here$/); await wait(300);
  check('cursor A set from a table row', (await text('#cA')) !== '—', await text('#cA'));
  await p.click('#stack .trw:not([hidden]) >> nth=4', { button: 'right' }); await wait(150);
  await pick(/^Set cursor B here$/); await wait(300);
  check('Δt between cursors', /ms|s/.test(await text('#cDt')), await text('#cDt'));

  // events: list, rules, a marker at A
  await p.click('[data-rail="events"]'); await wait(300);
  const sum0 = await text('.evsum');
  check('events listed with rule events', /\d+ events/.test(sum0) && /from rules/.test(sum0), sum0);
  const n0 = +(/(\d+) events/.exec(sum0) || [0, 0])[1];
  await p.click('#evAdd'); await wait(200);
  check('+ Event offers marker and range', /Marker at cursor A/.test((await menu()).join('|')) && /Range from A to B/.test((await menu()).join('|')));
  await pick(/Marker at cursor A/); await wait(200);
  check('+ Marker at A adds an event', +(/(\d+) events/.exec(await text('.evsum')) || [0, 0])[1] === n0 + 1, await text('.evsum'));
  await p.click('#evRules'); await wait(300);
  check('Rules opens the rules-and-checks editor at Event rules', await p.evaluate(() => { const d = document.getElementById('recDlg'); return !!d && !d.hidden && d.querySelector('[data-sname]').value === 'Event rules' && [...d.querySelectorAll('[data-kf="kind"]')].every(s => s.value === 'mark'); }));
  await p.evaluate(() => document.querySelector('#recDlg [data-x]').click()); await wait(200);

  // layouts: save the tab as a layout
  await p.evaluate(() => { const t = document.querySelector('#wsTabs .wstab[aria-selected="true"] [data-tabm]'); t && t.click(); }); await wait(250);
  check('tab menu', await p.isVisible('#tabMenu'));
  await p.click('#wsSaveAs'); await wait(150);
  await p.fill('#wsName', 'Core test layout'); await p.click('#wsNameOk'); await wait(400);
  await p.click('[data-rail="layouts"]'); await wait(300);
  check('layout saved and listed', /Core test layout/.test(await p.evaluate(() => document.body.innerText)));
  await p.click('[data-rail="explorer"]'); await wait(200);

  // limits from a chip
  await p.evaluate(() => document.querySelector('#stack [data-chip="accel_x"]').click()); await wait(250);
  await p.click('.cheditor [data-tab="limits"]'); await wait(150);
  await p.fill('.cheditor [data-lim="wh"]', '0.5'); await wait(200);
  check('limits saved for the channel', ((await settingsLS()).limits || {}).accel_x && (await settingsLS()).limits.accel_x.wh === 0.5, (await settingsLS()).limits);
  await p.keyboard.press('Escape'); await p.mouse.click(900, 880); await wait(200);

  // XY
  await p.evaluate(() => [...document.querySelectorAll('#stack section.pane [data-viewmenu]')][1].click()); await wait(200);
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="xy"]').click()); await wait(1200);
  check('XY plot with an X picker', await p.evaluate(() => !!document.querySelector('#stack .xyplot canvas') && !!document.querySelector('#stack [data-xsel]')));

  // pane settings: Ignore outliers is offered, off by default
  await p.evaluate(() => document.querySelector('#stack section.pane [data-axis]').click()); await wait(250);
  check('Ignore outliers offered, not chosen', await p.evaluate(() => { const s = document.querySelector('.axpop [data-ax="u0.range"]'); return !!s && !!s.querySelector('option[value="robust"]') && s.value === ''; }));
  await p.keyboard.press('Escape'); await p.mouse.click(900, 880);

  // lock
  await p.click('#b-lock'); await wait(200);
  check('lock on', (await p.getAttribute('#b-lock', 'aria-pressed')) === 'true');
  await p.click('#b-lock'); await wait(200);

  // session comes back
  const before = { n: await paneCount(), tabs: (await tabs()).length };
  await wait(2300); await p.reload(); await wait(3000);
  check('session restores the panes', (await paneCount()) === before.n, [before.n, await paneCount()]);
});
