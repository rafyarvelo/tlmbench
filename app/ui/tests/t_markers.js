// Sample markers, as the prototype's: a chart of one channel marks each raw sample with a small ring once the samples
// are more than 6 px apart; a chart of several channels, or one zoomed out, doesn't.
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
});
