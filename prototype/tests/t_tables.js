// Tables and chart interaction: sort, column select → chart, hide + Columns, box zoom (and its dead zone), Back, Fit, focus mode.
const T = require('./lib');
T.run('tables', async ({ p, check, paneCount, text, wait, until, expandTree }) => {
  const rng = () => text('#rangeTxt');
  // sort by a column from its header menu
  await p.evaluate(() => [...document.querySelectorAll('#stack .tth')].find(h => /accel_x/.test(h.textContent)).querySelector('.thm').click()); await wait(200);
  await p.evaluate(() => [...document.querySelectorAll('.catmenu .mi, .colmenu .mi, .moremenu .mi')].find(b => /Sort ascending/.test(b.textContent)).click()); await wait(1500);
  check('sorted', /sorted by accel_x/.test(await text('#stack .tstat')), await text('#stack .tstat'));
  await p.click('#stack [data-clear]'); await wait(600);
  check('Clear sort and filters', !/sorted by/.test(await text('#stack .tstat')));

  // select columns → chart
  const n0 = await paneCount();
  await p.evaluate(() => document.querySelectorAll('#stack .tth .tck')[0].click()); await wait(150);
  await p.evaluate(() => document.querySelectorAll('#stack .tth .tck')[1].click()); await wait(300);
  check('column selection bar', /2 selected/.test(await text('#stack .tselbar')), await text('#stack .tselbar'));
  await p.evaluate(() => document.querySelector('#stack .tselbar [data-sc="chart"]').click()); await wait(1000);
  check('selected columns → new chart', (await paneCount()) === n0 + 1);

  // hide one column, then Columns lists it hidden
  await p.evaluate(() => document.querySelector('#stack .tth .tck').click()); await wait(200);
  await p.evaluate(() => document.querySelector('#stack .tselbar [data-sc="hide"]').click()); await wait(800);
  check('Columns says 1 hidden', /Columns · 1 hidden/.test(await text('#stack [data-cols]')), await text('#stack [data-cols]'));

  // Columns: clicking a name brings that column into view (and shows it if it was hidden), in a table of 12 channels
  await expandTree();
  { const ids = ['accel_x', 'accel_y', 'accel_z', 'gyro_x', 'gyro_y', 'gyro_z', 'roll', 'pitch', 'yaw', 'motor_rpm', 'battery_v', 'current_a'];
    await p.click(`.chan[data-ch="${ids[0]}"] .nm`); for (const id of ids.slice(1)) await p.click(`.chan[data-ch="${id}"] .nm`, { modifiers: ['Control'] });
    await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => /^Table$/.test(b.textContent)).click()); await wait(1500); }
  const T0 = await p.evaluate(() => { const secs = [...document.querySelectorAll('#stack section.pane')].filter(s => s.querySelector('.tscroll')); let best = 0, bw = -1; secs.forEach((s, k) => { const n = s.querySelectorAll('.tth[data-cid]').length; if (n > bw) { bw = n; best = k; } }); return [...document.querySelectorAll('#stack section.pane')].indexOf(secs[best]); });
  await p.setViewportSize({ width: 820, height: 900 }); await wait(900); // narrow, so the table is wider than its pane
  const inPane = (fn, arg) => p.evaluate(([k, src, a]) => { const pane = document.querySelectorAll('#stack section.pane')[k]; return (new Function('pane', 'a', src))(pane, a); }, [T0, fn, arg]);
  const inView = id => inPane(`const sc = pane.querySelector('.tscroll'), h = pane.querySelector('.tth[data-cid="' + CSS.escape(a) + '"]'); if (!sc || !h) return false; const r = sc.getBoundingClientRect(), b = h.getBoundingClientRect(); return b.left >= r.left - 1 && b.right <= r.right + 1;`, id);
  await inPane(`pane.querySelector('[data-cols]').click()`); await wait(300);
  const lastId = await p.evaluate(() => [...document.querySelectorAll('.colpop [data-colgo]')].pop().dataset.colgo);
  const overflows = await inPane(`const sc = pane.querySelector('.tscroll'); return sc.scrollWidth > sc.clientWidth + 2;`);
  check('the widest table overflows, so its last column starts out of view', overflows && !(await inView(lastId)), [overflows, lastId]);
  await p.evaluate(id => document.querySelector(`.colpop [data-colgo="${CSS.escape(id)}"]`).click(), lastId); await wait(400);
  check('clicking its name scrolls it into view', await inView(lastId));
  { const left = () => inPane(`return Math.round(pane.querySelector('.tscroll').scrollLeft)`);
    const l0 = await left();
    await p.evaluate(() => { const l = document.querySelector('.colpop .list'); l.scrollTop = l.scrollHeight; });
    const top0 = await p.evaluate(() => document.querySelector('.colpop .list').scrollTop);
    await inPane(`pane.querySelector('[data-cols]').click()`); await wait(700);
    check('closing with the Columns button keeps the table scrolled to the column', !(await p.evaluate(() => !!document.querySelector('.colpop'))) && (await left()) === l0 && await inView(lastId), [l0, await left()]);
    await inPane(`pane.querySelector('[data-cols]').click()`); await wait(300);
    const top1 = await p.evaluate(() => document.querySelector('.colpop .list').scrollTop);
    check('reopening Columns keeps its list where you left it', top0 > 0 && Math.abs(top1 - top0) < 2, [top0, top1]);
    await p.mouse.click(5, 895); await wait(700);
    check('clicking away keeps the table scrolled too', (await left()) === l0, [l0, await left()]);
    await inPane(`pane.querySelector('[data-cols]').click()`); await wait(300); }
  await p.evaluate(() => { const i = document.querySelector('.colpop [data-col]:not([data-col="ev"])'); i.checked = false; i.dispatchEvent(new Event('change')); });
  const hidId = await p.evaluate(() => document.querySelector('.colpop [data-col]:not([data-col="ev"])').dataset.col);
  await p.evaluate(() => document.querySelector('.colpop [data-done]').click()); await wait(900);
  check('(a column hidden)', !(await inView(hidId)));
  await inPane(`pane.querySelector('[data-cols]').click()`); await wait(300);
  await p.evaluate(id => document.querySelector(`.colpop [data-colgo="${CSS.escape(id)}"]`).click(), hidId); await wait(900);
  check('a hidden column is shown and brought into view by its name', await inView(hidId));
  await p.evaluate(() => { const d = document.querySelector('.colpop [data-done]'); d && d.click(); }); await wait(300);
  // Autofit › Fit the columns in view: nothing in view is cut off
  await inPane(`pane.querySelector('[data-autofit]').click()`); await wait(200);
  check('Autofit offers the columns in view first', /^Fit the columns in view/.test(await p.evaluate(() => document.querySelector('.colmenu .mi').textContent)));
  await p.evaluate(() => document.querySelector('.colmenu .mi').click()); await wait(400);
  const cut = await inPane(`const sc = pane.querySelector('.tscroll'), r = sc.getBoundingClientRect(), pin = [...pane.querySelectorAll('.tth.pin')].reduce((x, h) => Math.max(x, h.getBoundingClientRect().right), r.left), out = [];
    pane.querySelectorAll('.tth[data-cid]').forEach(h => { const b = h.getBoundingClientRect(); if (b.right <= pin || b.left >= r.right) return; if (b.right > r.right || (b.left < pin && !h.classList.contains('pin'))) return; const l = h.querySelector('.tlab'); if (l && l.scrollWidth > l.clientWidth + 1) out.push('head ' + h.dataset.cid); });
    pane.querySelectorAll('.trw:not([hidden]) > span[data-col]').forEach(c => { const b = c.getBoundingClientRect(); if (b.right > r.right || b.left < pin) return; if (c.scrollWidth > c.clientWidth + 1) out.push('cell ' + c.dataset.col); });
    return [...new Set(out)];`);
  check('after fitting, no header or value fully in view is cut off', cut.length === 0, cut);
  await p.setViewportSize({ width: 1440, height: 900 }); await wait(900);

  // box zoom: a tiny drag is ignored, with a note; a real one zooms; Back returns
  const plot = await p.locator('#stack section.pane .plot').first().boundingBox();
  const r0 = await rng();
  await p.mouse.move(plot.x + 300, plot.y + 50); await p.mouse.down(); await p.mouse.move(plot.x + 302, plot.y + 50, { steps: 2 }); await p.mouse.up(); await wait(300);
  check('a 2 px drag does not zoom', (await rng()) === r0);
  await p.mouse.move(plot.x + 200, plot.y + 50); await p.mouse.down(); await p.mouse.move(plot.x + 420, plot.y + 50, { steps: 6 }); await p.mouse.up(); await wait(900);
  const r1 = await rng();
  check('box zoom narrows the view', r1 !== r0 && !/02:00:00.000$/.test(r1), r1);
  await p.click('#b-back'); await wait(900);
  check('Back returns to the full view', await until(r => document.getElementById('rangeTxt').textContent.replace(/\s+/g, ' ').trim() === r, r0, 4000), [r0, await rng()]); // (once its glide has settled)
  await p.mouse.move(plot.x + 200, plot.y + 50); await p.mouse.down(); await p.mouse.move(plot.x + 420, plot.y + 50, { steps: 6 }); await p.mouse.up(); await wait(900);
  await p.click('#b-fit'); await wait(900);
  check('Fit shows the whole recording', await until(r => document.getElementById('rangeTxt').textContent.replace(/\s+/g, ' ').trim() === r, r0, 4000));

  // focus mode
  await p.keyboard.press('Control+.'); await wait(300);
  check('focus mode hides the chrome', await p.evaluate(() => document.getElementById('app').classList.contains('focus')));
  await p.keyboard.press('Escape'); await wait(300);
  check('Esc leaves focus mode', await p.evaluate(() => !document.getElementById('app').classList.contains('focus')));
}, { settings: { fileOpenNew: 'starterboth', fileOpen: 'starterboth' }, wait: 6000 });
