// The stack's heights, as the prototype's: charts are 150 px with up to three rows, 125 px with four or five, 105 px
// with more, and they keep that height; when the panes don't fit, the stack scrolls (Add pane at its end, the
// overview staying under it), and nothing is drawn outside the stack's visible part.
import { run } from './lib.js';

run('stack', async ({ p, check, wait, until, paneCount }) => {
  const heights = () => p.$$eval('#stack .plot', (e) => e.map((x) => Math.round(x.getBoundingClientRect().height)));
  check('three rows: 150 px charts', (await heights()).every((h) => h === 150) && (await heights()).length === 3, await heights());
  await p.click('#addPane');
  check('four rows: 125 px', await until(() => [...document.querySelectorAll('#stack .plot')].every((x) => Math.round(x.getBoundingClientRect().height) === 125)), await heights());
  await p.click('#addPane');
  check('five rows: still 125 px', (await heights()).every((h) => h === 125), await heights());
  await p.click('#addPane');
  await p.click('#addPane');
  check('seven rows: 105 px, and they keep it', await until(() => [...document.querySelectorAll('#stack .plot')].every((x) => Math.round(x.getBoundingClientRect().height) === 105)), await heights());
  check('seven panes', (await paneCount()) === 7);

  const geo = () => p.evaluate(() => {
    const s = document.getElementById('stack');
    const r = s.getBoundingClientRect();
    const ov = document.querySelector('.ovplot').getBoundingClientRect();
    return { scrolls: s.scrollHeight > s.clientHeight + 1, top: r.top, bottom: r.bottom, ovTop: ov.top, ovBottom: ov.bottom, win: innerHeight };
  });
  const g = await geo();
  check('the panes don\'t fit: the stack scrolls', g.scrolls, g);
  check('the overview stays in the window, under the stack', g.ovTop > g.bottom && g.ovBottom <= g.win, g);
  await p.evaluate(() => document.getElementById('stack').scrollTo(0, 1e6));
  check('Add pane is at the end of the stack', await until(() => {
    const s = document.getElementById('stack').getBoundingClientRect();
    const a = document.getElementById('addPane').getBoundingClientRect();
    return a.bottom <= s.bottom + 1 && a.bottom > s.bottom - 40;
  }));

  // the last pane (selected) gets a channel; scroll so it straddles the stack's bottom edge: nothing of it shows
  // between the stack and the overview's plot
  await p.click('#chans .row.chan[data-ch="0"] .box');
  await p.evaluate(() => {
    const s = document.getElementById('stack');
    s.scrollTo(0, 1e6);
    const b = s.getBoundingClientRect().bottom;
    const r = s.querySelector('[data-pane="6"] .plot').getBoundingClientRect();
    s.scrollBy(0, r.top + r.height / 2 - b);
  });
  await wait(700);
  const lit = async () => {
    const png = (await p.screenshot()).toString('base64');
    const area = await p.evaluate(() => {
      const s = document.getElementById('stack').getBoundingClientRect();
      const ov = document.querySelector('.ovplot').getBoundingClientRect();
      // the overview's heading row, between its select and its range text
      const l = document.querySelector('[data-ovch]').getBoundingClientRect().right + 10;
      const r = document.querySelector('[data-ovrange]').getBoundingClientRect().left - 10;
      return { x: l, y: s.bottom + 1, w: r - l, h: ov.top - s.bottom - 2 };
    });
    return p.evaluate(async ([png, r]) => {
      const img = new Image();
      img.src = 'data:image/png;base64,' + png;
      await img.decode();
      const c = document.createElement('canvas');
      c.width = img.width;
      c.height = img.height;
      const g = c.getContext('2d');
      g.drawImage(img, 0, 0);
      const d = g.getImageData(Math.round(r.x), Math.round(r.y), Math.round(r.w), Math.max(1, Math.round(r.h))).data;
      let n = 0;
      for (let i = 0; i < d.length; i += 4) if (d[i] + d[i + 1] + d[i + 2] > 330) n++;
      return n;
    }, [png, area]);
  };
  const straddle = await p.evaluate(() => {
    const s = document.getElementById('stack').getBoundingClientRect();
    const r = document.querySelector('#stack [data-pane="6"] .plot').getBoundingClientRect();
    return r.top < s.bottom - 20 && r.bottom > s.bottom + 20;
  });
  check('a chart with a trace runs past the stack\'s bottom edge', straddle);
  const n = await lit();
  check('...and none of its traces or axes draw outside the stack', n === 0, n);
});
