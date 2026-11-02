// Checks: sets with groups, roll-up, badges (Explorer, tab, chips), Check events, stepping, the editor (lists and
// expressions with autocomplete, value names, derived channels), the Checks pane view, folder runs, persistence.
const T = require('./lib');
T.run('checks', async ({ p, check, wait, until, menu, pick, settingsLS }) => {
  const panelText = () => p.evaluate(() => document.getElementById('checksBody').innerText.replace(/\s+/g, ' '));
  await p.evaluate(() => document.querySelector('[data-rail="checks"]').click());
  const ok = await until(() => { const t = document.getElementById('checksBody').innerText; return /Flight acceptance/.test(t) && !/hecking/.test(t); }, null, 30000);
  check('the sample set runs on the open flight', ok);
  const res = await p.evaluate(() => window.__tlmChecks.res('main', 'cs_flight'));
  const v = res ? Object.fromEntries(Object.entries(res).map(([k, x]) => [k, x.v])) : {};
  check('sample checks get verdicts', v.k2 === 'pass' && v.k3 === 'warn' && v.k9 === 'fail' && v.k13 === 'fail' && v.k15 === 'pass', v);
  check('a check on a derived channel runs (Battery power)', v.k7 === 'pass', v.k7);
  check('value names in expressions (flight_mode == \'Mission\')', v.k9 === 'fail' && v.k14 === 'pass');
  check('summary checks give their value', /max 16\.\d+ m\/s/.test(await panelText()) && /duration 9\.00 s \(limit 5\)/.test(await panelText()));
  check('no check errors', !Object.values(res || {}).some(x => x.v === 'err'), Object.entries(res || {}).filter(([, x]) => x.v === 'err').map(([k, x]) => k + ': ' + x.msg));
  const t = await panelText();
  check('groups roll up worst first', /GPS 1 warning/.test(t) && /Power .*Propulsion 1 of 4 failed, 1 warning/.test(t), t.slice(0, 400));
  check('info-only groups are marked and collapsed', /Environment info only/.test(t) && !/Air temperature/.test(t));

  // badges and marks
  const badge = sel => p.evaluate(s => { const b = document.querySelector(s); return b ? b.dataset.cn : null; }, sel);
  check('tab badge', /✗ \d/.test(await badge('#wsTabs .wstab .cbadge')), await badge('#wsTabs .wstab .cbadge'));
  check('Explorer file badge', /✗ \d/.test(await badge('#fileTree [data-file="main"] .cbadge')));
  check('tab and file names unchanged in text', await p.evaluate(() => /^synthetic_flight\.bin$/.test(document.querySelector('#wsTabs .wstab .nm').innerText)));
  check('chips of failing channels are marked', await p.evaluate(() => !!document.querySelector('#stack [data-chip="motor_rpm"].chkbad') && !document.querySelector('#stack [data-chip="accel_x"].chkbad')));
  check('failures become Check events', await p.evaluate(() => [...document.querySelectorAll('#evList *')].some(e => /RPM below 7500 in Mission/.test(e.textContent))));

  // step through violations
  const range = () => p.evaluate(() => document.getElementById('stRange') ? document.getElementById('stRange').textContent : document.querySelector('.trange, #timeRange, .tbrange') ? document.querySelector('.trange, #timeRange, .tbrange').textContent : document.body.innerText.match(/\d\d:\d\d:\d\d\.\d{3} → \d\d:\d\d:\d\d\.\d{3}/)[0]);
  const r0 = await range();
  await p.evaluate(() => [...document.querySelectorAll('#checksBody .crow')].find(r => /RPM below 7500/.test(r.textContent)).click()); await wait(1200);
  const r1 = await range();
  check('clicking a failing check goes to its first violation', r1 !== r0, [r0, r1]);
  await p.evaluate(() => [...document.querySelectorAll('#checksBody .crow')].find(r => /RPM below 7500/.test(r.textContent)).querySelector('[data-cnext]').click()); await wait(1200);
  check('▼ steps to the next one', (await range()) !== r1);
  await p.evaluate(() => [...document.querySelectorAll('#checksBody .crow')].find(r => /RPM below 7500/.test(r.textContent)).querySelector('[data-cprev]').click()); await wait(1200);
  check('▲ steps back to the one before', (await range()) === r1, [r1, await range()]);

  // the editor: lists, expressions with autocomplete, value names, preview
  await p.evaluate(() => document.querySelector('#checksBody [data-cedit="cs_flight"]').click()); await wait(400);
  check('editor lists the checks and where the set runs', await p.evaluate(() => document.querySelectorAll('#recDlg .ckrow').length === 16 && /every file with these channels/.test(document.querySelector('#recDlg .ckatt').textContent)));
  const row5 = '#recDlg .ckrow[data-k="4"]';
  check('simple checks open as lists', await p.evaluate(s => !!document.querySelector(`${s} [data-xp="ch"]`) && document.querySelector(`${s} [data-xp="ch"]`).value === 'battery_v', row5));
  check('complex ones open as an expression', await p.evaluate(() => !!document.querySelector('#recDlg .ckrow[data-k="5"] [data-xt="expect"]')));
  await p.waitForFunction(s => /On synthetic_flight\.bin: passes/.test(document.querySelector(`${s} [data-kp]`).textContent), row5, { timeout: 8000 });
  check('each row previews on the open file', true);
  await p.selectOption(`${row5} [data-xp="v"]`.replace('[data-xp="v"]', 'input[data-xp="v"]'), '').catch(() => {});
  await p.fill(`${row5} input[data-xp="v"]`, '23'); await p.dispatchEvent(`${row5} input[data-xp="v"]`, 'change'); await wait(200);
  await p.waitForFunction(s => /fails/.test(document.querySelector(`${s} [data-kp]`).textContent), row5, { timeout: 8000 });
  check('changing a value updates the preview', /battery_v > 23/.test(await p.evaluate(s => window.__tlmChecks.buildPick(window.__tlmChecks.parsePick('battery_v > 23')), row5)));
  // to text, with autocomplete
  await p.click(`${row5} [data-xm="expect"]`); await wait(200);
  const inp = `${row5} [data-xt="expect"]`;
  check('switches to an expression', (await p.inputValue(inp)) === 'battery_v > 23');
  await p.fill(inp, ''); await p.type(inp, 'cell_5', { delay: 20 }); await wait(200);
  const sugg = await p.evaluate(() => [...document.querySelectorAll('.acpop .aci span')].map(e => e.textContent));
  check('autocomplete suggests channels', sugg.includes('cell_5_v'), sugg);
  await p.keyboard.press('Enter'); await wait(100);
  check('Enter inserts the name', (await p.inputValue(inp)) === 'cell_5_v');
  await p.type(inp, ' > 3.5 && "Batt', { delay: 15 }); await wait(200);
  const sugg2 = await p.evaluate(() => [...document.querySelectorAll('.acpop .aci')].map(e => e.textContent));
  check('derived channels are suggested', sugg2.some(x => /Battery power.*ƒ/.test(x)), sugg2);
  await p.keyboard.press('Enter'); await p.type(inp, ' < 2000', { delay: 15 }); await wait(400);
  check('expression with a derived channel', /cell_5_v > 3\.5 && "Battery power" < 2000/.test(await p.inputValue(inp)), await p.inputValue(inp));
  await p.waitForFunction(s => /On synthetic_flight\.bin: (passes|fails)/.test(document.querySelector(`${s} [data-kp]`).textContent), row5, { timeout: 8000 });
  const whenT = `${row5} [data-xm="when"]`; await p.click(whenT); await wait(200);
  const w = `${row5} [data-xt="when"]`;
  await p.fill(w, ''); await p.type(w, "flight_mode == 'Mi", { delay: 20 }); await wait(200);
  const sugg3 = await p.evaluate(() => [...document.querySelectorAll('.acpop .aci span')].map(e => e.textContent));
  check('value names are suggested after ==', sugg3.includes('Mission'), sugg3);
  await p.keyboard.press('Enter'); await wait(100);
  check('value name inserted with its quote', (await p.inputValue(w)) === "flight_mode == 'Mission'");
  await p.fill(w, "flight_mode == 'Hover'"); await p.dispatchEvent(w, 'input'); await wait(600);
  check('unknown value names are explained', /no value called 'Hover'.*'Mission'/.test(await p.textContent(`${row5} [data-kp]`)), await p.textContent(`${row5} [data-kp]`));
  await p.fill(w, "flight_mode == 'Mission'"); await p.dispatchEvent(w, 'input'); await wait(300);
  // a new check from lists
  await p.click('#recDlg [data-kadd]'); await wait(300);
  const nr = await p.evaluate(() => document.querySelectorAll('#recDlg .ckrow').length - 1), row = `#recDlg .ckrow[data-k="${nr}"]`;
  await p.selectOption(`${row} [data-xp="ch"]`, 'gps_sats'); await wait(200);
  await p.selectOption(`${row} [data-xp="op"]`, '>='); await wait(200);
  await p.fill(`${row} input[data-xp="v"]`, '16'); await p.dispatchEvent(`${row} input[data-xp="v"]`, 'change'); await wait(200);
  await p.fill(`${row} [data-kf="name"]`, 'Sixteen satellites'); await p.dispatchEvent(`${row} [data-kf="name"]`, 'input');
  await p.fill(`${row} [data-kf="group"]`, 'GPS'); await p.dispatchEvent(`${row} [data-kf="group"]`, 'change'); await wait(300);
  await p.waitForFunction(s => /fails/.test(document.querySelector(`${s} [data-kp]`).textContent), row, { timeout: 8000 });
  check('a new check from lists previews', true);
  await p.click('#recDlg [data-ssave]'); await wait(300);
  const ok2 = await until(() => /Sixteen satellites/.test(document.getElementById('checksBody').innerText) && !/hecking/.test(document.getElementById('checksBody').innerText), null, 20000);
  check('saved, run again and shown', ok2);
  check('saved in settings', ((await settingsLS()).checkSets || []).find(s => s.id === 'cs_flight').checks.length === 17);

  // Add check… from a channel
  await p.evaluate(() => document.querySelector('[data-rail="explorer"]').click()); await wait(200);
  await p.evaluate(() => { const b = document.getElementById('treeExpand'); if (b) b.click(); }); await wait(200);
  await p.click('.chan[data-ch="esc_temp_1"] .nm', { button: 'right' }); await wait(150);
  check('channel menu offers Add check…', (await menu()).includes('Add check…'));
  await pick(/^Add check…/); await wait(400);
  check('Add check… opens a new set with the channel', await p.evaluate(() => /esc_temp_1/.test(document.querySelector('#recDlg [data-k="0"]').textContent + document.querySelector('#recDlg [data-k="0"] [data-kf="name"]').value)));
  await p.evaluate(() => document.querySelector('#recDlg [data-x]').click()); await wait(200);

  // the Checks pane view, and a layout that has one
  await p.evaluate(() => document.querySelector('#stack section.pane [data-viewmenu]').click()); await wait(200);
  check('Checks is a pane view', await p.evaluate(() => !!document.querySelector('.viewmenu [data-vm="checks"]')));
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="checks"]').click()); await wait(600);
  check('a Checks pane with channels shows the checks that use them', /No checks use these channels|accel/.test(await p.evaluate(() => document.querySelector('#stack .fchecks').innerText)));
  await p.evaluate(() => document.querySelector('[data-rail="layouts"]').click()); await wait(300);
  const opened = await p.evaluate(() => { const b = [...document.querySelectorAll('#layoutList [data-lopen]')].find(x => /Flight acceptance/.test(x.textContent)); if (b) b.click(); return !!b; }); await wait(800);
  await p.evaluate(() => { const g = document.querySelector('#slotDlg:not([hidden]) [data-go]'); if (g) g.click(); }); await wait(1500);
  check('the Flight acceptance layout opens', opened);
  const paneTxt = await until(() => { const e = document.querySelector('#stack .fchecks'); return e && /Flight acceptance/.test(e.innerText) ? e.innerText : null; }, null, 20000);
  check('its Checks pane shows the whole set by group', paneTxt && /GPS/.test(paneTxt) && /Propulsion/.test(paneTxt), paneTxt && paneTxt.slice(0, 200));
  check('the layout keeps the Checks pane (no channels)', await p.evaluate(() => document.querySelectorAll('#stack .fchecks').length === 1));

  // a gauge shows when the time it reads is inside a violation
  const run = await p.evaluate(() => window.__tlmChecks.res('main', 'cs_flight').k9.runs[0]);
  await p.click('#b-play'); await wait(200);
  await p.evaluate(t => { const r = document.querySelector('#playStrip [data-pscrub]'); r.value = String(Math.round((t[0] + t[1]) / 2 / 7200 * 1000)); r.dispatchEvent(new Event('input', { bubbles: true })); }, run); await wait(500);
  check('gauges mark a failing check at the playhead', await p.evaluate(() => !!document.querySelector('#stack .gw[data-gw="motor_rpm"].chkviol')));
  await p.evaluate(() => document.querySelector('#playStrip [data-pclose]').click()); await wait(200);

  // folder run
  await p.evaluate(() => document.querySelector('[data-rail="checks"]').click()); await wait(300);
  await p.evaluate(() => document.querySelector('#checksBody [data-cfolder]').click()); await wait(300);
  await p.selectOption('#recDlg [data-ffold]', 'flights'); await wait(200);
  await p.click('#recDlg [data-frun]');
  const done = await until(() => { const d = document.getElementById('recDlg'); return d && !/Running/.test(d.querySelector('[data-frun]').textContent) && d.querySelectorAll('.cgrid tbody tr').length >= 4 && ![...d.querySelectorAll('.cgrid tbody tr td:nth-child(2)')].some(td => /—|…/.test(td.textContent)); }, null, 60000);
  check('folder run checks every flight', done);
  const grid = await p.evaluate(() => [...document.querySelectorAll('#recDlg .cgrid tbody tr')].map(tr => [...tr.children].map(td => td.innerText.replace(/\s+/g, ' ').trim()).join(' | ')));
  check('files × groups', grid.length === 4 && grid.some(r => /flight_0418/.test(r)) && grid.some(r => /gps_logger/.test(r)), grid);
  check('a file without the channels says so per group', grid.some(r => /gps_logger.*can't run/i.test(r)), grid.find(r => /gps_logger/.test(r)));
  // Report: pass rates per check across the files, a trend in date order, filter by group
  await p.click('#recDlg [data-ftab="report"]'); await wait(300);
  const rep = await p.evaluate(() => [...document.querySelectorAll('#recDlg .creport tbody tr')].map(tr => ({ t: tr.innerText.replace(/\s+/g, ' ').trim(), pts: tr.querySelectorAll('.tspark circle').length })));
  check('the Report tab lists each check with its pass rate', rep.length >= 4 && rep.every(r => /\d+ of \d+|not run/.test(r.t)), rep.slice(0, 3));
  check('...and a trend with a point per file, oldest first', rep.some(r => r.pts >= 3) && /2026-10-12 → 2026-10-18/.test(await p.evaluate(() => document.querySelector('#recDlg .rbar').innerText)), rep.map(r => r.pts));
  await p.selectOption('#recDlg [data-rgroup]', 'GPS'); await wait(200);
  check('filter by group', await p.evaluate(() => [...document.querySelectorAll('#recDlg .creport tbody tr td:nth-child(2)')].every(td => td.textContent === 'GPS')));
  await p.click('#recDlg [data-ftab="grid"]'); await wait(300);
  
  await p.evaluate(() => { const b = [...document.querySelectorAll('#recDlg .cgrid tbody tr')].find(tr => /flight_0418/.test(tr.textContent)).querySelector('[data-go]'); b.click(); }); await wait(2500);
  check('clicking a cell opens that file', await p.evaluate(() => /flight_0418/.test(document.querySelector('#wsTabs [aria-selected="true"]').innerText)));
  check('its badge is there without running again', /✗|!|✓/.test(await badge('#fileTree [data-file="0418"] .cbadge') || ''));

  // reload: sets, attachments and the layout's Checks pane stay
  await wait(2300); await p.reload(); await wait(3000);
  check('sets persist', ((await settingsLS()).checkSets || []).some(s => s.checks.some(k => k.name === 'Sixteen satellites')));
});
