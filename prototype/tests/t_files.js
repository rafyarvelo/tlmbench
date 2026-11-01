// Several files: open in tabs, the open strip, Show only these, Compare, channel prefixes and numbers, Close all + undo.
const T = require('./lib');
T.run('files', async ({ p, check, tabs, panes, text, drop, csv, pick, menu, wait, settingsLS }) => {
  await p.dblclick('.fitem[data-file="0415"] .fname'); await wait(2500);
  const t = await tabs();
  check('second file opens in its own tab', t.length === 2 && /flight_0415\.bin\*/.test(t[1]), t);
  check('open strip counts files', /OPEN · 2/i.test(await text('#exOpen')), await text('#exOpen'));
  { const x = await p.evaluate(() => { const D = window.__tlmDerived, keys = [...document.querySelectorAll('.fitem.open')].map(e => e.dataset.ofile || e.dataset.key).filter(Boolean); const other = D.slots().find(x => /flight_0415/.test(x[2])); return { bySlot: D.resolveName('main', other[1] + ':accel_x'), byName: D.resolveName('main', 'flight_0415.accel_x'), none: D.resolveName('main', '9:accel_x') }; });
    check('a derived expression can name another open file\'s channel (by number or name)', !!x.bySlot && x.bySlot === x.byName && !x.none, x); }

  // Show only these: toggles, and says so
  await p.click('[data-oonly]'); await wait(300);
  check('only open files listed', (await p.evaluate(() => document.querySelectorAll('.fitem').length)) === 2);
  check('banner with Show all', /Open files only/.test(await text('.exonly')));
  await p.click('[data-oshowall]'); await wait(300);
  check('Show all lists every file again', (await p.evaluate(() => document.querySelectorAll('.fitem').length)) > 2);

  // Compare two selected files
  await p.click('.fitem[data-file="main"] .fname'); await p.click('.fitem[data-file="0415"] .fname', { modifiers: ['Control'] }); await wait(200);
  check('Compare in the selection bar', /Compare/.test(await text('#exSelBar')));
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => b.textContent === 'Compare').click()); await wait(2500);
  check('compare tab', (await tabs()).some(x => /synthetic_flight vs flight_0415\*/.test(x)), await tabs());
  const pn = await panes();
  check('one pane per shared channel', pn.length === 4 && pn.every(x => /synthetic_flight/.test(x) && /flight_0415/.test(x)), pn);
  await p.keyboard.press('Escape');

  // stock CSVs: compare by right-click, prefix rule from the import profile, a per-file alias
  await drop([csv('AAPL.csv'), csv('MSFT.csv')]);
  await p.selectOption('[data-imp="prefixMode"]', 'regex'); await wait(150);
  await p.fill('[data-imp="prefixVal"]', '^(..)'); await p.press('[data-imp="prefixVal"]', 'Enter'); await wait(200);
  check('prefix preview', /AA · Open, MS · Open/.test(await text('[data-prefprev]')), await text('[data-prefprev]'));
  await p.click('#liveDlg [data-go]'); await wait(2200);
  await p.evaluate(() => { const t = [...document.querySelectorAll('#wsTabs .wstab')].find(e => /^AAPL/.test(e.innerText.trim())); t && t.click(); }); await wait(900);
  await p.click('.fitem .fname >> text=MSFT.csv', { button: 'right' }); await wait(150);
  check('Compare with the file in front', (await menu()).some(x => /^Compare with AAPL\.csv/.test(x)), await menu());
  await pick(/^Compare with/); await wait(2200);
  const sp = await panes();
  check('stock compare uses the prefix rule', sp.length === 4 && /AA Open MS Open/.test(sp[0]), sp);
  p.once('dialog', d => d.accept('Apple'));
  await p.click('.fitem .fname >> text=AAPL.csv', { button: 'right' }); await wait(150);
  await pick(/^Channel prefix/); await wait(800);
  check('per-file alias wins', /Apple Open/.test((await panes())[0]), (await panes())[0]);

  // Settings → file numbers instead of names
  await p.click('#b-settings'); await wait(300);
  await p.selectOption('#setPanel [data-set="fileLabel"]', 'number'); await wait(600);
  check('file numbers apply right away', /Open·\d/.test((await panes())[0]) && !/Apple/.test((await panes())[0]), (await panes())[0]);
  check('saved', (await settingsLS()).fileLabel === 'number');
  await p.selectOption('#setPanel [data-set="fileLabel"]', 'prefix'); await wait(300);

  // Close all, then undo
  await p.click('[data-rail="explorer"]'); await wait(300);
  await p.click('[data-oall]'); await wait(800);
  check('Close all empties the workspace', (await p.evaluate(() => document.querySelectorAll('.fitem.open').length)) === 0);
  await p.evaluate(() => { const b = [...document.querySelectorAll('#wsNote button')].find(b => /undo/i.test(b.textContent)); b && b.click(); }); await wait(2000);
  check('undo reopens the files and tabs', (await p.evaluate(() => document.querySelectorAll('.fitem.open').length)) >= 3 && (await tabs()).length >= 4, await tabs());
});
