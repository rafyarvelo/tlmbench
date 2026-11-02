// Importing CSVs: the dialog, time and rate, When it opens, several files (shared tab), the profile coming back,
// re-import replacing, odd headers, and Recipes and profiles.
const T = require('./lib');
T.run('import', async ({ p, check, tabs, panes, text, drop, csv, wait }) => {
  await drop([csv('AAPL.csv')]);
  check('import dialog', /Open AAPL\.csv/.test(await text('#liveDlg header')));
  check('columns all in the default category list flat (no lone Data header)', await p.evaluate(() => !document.querySelector('#liveDlg .impcat') && document.querySelectorAll('#liveDlg .impcol').length >= 5));
  check('date column and daily rate detected', /daily/.test(await text('#liveDlg')), (await text('#liveDlg')).slice(0, 200));
  check('No time disables Time unit', await p.evaluate(() => { const r = document.querySelector('input[name="imprate"][value="samples"]'); r.click(); return true; }) && await p.evaluate(() => { const s = document.querySelector('[data-imp="unit"]'); return !s || s.disabled; }));
  await p.evaluate(() => document.querySelector('input[name="imprate"][value="recorded"]').click()); await wait(200);
  await p.selectOption('[data-imp="openAs"]', 'table'); await wait(150);
  await p.click('#liveDlg [data-go]'); await wait(2000);
  check('opens in its own tab', (await tabs()).some(t => /^AAPL\.csv\*/.test(t)), await tabs());
  check('as one table (chosen in the dialog)', /rows in view/.test(await text('#stack .tstat') || ''));
  check('...and Explorer lists its channels flat too', await p.evaluate(() => { const g = [...document.querySelectorAll('#fileTree .ghead .gnm')].map(e => e.textContent); return !g.includes('Data') && !!document.querySelector('#fileTree .chan[data-ch*="Close"]'); }));

  // the same header again: the profile comes back; the same file replaces
  await drop([csv('AAPL.csv'), csv('MSFT.csv')]);
  check('profile recognised', /AAPL/.test(await text('#liveDlg .impprof') || await text('#liveDlg')));
  check('already-imported warning', /Already imported/.test(await text('#liveDlg')));
  check('Panes choice came back from the profile', (await p.inputValue('[data-imp="openAs"]')) === 'table');
  await p.selectOption('[data-imp="multi"]', 'shared'); await wait(200);
  await p.click('#liveDlg [data-go]'); await wait(2500);
  const fi = await p.evaluate(() => [...document.querySelectorAll('.fitem')].map(f => f.querySelector('.fname').textContent).filter(n => /AAPL|MSFT/.test(n)));
  check('re-import replaced, no duplicate', fi.filter(n => n === 'AAPL.csv').length === 1, fi);
  check('several files in one tab', (await tabs()).some(t => /AAPL \+ 1 more\*/.test(t)), await tabs());

  // awkward headers keep their names
  await drop([csv('long_special_names.csv')]);
  const names = await p.evaluate(() => [...document.querySelectorAll('.imph')].map(e => e.textContent));
  check('headers kept verbatim', names.length > 3 && names.some(n => /[\[\(°%µ\s]/.test(n)), names.slice(0, 5));
  await p.evaluate(() => document.querySelector('#liveDlg [data-x]').click()); await wait(200);

  // Recipes and profiles: profile row with prefix and layout
  await p.click('#exFolder'); await wait(150);
  await p.evaluate(() => [...document.querySelectorAll('.catmenu .mi')].find(b => /profiles and recipes/.test(b.textContent)).click()); await wait(300);
  const mgr = await text('#recDlg');
  check('profile listed with its pieces', /AAPL/.test(mgr) && /Channel prefix/.test(mgr) && /Opens with/.test(mgr), mgr.slice(0, 200));
  await p.keyboard.press('Escape'); await wait(200);
  check('Esc closes it', !(await p.isVisible('#recDlg')));
});
