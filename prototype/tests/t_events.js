// Events panel: filter by category with chips (toggle, only this, all), Expand all / Collapse all in the title row.
const T = require('./lib');
T.run('events', async ({ p, check, wait, until }) => {
  await p.evaluate(() => document.querySelector('[data-rail="events"]').click()); await wait(400);
  const rows = () => p.evaluate(() => document.querySelectorAll('#evList .ev[data-ev]').length);
  const chips = () => p.evaluate(() => [...document.querySelectorAll('#evList .evcat[data-evcat]')].map(b => b.textContent.replace(/\s+/g, ' ').trim() + (b.getAttribute('aria-pressed') === 'true' ? '' : ' (off)')));
  await until(() => /Check failed/.test(document.getElementById('evList').innerText), null, 20000); await wait(500); // the checks' events are in
  const c0 = await chips(), n0 = await rows();
  check('a chip per category, with counts', c0.length >= 3 && c0.every(x => /\d$/.test(x)), c0);
  const alarm = c0.find(x => /^Alarm/.test(x)), na = +alarm.match(/(\d+)$/)[1];
  await p.click('#evList .evcat[data-evcat="alarm"]'); await wait(450);
  check('clicking a chip hides that category in the list', (await rows()) === n0 - na && (await chips()).some(x => /Alarm.*\(off\)/.test(x)), [n0, na, await rows()]);
  check('All appears to undo it', await p.evaluate(() => !!document.querySelector('#evList [data-evcatall]')));
  await p.click('#evList [data-evcatall]'); await wait(300);
  check('All shows everything again', (await rows()) === n0);
  await p.dblclick('#evList .evcat[data-evcat="alarm"]'); await wait(400);
  check('double-click shows only that category', (await rows()) === na && (await chips()).filter(x => /\(off\)/.test(x)).length === c0.length - 1, await chips());
  await p.click('#evList [data-evcatall]'); await wait(300);
  // collapse / expand all from the title row
  check('Expand all and Collapse all in the Events title row', await p.evaluate(() => !!document.querySelector('[data-panel="events"] .phd #evExpand') && !!document.querySelector('[data-panel="events"] .phd #evCollapse')));
  await p.click('#evCollapse'); await wait(300);
  const g = await p.evaluate(() => ({ by: document.getElementById('evGroup').value, groups: document.querySelectorAll('#evList [data-evg]').length, open: document.querySelectorAll('#evList [data-evg][aria-expanded="true"]').length, rows: document.querySelectorAll('#evList .ev[data-ev]').length }));
  check('Collapse all groups an ungrouped list by category and closes every group', g.by === 'cat' && g.groups >= 3 && g.open === 0 && g.rows === 0, g);
  await p.click('#evExpand'); await wait(300);
  check('Expand all opens them again', await p.evaluate(() => document.querySelectorAll('#evList [data-evg][aria-expanded="false"]').length === 0 && document.querySelectorAll('#evList .ev[data-ev]').length > 0));
  // the chip row scrolls; clicking a chip keeps the row where it was, and All stays first
  const sc = await p.evaluate(() => { const r = document.querySelector('#evList .evcats'); r.scrollLeft = r.scrollWidth; return { left: r.scrollLeft, over: r.scrollWidth > r.clientWidth + 4, first: r.firstElementChild.matches('.evcfix') && r.firstElementChild.firstElementChild.matches('[data-evcatall]') }; });
  const lastChip = await p.evaluate(() => [...document.querySelectorAll('#evList .evcat[data-evcat]')].pop().dataset.evcat);
  await p.evaluate(id => document.querySelector(`#evList .evcat[data-evcat="${CSS.escape(id)}"]`).click(), lastChip); await wait(500);
  const sc2 = await p.evaluate(id => { const r = document.querySelector('#evList .evcats'), c = r.querySelector(`[data-evcat="${CSS.escape(id)}"]`), rb = r.getBoundingClientRect(), cb = c.getBoundingClientRect(); return { left: r.scrollLeft, inView: cb.left >= rb.left - 1 && cb.right <= rb.right + 1, allFirst: r.firstElementChild.matches('.evcfix'), allX: r.firstElementChild.getBoundingClientRect().left - rb.left }; }, lastChip);
  check('All and None are always first, pinned at the left', sc.first && sc2.allFirst && Math.abs(sc2.allX) < 2, [sc, sc2]);
  check('clicking a chip far along the row keeps the row scrolled there, chip in view', sc.over && sc2.left > 0 && sc2.inView, [sc, sc2]);
  await p.evaluate(() => document.querySelector('#evList [data-evcatall]').click()); await wait(300);

  // None hides every category; then a click shows just the ones you want
  await p.evaluate(() => document.querySelector('#evList [data-evcatnone]').click()); await wait(300);
  const none = await p.evaluate(() => ({ rows: document.querySelectorAll('#evList .ev[data-ev]').length, on: document.querySelectorAll('#evList .evcat[data-evcat][aria-pressed="true"]').length, pressed: document.querySelector('#evList [data-evcatnone]').getAttribute('aria-pressed') }));
  check('None hides every category', none.rows === 0 && none.on === 0 && none.pressed === 'true', none);
  await p.evaluate(() => document.querySelector('#evList .evcat[data-evcat="alarm"]').click()); await wait(500);
  const one = await p.evaluate(() => ({ rows: document.querySelectorAll('#evList .ev[data-ev]').length, on: [...document.querySelectorAll('#evList .evcat[data-evcat][aria-pressed="true"]')].map(b => b.dataset.evcat) }));
  check('...then clicking a chip shows just that one', one.rows > 0 && one.on.join() === 'alarm', one);
  await p.evaluate(() => document.querySelector('#evList [data-evcatall]').click()); await wait(300);

  const hdr = await p.evaluate(() => { const q = document.getElementById('evQ').getBoundingClientRect(), g = document.getElementById('evGroup').getBoundingClientRect(), c = document.querySelector('#evList .evcats'); return { line: Math.abs(q.top - g.top) < 2, oneRow: getComputedStyle(c).flexWrap === 'nowrap' }; });
  check('filter and grouping share a line; chips stay on one row', hdr.line && hdr.oneRow, hdr);
  // the rules editor: a rule's group follows its category (no group field); timing hides behind + Timing until set
  await p.click('#evRules'); await wait(600);
  const ed = await p.evaluate(() => { const rows = [...document.querySelectorAll('.chkdlg .ckrow.mk')]; return { n: rows.length, noGrp: rows.filter(r => !r.querySelector('[data-kf="group"]')).length, noIcon: rows.every(r => !r.querySelector('[data-kv]')), timingBtn: rows.filter(r => r.querySelector('[data-ktime]')).length, grace: rows.filter(r => r.querySelector('[data-kf="grace"]')).length }; });
  check('rule rows: no kind icon, group field hidden when it matches the category', ed.n > 0 && ed.noIcon && ed.noGrp > 0, ed);
  check('rows without timing show + Timing; rows with timing show grace and settle', ed.timingBtn > 0 && ed.grace > 0, ed);
  await p.evaluate(() => document.querySelector('.chkdlg .ckrow.mk [data-ktime]').closest('.ckrow').querySelector('[data-ktime]').click()); await wait(300);
  check('+ Timing reveals grace and settle', await p.evaluate(() => document.activeElement && document.activeElement.dataset.kf === 'grace'));
  await p.evaluate(() => document.querySelector('.chkdlg [data-x]').click()); await wait(200);

  // the charts follow the chips (per tab); the link turns that off; a new tab starts from Settings
  await p.evaluate(() => { if (!document.querySelector('#evList .evcats')) document.querySelector('[data-rail="events"]').click(); }); await wait(300);
  const E = () => p.evaluate(() => ({ shown: window.__tlmEvents.shown(), total: window.__tlmEvents.total(), alarm: window.__tlmEvents.cat('alarm') }));
  const e0 = await E();
  await p.evaluate(() => document.querySelector('#evList [data-evcatnone]').click()); await wait(300);
  check('with None, the charts show no events either', (await E()).shown === 0 && e0.shown === e0.total, [e0, await E()]);
  await p.evaluate(() => document.querySelector('#evList [data-evlink]').click()); await wait(300);
  check('unlinking: the charts go back to Settings › Show on charts, the list stays filtered', (await E()).shown === e0.total && (await p.evaluate(() => document.querySelectorAll('#evList .ev[data-ev]').length)) === 0);
  await p.evaluate(() => document.querySelector('#evList [data-evlink]').click()); await wait(200);
  await p.evaluate(() => document.querySelector('#evList [data-evcatall]').click()); await wait(200);
  await p.evaluate(() => document.querySelector('#evList .evcat[data-evcat="alarm"]').click()); await wait(500);
  const e1 = await E();
  check('hiding a category hides it on the charts', e1.alarm === 0 && e1.shown === e0.total - e0.alarm, [e0, e1]);
  await p.click('#wsTabNew'); await wait(250);
  await p.evaluate(() => document.querySelector('#addMenu [data-new]').click()); await wait(300);
  await p.fill('#wsName', 'Fresh'); await p.click('#wsNameOk'); await wait(800);
  check('a new tab starts with every category shown (Settings)', (await E()).alarm === e0.alarm);
  await p.locator('#wsTabs .wstab').nth(0).click(); await wait(600);
  check('the first tab still hides Alarm', (await E()).alarm === 0);
});
