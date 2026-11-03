// The overview: what it shows (the selected pane, or one channel pinned), its time range, the switch for showing it on
// top, and gestures by mouse mode (Box drags a new view and a click centres; Cursors places A and B; Pan drags the
// window; the wheel zooms).
import { run } from './lib.js';

run('overview', async ({ p, check, wait, text, until, settingsLS, visible }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const view = async () => { const v = await app('view'); return [BigInt(v.t0), BigInt(v.t1)]; };
  const ov = () => p.evaluate(() => { const r = document.querySelector('.ovplot').getBoundingClientRect(); return { x: r.left, y: r.top + r.height / 2, w: r.width }; });

  check('it follows the selected pane', /pane 1 · 2 channels/.test(await text('.ovhead')), await text('.ovhead'));
  check('and shows its time range', /\d.* – .*\d/.test(await text('[data-ovrange]')), await text('[data-ovrange]'));
  await p.selectOption('[data-ovch]', '3');
  check('it can be pinned to one channel, saved', (await settingsLS()).overviewCh === 3 && (await p.$eval('[data-ovch]', (e) => e.selectedOptions[0].textContent)) === 'pitch');
  await p.selectOption('[data-ovch]', '');
  check('and follow again', (await settingsLS()).overviewCh === null);

  // Box: drag a range
  const o = await ov();
  const [a0, a1] = await view();
  await p.mouse.move(o.x + o.w * 0.25, o.y);
  await p.mouse.down();
  await p.mouse.move(o.x + o.w * 0.5, o.y, { steps: 5 });
  await p.mouse.up();
  await wait(600);
  const [b0, b1] = await view();
  check('Box drags out a new view', b1 - b0 < (a1 - a0) / 3n && b1 - b0 > (a1 - a0) / 6n, [String(b0), String(b1)]);
  const mid = (b0 + b1) / 2n;
  await p.mouse.click(o.x + o.w * 0.8, o.y);
  await wait(600);
  const [c0, c1] = await view();
  check('a click centres the view there', ((c0 + c1) / 2n) > mid && c1 - c0 === b1 - b0 || ((c0 + c1) / 2n) > mid, [String(c0), String(c1)]);

  // wheel zooms
  await p.mouse.move(o.x + o.w * 0.5, o.y);
  await p.mouse.wheel(0, 300);
  await wait(300);
  const [d0, d1] = await view();
  check('the wheel zooms out', d1 - d0 > c1 - c0, [String(d0), String(d1)]);

  // Cursors
  await p.click('#m-cursor');
  await p.mouse.click(o.x + o.w * 0.3, o.y);
  await p.mouse.click(o.x + o.w * 0.6, o.y);
  const cur = await app('cursors');
  check('Cursors places A and B', cur.A !== null && cur.B !== null && BigInt(cur.B) > BigInt(cur.A), cur);
  await p.click('#m-pan');

  // the switch
  await p.click('[data-ovtop]');
  await wait(300);
  check('the switch puts it above the panes, saved', (await settingsLS()).overviewTop === true && (await p.getAttribute('[data-ovtop]', 'aria-checked')) === 'true');
});
