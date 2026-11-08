// Sample markers, as the prototype's: a chart of one channel marks each raw sample with a small ring once the samples
// are more than 6 px apart; a chart of several channels, or one zoomed out, doesn't. Draw every sample (Settings)
// asks the core for raw samples up to a budget.
import { run } from './lib.js';

run('markers', async ({ p, check, plotAt, until, wait }) => {
  const markers = () => p.evaluate(() => window.__tlmApp.markers());
  const panes = () => p.evaluate(() => window.__tlmApp.panes());
  check('zoomed out: no markers', (await markers()).every((n) => !n), await markers());
  // take pane 1 down to one channel
  await p.click('#stack [data-pane="0"] .phead');
  await p.hover('#stack [data-pane="0"] .chip >> nth=1');
  await p.click('#stack [data-pane="0"] .chip >> nth=1 >> [data-chip-x]');
  check('pane 1 has one channel', await until(() => window.__tlmApp.panes()[0].length === 1), await panes());

  // zoom in on pane 1 until its samples are far apart
  const c = await plotAt(0);
  await p.mouse.move(c.x, c.y);
  for (let k = 0; k < 40 && !(await markers())[0]; k++) {
    await p.mouse.wheel(0, -600);
    await wait(120);
  }
  const m = await markers();
  check('zoomed in: a ring per sample in the one-channel chart', m[0] > 0 && m[0] < 1440 / 6, m);
  check('...not in the charts of two channels', !m[1] && !m[2], m);

  // Draw every sample (Settings): a whole recording's samples come back raw, not reduced
  await p.keyboard.press('f');
  const status = () => p.evaluate(() => document.getElementById('status')?.textContent ?? '');
  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]');
  await p.check('[data-set="showPerf"]').catch(() => p.click('[data-set="showPerf"]'));
  check('zoomed out the views are reduced (M4)', await until(() => / M4 /.test(document.getElementById('status')?.textContent ?? '')), await status());
  await p.selectOption('[data-set="drawRaw"]', '1000000');
  check('Up to 1M per channel draws every sample of the 200k in view', await until(() => / raw /.test(document.getElementById('status')?.textContent ?? '') && /^[\d,]{7,} points/.test(document.getElementById('status')?.textContent ?? '')), await status());
  await p.selectOption('[data-set="drawRaw"]', '100000');
  check('Up to 100k reduces them again', await until(() => / M4 /.test(document.getElementById('status')?.textContent ?? '')), await status());
});
