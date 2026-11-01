// Open folder (read-only): its files appear under the folder, import on open, Import all, unreadable ones hidden.
const path = require('path');
const T = require('./lib');
T.run('folder', async ({ p, check, tabs, text, wait }) => {
  await p.click('#exFolder'); await wait(200);
  const [fc] = await Promise.all([p.waitForEvent('filechooser'), p.evaluate(() => [...document.querySelectorAll('.catmenu .mi')].find(b => /Open folder/.test(b.textContent)).click())]);
  check('a folder picker', fc.isMultiple());
  await fc.setFiles(path.join(T.DIR, 'fold/stocks')); await wait(600);
  check('note says read-only', /Read-only: nothing is written/.test(await text('#wsNote')), await text('#wsNote'));
  check('folder name on the button', (await text('#exFolder')) === 'stocks', await text('#exFolder'));
  const listed = await p.evaluate(() => [...document.querySelectorAll('#fileTree .fitem .fname')].map(e => e.textContent));
  check('its CSVs listed', listed.includes('AAPL.csv') && listed.includes('MSFT.csv'), listed);
  check('files it cannot read hidden by default', !listed.includes('readme.md'), listed);
  await p.dblclick('.fitem:has(.fname:text-is("AAPL.csv")) .fname'); await wait(800);
  check('opening imports it', /Open AAPL\.csv/.test(await text('#liveDlg header')));
  await p.click('#liveDlg [data-go]'); await wait(2000);
  check('imported file stays in its folder and opens a tab', (await tabs()).some(t => /^AAPL\.csv\*/.test(t)), await tabs());
  await p.click('#exFolder'); await wait(200);
  await p.evaluate(() => [...document.querySelectorAll('.catmenu .mi')].find(b => /Import all/.test(b.textContent)).click()); await wait(800);
  check('Import all takes the rest', /MSFT/.test(await text('#liveDlg')), (await text('#liveDlg header')));
  await p.click('#liveDlg [data-go]'); await wait(2500);
  check('both imported', (await p.evaluate(() => [...document.querySelectorAll('.fitem.open .fname')].map(e => e.textContent))).filter(n => /AAPL|MSFT/.test(n)).length === 2);
});
