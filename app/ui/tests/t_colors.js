// Channel colours (the least-used palette colour, not one already in the pane) and moving a channel to a category.
import { run } from './lib.js';

run('colors', async ({ p, check, text, wait, until, settingsLS }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const col = (ch) => p.evaluate((ch) => document.querySelector(`#stack .chip[data-ch="${ch}"]`)?.getAttribute('style') ?? '', ch);
  check('a lone channel takes the first colour, not its number\'s', /--trace-1\)/.test(await col(2)), await col(2));
  await p.click('#explorer .row.chan[data-ch="0"] .box');
  await p.click('#explorer .row.chan[data-ch="1"] .box');
  check('the next ones take the least-used', /--trace-2\)/.test(await col(0)) && /--trace-3\)/.test(await col(1)), [await col(0), await col(1)]);
  check('the explorer swatch uses it too', (await p.$eval('#explorer .row.chan[data-ch="2"]', (e) => e.getAttribute('style'))).includes('--trace-1)'));

  // a category of your own
  await p.click('#explorer .row.chan[data-ch="4"]', { button: 'right' });
  await p.click('.menu [data-cat]');
  await p.fill('[data-cat-input]', 'ctl');
  await p.click('.dlg [data-apply]');
  check('Move to a category… makes it', (await p.$$eval('#explorer .row.cat .nm', (e) => e.map((x) => x.textContent))).includes('ctl') && Object.values((await settingsLS()).catOverride).join() === 'ctl', await p.$$eval('#explorer .row.cat .nm', (e) => e.map((x) => x.textContent)));
  await p.click('#explorer .row.chan[data-ch="4"]', { button: 'right' });
  await p.click('.menu [data-cat]');
  await p.click('.dlg [data-reset]');
  check('Use the default puts it back', !(await p.$$eval('#explorer .row.cat .nm', (e) => e.map((x) => x.textContent))).includes('ctl'));
}, { settings: { layouts: { '/data/flight.tlm': [{ channels: [2] }] } } });
