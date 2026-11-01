// Phone: the page scrolls as one, adding panes or channels keeps your place, every chart has time labels,
// and a selected row shows its actions on their own line.
const T = require('./lib');
T.run('phone', async ({ p, check, paneCount, expandTree, wait }) => {
  check('no hover on this device', await p.evaluate(() => matchMedia('(hover: none)').matches));
  const heights = await p.evaluate(() => [...document.querySelectorAll('#stack section.pane .plot')].map(e => Math.round(e.getBoundingClientRect().height)));
  check('every chart has time labels', heights.length >= 3 && heights.every(h => h === heights[0]), heights);

  const top = () => p.evaluate(() => Math.round(document.scrollingElement.scrollTop));
  await p.evaluate(() => { document.scrollingElement.scrollTop = 99999; }); await wait(300);
  const t0 = await top();
  const n0 = await paneCount();
  await p.evaluate(() => [...document.querySelectorAll('button')].find(b => /Add pane/.test(b.textContent) && b.offsetParent).click()); await wait(800);
  check('Add pane adds one', (await paneCount()) === n0 + 1);
  check('...and keeps your place', Math.abs((await top()) - t0) < 40, [t0, await top()]);

  await expandTree();
  await p.evaluate(() => document.querySelector('.chan[data-ch="motor_3_rpm"]').scrollIntoView({ block: 'center' })); await wait(200);
  const t1 = await top();
  await p.evaluate(() => document.querySelector('.chan[data-ch="motor_3_rpm"] .tog').click()); await wait(800);
  check('ticking a channel keeps your place', Math.abs((await top()) - t1) < 40, [t1, await top()]);

  // a long name on a selected row wraps instead of being cut by its buttons
  for (let i = 0; i < 12; i++) { const more = await p.evaluate(() => { const g = document.querySelector('#fileTree .ghead[aria-expanded="false"], #tree .ghead[aria-expanded="false"]'); if (g) g.click(); return !!g; }); if (!more) break; await wait(100); }
  const S = '.chan[data-ch="bms_pack_module_03_cell_12_voltage_mv"]';
  await p.evaluate(s => document.querySelector(s).scrollIntoView({ block: 'center' }), S); await wait(150);
  await p.tap(S + ' .nm'); await wait(400);
  const r = await p.evaluate(s => { const n = document.querySelector(s + ' .nm'); return { cut: n.scrollWidth > n.clientWidth + 1, actions: getComputedStyle(document.querySelector(s + ' .hov')).display !== 'none' }; }, S);
  check('selected long name is not cut off', !r.cut, r);
  check('its actions show', r.actions, r);
}, { phone: true, wait: 3000 });
