// Rules and checks in one editor: the old rules became the "Event rules" set; a row is a Check (pass/fail) or a
// Rule (marks events in a category); lists cover magnitude, outside and "past its limits"; on/off; Mark when…
const T = require('./lib');
T.run('rules', async ({ p, check, wait, until, menu, pick, settingsLS, expandTree }) => {
  const S = await settingsLS();
  check('old rules moved into an "Event rules" set', S.rulesMerged === true && (S.checkSets || []).some(s => s.id === 'cs_rules' && s.checks.every(k => k.kind === 'mark')));
  await p.evaluate(() => document.querySelector('[data-rail="checks"]').click());
  const ok = await until(() => { const t = document.getElementById('checksBody').innerText; return /Event rules/.test(t) && /Flight acceptance/.test(t) && !/hecking/.test(t); }, null, 30000);
  check('the panel lists rules and checks', ok);
  check('panel is called Rules & checks', await p.evaluate(() => /Rules & checks/i.test(document.querySelector('[data-panel="checks"] .phd').textContent)));
  const rs = await p.evaluate(() => document.querySelector('#checksBody .cset[data-cset="cs_rules"]').innerText.replace(/\s+/g, ' '));
  check('rule groups count their events, with no pass/fail', /Alarm \d+ events/.test(rs) && !/failed/.test(rs), rs);
  check('rules alone give no badge (only checks do)', await p.evaluate(() => /✗/.test(document.querySelector('#wsTabs .cbadge').dataset.cn)));
  const evs = await p.evaluate(() => { const b = document.querySelector('[data-rail="events"]'); b.click(); return null; }); await wait(400);
  const sum = await p.evaluate(() => document.querySelector('.evsum').textContent);
  check('rule events in the Events list', /from rules/.test(sum), sum);
  const nRule = +(/(\d+) from rules/.exec(sum) || [0, 0])[1];

  // the Rules button opens the editor at Event rules; old conditions read as lists
  await p.click('#evRules'); await wait(500);
  const rows = await p.evaluate(() => [...document.querySelectorAll('#recDlg .ckrow')].map(r => ({ name: r.querySelector('[data-kf="name"]').value, kind: r.querySelector('[data-kf="kind"]').value, agg: (r.querySelector('[data-xp="agg"]') || {}).value, op: (r.querySelector('[data-xp="op"]') || {}).value, v: (r.querySelector('input[data-xp="v"]') || {}).value, cat: (r.querySelector('[data-kf="cat"]') || {}).value, on: r.querySelector('[data-kf="on"]').checked, label: r.querySelector('.ckr2 .cklab').textContent })));
  const shock = rows.find(r => r.name === 'Shock'), hot = rows.find(r => r.name === 'ESC hot'), low = rows.find(r => r.name === 'Low battery');
  check('each rule is a Rule row with its category', rows.length >= 4 && rows.every(r => r.kind === 'mark' && r.cat && r.label === 'Mark when'), rows);
  check('"magnitude above" reads as the magnitude of … >', shock && shock.agg === 'abs' && shock.op === '>' && shock.v === '1.6', shock);
  check('"past its warn limits" reads as lists, with the limits shown', hot && hot.op === 'warn' && await p.evaluate(() => /\(/.test([...document.querySelectorAll('#recDlg .ckrow')].find(r => r.querySelector('[data-kf="name"]').value === 'ESC hot').querySelector('.ckx .muted').textContent)), hot);
  check('an off rule stays off', low && low.on === false);
  // change the Shock level: fewer events
  const k = rows.indexOf(shock), row = `#recDlg .ckrow[data-k="${k}"]`;
  await p.waitForFunction(s => /marks \d+ event/.test(document.querySelector(`${s} [data-kp]`).textContent), row, { timeout: 10000 });
  const before = +(/marks (\d+)/.exec(await p.textContent(`${row} [data-kp]`)) || [0, 0])[1];
  await p.fill(`${row} input[data-xp="v"]`, '2.4'); await p.dispatchEvent(`${row} input[data-xp="v"]`, 'change'); await wait(200);
  await p.waitForFunction(([s, b]) => { const m = /marks (\d+)|no events/.exec(document.querySelector(`${s} [data-kp]`).textContent); return m && (m[1] == null ? 0 : +m[1]) < b; }, [row, before], { timeout: 10000 });
  check('a rule previews its events', before > 0);
  // a new Rule row, and switching a row's kind
  await p.click('#recDlg [data-kaddr]'); await wait(300);
  const last = await p.evaluate(() => { const r = [...document.querySelectorAll('#recDlg .ckrow')].pop(); return { kind: r.querySelector('[data-kf="kind"]').value, cat: !!r.querySelector('[data-kf="cat"]'), sev: !!r.querySelector('[data-kf="sev"]') }; });
  check('+ Rule adds a rule row with a category', last.kind === 'mark' && last.cat && !last.sev, last);
  const n = await p.evaluate(() => document.querySelectorAll('#recDlg .ckrow').length - 1);
  await p.selectOption(`#recDlg .ckrow[data-k="${n}"] [data-kf="kind"]`, 'expect'); await wait(300);
  check('switching to Check shows Expect and severity', await p.evaluate(n => { const r = document.querySelector(`#recDlg .ckrow[data-k="${n}"]`); return r.querySelector('.ckr2 .cklab').textContent === 'Expect' && !!r.querySelector('[data-kf="sev"]') && !r.querySelector('[data-kf="cat"]'); }, n));
  await p.click(`#recDlg .ckrow[data-k="${n}"] [data-kdel]`); await wait(200);
  await p.click('#recDlg [data-ssave]'); await wait(300);
  const fewer = await until(nr => { const m = /(\d+) from rules/.exec((document.querySelector('.evsum') || {}).textContent || ''); return m && +m[1] < nr ? +m[1] : 0; }, nRule, 20000);
  check('saved: fewer Shock events in the list', fewer > 0 && fewer < nRule, [nRule, fewer]);
  check('saved in the set', ((await settingsLS()).checkSets.find(s => s.id === 'cs_rules').checks.find(k => k.name === 'Shock') || {}).expect === 'abs(accel_x) > 2.4');

  // editing a rule event opens its row
  await p.evaluate(() => document.querySelector('[data-rail="events"]').click()); await wait(300);
  await p.evaluate(() => { const r = [...document.querySelectorAll('#evList .evitem')].find(e => e.querySelector('.evname') && e.querySelector('.evname').textContent.trim() === 'Shock' && e.querySelector('[data-evedit]')); r.querySelector('[data-evedit]').click(); }); await wait(500);
  check('editing a rule event opens its rule', await p.evaluate(() => { const f = document.querySelector('#recDlg .ckrow.focus [data-kf="name"]'); return !!f && /Shock/.test(f.value); }));
  await p.evaluate(() => document.querySelector('#recDlg [data-x]').click()); await wait(200);

  // event categories, edited in the editor
  await p.click('#evRules'); await wait(500);
  await p.evaluate(() => document.querySelector('#recDlg [data-ckcats] > summary').click()); await wait(200);
  check('the editor lists event categories with counts', await p.evaluate(() => document.querySelectorAll('#recDlg [data-cname]').length >= 5 && /\d+ events? · \d+ rules?/.test(document.querySelector('#recDlg .ckcatrows').textContent)));
  const j = await p.evaluate(() => [...document.querySelectorAll('#recDlg [data-cname]')].findIndex(i => i.value === 'Signal loss'));
  await p.fill(`#recDlg [data-cname="${j}"]`, 'GPS signal'); await p.dispatchEvent(`#recDlg [data-cname="${j}"]`, 'change'); await wait(300);
  check('renaming a category updates the rules\' lists and settings', await p.evaluate(() => [...document.querySelectorAll('#recDlg [data-kf="cat"] option')].some(o => o.textContent === 'GPS signal')) && ((await settingsLS()).eventCats || []).some(c => c.name === 'GPS signal'));
  check('the section stays open after a change', await p.evaluate(() => document.querySelector('#recDlg [data-ckcats]').open));
  await p.click('#recDlg [data-cadd]'); await wait(300);
  check('+ Category adds one, ready to name', await p.evaluate(() => document.activeElement && document.activeElement.matches('[data-cname]') && document.activeElement.value === 'New category'));
  p.once('dialog', dl => dl.accept('Thermal'));
  await p.selectOption('#recDlg .ckrow[data-k="0"] [data-kf="cat"]', '__new'); await wait(400);
  check('"+ New category…" in a rule makes one and picks it', await p.evaluate(() => { const s = document.querySelector('#recDlg .ckrow[data-k="0"] [data-kf="cat"]'); return s.options[s.selectedIndex].textContent === 'Thermal'; }));
  await p.evaluate(() => document.querySelector('#recDlg [data-x]').click()); await wait(200);
  check('new categories reach the Events panel', await p.evaluate(() => { document.querySelector('[data-rail="events"]').click(); return true; }) && /GPS signal/.test(await p.evaluate(() => document.getElementById('evList').innerText)));

  // Mark when… on a channel
  await p.evaluate(() => document.querySelector('[data-rail="explorer"]').click()); await wait(200);
  await expandTree();
  await p.click('.chan[data-ch="battery_v"] .nm', { button: 'right' }); await wait(150);
  check('channel menu offers Mark when… and Add check…', (await menu()).includes('Mark when…') && (await menu()).includes('Add check…'));
  await pick(/^Mark when…/); await wait(500);
  check('Mark when… adds a rule row to Event rules', await p.evaluate(() => { const r = document.querySelector('#recDlg .ckrow.focus'); return document.querySelector('#recDlg [data-sname]').value === 'Event rules' && !!r && r.querySelector('[data-kf="kind"]').value === 'mark' && /battery_v/.test(r.querySelector('[data-kf="name"]').value); }));
});
