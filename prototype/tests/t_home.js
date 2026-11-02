// The start page: a fresh launch doesn't restore behind it; Open folder goes to Explorer; the session row restores; Open only this.
const T = require('./lib');
T.run('home', async ({ p, check, tabs, text, menu, pick, wait }) => {
  // build a session with two files, then relaunch with the start page on
  await p.dblclick('.fitem[data-file="0415"] .fname'); await wait(2500);
  await wait(2300);
  check('two tabs before relaunch', (await tabs()).length === 2, await tabs());
  // (set and reload in one go: a setting the page saves in between would write startPage back)
  await Promise.all([p.waitForNavigation(), p.evaluate(() => { const s = JSON.parse(localStorage.getItem('plotter.settings')); s.startPage = true; localStorage.setItem('plotter.settings', JSON.stringify(s)); location.reload(); })]);
  await wait(1500);
  check('start page shows', await p.isVisible('#startPage'));
  check('nothing restored behind it', (await tabs()).length <= 1, await tabs());
  check('saved session offered', /tab/.test(await text('.strow.sess') || ''), await text('.strow.sess'));

  // Open folder → sample → Explorer, nothing open, session kept
  await p.click('.stcard[data-st="folder"]'); await wait(300);
  await p.click('#slotDlg [data-fold="sample"]'); await wait(800);
  check('Open folder lands in Explorer', !(await p.isVisible('#startPage')) && await p.isVisible('#exHead'));
  check('with nothing open', (await p.evaluate(() => document.querySelectorAll('.fitem.open').length)) === 0);
  await wait(2300);

  // back home, restore the session
  await p.click('#railHome').catch(() => p.click('#brandHome')); await wait(400);
  await p.click('.strow.sess'); await wait(2500);
  check('session row restores the tabs', (await tabs()).length === 2, await tabs());

  // Open only this
  await p.click('.fitem[data-file="main"] .fname', { button: 'right' }); await wait(200);
  check('file menu has Open only this', (await menu()).includes('Open only this'));
  await pick(/^Open only this$/); await wait(2500);
  check('only that file is open', (await p.evaluate(() => [...document.querySelectorAll('.fitem.open')].map(f => f.dataset.file))).join() === 'main');
  check('with undo', /undo/i.test(await text('#wsNote') || ''));
}, { settings: { fileOpenNew: 'starter' } });
