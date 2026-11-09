// Rules & checks: the rail panel lists the sets that run on the open file (sets, groups, rows with verdicts), rolls up
// worst first, collapses info-only groups, steps through violations (▲ ▼), selects rows, marks failing channels (Explorer,
// chips) and files (Explorer, tab) and makes read-only Check failed events; the editor changes a row (Lists and ƒ with
// autocomplete), previews it on the open file and saves; Add check… and Mark when… start a row from a channel.
import { run } from './lib.js';

const SET = {
  schema: 1,
  name: 'Flight acceptance',
  runs_on_any_file: true,
  info_groups: ['Environment'],
  check: [
    { name: 'RPM below 7500 in Mission', kind: 'check', group: 'Propulsion', expect: 'motor_rpm < 7500', when: "flight_mode == 'Mission'", severity: 'fail', grace: 0.5, settle: 0, enabled: true },
    { name: 'Battery above 21 V', kind: 'check', group: 'Power', expect: 'battery_v > 21', severity: 'fail', grace: 1, settle: 0, enabled: true },
    { name: 'Peak current under 40 A', kind: 'check', group: 'Power', expect: 'max(motor_rpm) - min(motor_rpm) < 9000', severity: 'warn', grace: 0, settle: 0, enabled: true },
    { name: 'Air temperature under 30 °C', kind: 'check', group: 'Environment', expect: 'battery_v < 30', severity: 'fail', grace: 0, settle: 0, enabled: true },
  ],
};
const RULES = {
  schema: 1,
  name: 'Event rules',
  runs_on_any_file: true,
  info_groups: [],
  check: [
    { name: 'Shock', kind: 'rule', group: 'Alarm', expect: 'abs(motor_rpm) > 1.6', category: 'alarm', severity: 'fail', grace: 0, settle: 0, enabled: true },
    { name: 'ESC hot', kind: 'rule', group: 'Alarm', expect: 'past_warn(battery_v)', category: 'alarm', severity: 'fail', grace: 5, settle: 0, enabled: true },
    { name: 'Low battery', kind: 'rule', group: 'Data quality', expect: 'battery_v < 22.4', category: 'quality', severity: 'fail', grace: 5, settle: 0, enabled: false },
  ],
};

run(
  'checks',
  async ({ p, check, text, until, fake, wait, settingsLS }) => {
    const rail = (n) => p.click(`[data-rail="${n}"]`);
    const body = () => text('#checksBody');
    const view = () => p.evaluate(() => window.__tlmApp.view().t0);
    await rail('checks');
    check('the Rules & checks panel opens from the rail', await until(() => !!document.querySelector('#checksBody')));
    check('it is called Rules & checks', /rules & checks/i.test(await text('#checksPanel .phd')), await text('#checksPanel .phd'));
    check('the sets that run on the file are listed', await until(() => /Flight acceptance/.test(document.querySelector('#checksBody').innerText) && /Event rules/.test(document.querySelector('#checksBody').innerText)), await body());
    const asked = await fake(() => window.__tlmFake.checkRuns.at(-1));
    check('the core was asked to run both sets on the open file', asked.path === '/data/flight.tlm' && asked.sets.includes('Flight acceptance') && asked.sets.includes('Event rules'), asked);

    // roll-up
    const t = await body();
    const grp = await p.$$eval('#checksBody .cgrp', (e) => e.map((x) => x.textContent.replace(/\s+/g, ' ').trim()));
    check('Power: one warned, none failed', grp.some((g) => /Power/.test(g) && /1 warning/.test(g) && !/failed/.test(g)), grp);
    check('Propulsion fails', grp.some((g) => /Propulsion/.test(g) && /1 of 1 failed/.test(g)), grp);
    check('an info-only group is marked and collapsed', grp.some((g) => /Environment.*info only/.test(g)) && !/Air temperature/.test(t), grp);
    check('rule groups count their events, with no pass/fail', grp.some((g) => /Alarm/.test(g) && /\d+ events?/.test(g) && !/failed/.test(g)), grp);
    check('the set says what failed, leaving out info-only groups', /Flight acceptance\s*✗\s*1 failed, 1 warning/.test(t), t);

    // badges and marks
    const cn = (sel) => p.evaluate((s) => document.querySelector(s)?.dataset.cn ?? null, sel);
    check('tab badge counts what failed', /✗ 1/.test((await cn('#wsTabs .wstab [data-chkfile]')) ?? ''), await cn('#wsTabs .wstab [data-chkfile]'));
    await rail('explorer');
    check('Explorer shows the same badge on the file', /✗ 1/.test((await cn('#explorer [data-chkfile]')) ?? ''), await cn('#explorer [data-chkfile]'));
    check('the file name keeps its own text', await p.evaluate(() => /^flight\.tlm$/.test(document.querySelector('#wsTabs .wstab .nm').innerText)));
    check('Explorer marks channels used by a failing check', await until(() => !!document.querySelector('#explorer .row.chan [data-chkdot]')));
    check('chips of failing channels are marked', await until(() => !!document.querySelector('#stack .chip [data-chkdot]')));

    // Check events
    await rail('events');
    const sum = await until(() => document.querySelector('#evSum')?.textContent.includes('from checks') && document.querySelector('#evSum').textContent, null, 5000);
    check('failures become Check failed events, read-only, next to rule events', /3 from checks/.test(sum) && /from rules/.test(sum), sum);
    check('with their own category', await p.evaluate(() => [...document.querySelectorAll('#eventsPanel .ev')].some((e) => /RPM below 7500/.test(e.textContent))));
    check('no markers or ranges of yours were saved for them', (await fake(() => window.__tlmFake.eventSaves.length)) === 0);
    await rail('checks');

    // stepping
    const row = (n) => p.locator('#checksBody .crow', { hasText: n }).first();
    const v0 = await view();
    await row('RPM below 7500').click({ position: { x: 40, y: 8 } });
    check('clicking a failing check goes to its first violation', await until((a) => window.__tlmApp.view().t0 !== a, v0, 4000));
    await wait(900);
    const v1 = await view();
    await row('RPM below 7500').hover();
    await row('RPM below 7500').locator('[data-cnext]').click();
    check('▼ steps to the next one', await until((a) => window.__tlmApp.view().t0 !== a, v1, 4000));
    await wait(900);
    const v2 = await view();
    await row('RPM below 7500').locator('[data-cprev]').click();
    check('▲ steps back to the one before', await until((a) => window.__tlmApp.view().t0 === a, v1, 4000), [v1, v2, await view()]);

    // selecting
    const selN = () => p.evaluate(() => document.querySelectorAll('#checksBody .crow.sel').length);
    check('the row is selected', (await selN()) === 1);
    await row('RPM below 7500').click({ position: { x: 40, y: 8 } });
    check('a second click deselects it', (await selN()) === 0);
    await row('RPM below 7500').click({ position: { x: 40, y: 8 } });
    await p.keyboard.press('Escape');
    check('Esc deselects', (await selN()) === 0);
    await row('RPM below 7500').click({ position: { x: 40, y: 8 } });
    await p.click('#checksBody .cknote, #checksBody .ckbar', { position: { x: 4, y: 4 } }).catch(() => {});
    check('a click on empty space in the panel deselects', (await selN()) === 0);
    const tip = await row('RPM below 7500').getAttribute('data-tip');
    check('the tooltip says what it expects, when, and which channels', /Expect motor_rpm < 7500/.test(tip) && /when flight_mode == 'Mission'/.test(tip) && /channels: motor_rpm/.test(tip), tip);
    await row('RPM below 7500').hover();
    await row('RPM below 7500').locator('[data-cchan]').click();
    const items = await p.$$eval('.chmenu .mi', (e) => e.map((x) => x.textContent));
    check("a row's channels button offers a new chart, the selected pane, a new table", items.some((x) => /in a new chart$/.test(x)) && items.some((x) => /to the selected pane/.test(x)) && items.some((x) => /in a new table/.test(x)), items);
    await p.keyboard.press('Escape');
    await row('RPM below 7500').click({ button: 'right', position: { x: 40, y: 8 } });
    const ctx = await p.$$eval('.chmenu .mi', (e) => e.map((x) => x.textContent));
    check('right-click has the same actions and Edit', ctx.some((x) => /Edit this check/.test(x)) && ctx.some((x) => /Select|Deselect/.test(x)), ctx);
    await p.keyboard.press('Escape');

    // the editor
    await p.click('#checksBody [data-cedit="Flight acceptance"]');
    check('the editor lists the checks and where the set runs', await until(() => document.querySelectorAll('#recDlg .ckrow').length === 4 && /every file with these channels/i.test(document.querySelector('#recDlg .ckatt').textContent)));
    const r2 = '#recDlg .ckrow[data-k="1"]';
    check('simple checks open as lists', (await p.inputValue(`${r2} [data-xp="ch"]`)) === 'battery_v' && (await p.inputValue(`${r2} input[data-xp="v"]`)) === '21');
    check('complex ones open as an expression', await p.evaluate(() => !!document.querySelector('#recDlg .ckrow[data-k="2"] [data-xt="expect"]')));
    check('each row previews on the open file', await until(() => /On flight\.tlm: /.test(document.querySelector('#recDlg .ckrow[data-k="1"] [data-kp]').textContent), null, 6000));
    const n0 = await fake(() => window.__tlmFake.checkPreviews.length);
    await p.fill(`${r2} input[data-xp="v"]`, '23');
    await p.dispatchEvent(`${r2} input[data-xp="v"]`, 'change');
    check('changing a value previews again', await until((n) => window.__tlmFake.checkPreviews.length > n, n0, 6000));
    check('with the new expression', (await fake(() => window.__tlmFake.checkPreviews.at(-1).expect)) === 'battery_v > 23');
    // to text, with autocomplete
    await p.click(`${r2} [data-xm="expect"]`);
    const inp = `${r2} [data-xt="expect"]`;
    check('switches to an expression', (await p.inputValue(inp)) === 'battery_v > 23');
    await p.fill(inp, '');
    await p.type(inp, 'motor_r', { delay: 20 });
    const sugg = await until(() => [...document.querySelectorAll('#recDlg .acpop .aci span')].map((e) => e.textContent), null, 2000);
    check('autocomplete suggests channels', sugg.includes('motor_rpm'), sugg);
    await p.keyboard.press('Enter');
    check('Enter inserts the name', (await p.inputValue(inp)) === 'motor_rpm');
    await p.type(inp, ' > 3', { delay: 15 });
    // a new row from lists
    await p.click('#recDlg [data-kadd]');
    const nr = await p.evaluate(() => document.querySelectorAll('#recDlg .ckrow').length - 1);
    const nrow = `#recDlg .ckrow[data-k="${nr}"]`;
    check('+ Check adds a row with Expect and a severity', await p.evaluate((s) => !!document.querySelector(`${s} [data-kf="sev"]`) && !document.querySelector(`${s} [data-kf="cat"]`), nrow));
    await p.selectOption(`${nrow} [data-xp="ch"]`, 'battery_v');
    await p.selectOption(`${nrow} [data-xp="op"]`, '>=');
    await p.fill(`${nrow} input[data-xp="v"]`, '16');
    await p.dispatchEvent(`${nrow} input[data-xp="v"]`, 'change');
    await p.fill(`${nrow} [data-kf="name"]`, 'Sixteen volts');
    await p.fill(`${nrow} [data-kf="group"]`, 'Power');
    await p.click('#recDlg [data-ssave]');
    check('Save keeps the set in the core', await until(() => window.__tlmFake.checkSetSaves.length === 1), await fake(() => window.__tlmFake.checkSetSaves.length));
    const saved = await fake(() => window.__tlmFake.checkSetSaves[0]);
    check('with the edits', saved.replacing === 'Flight acceptance' && saved.set.check.length === 5 && saved.set.check[1].expect === 'motor_rpm > 3' && saved.set.check[4].expect === 'battery_v >= 16' && saved.set.check[4].name === 'Sixteen volts', saved.set.check.map((k) => k.expect));
    check('and runs it again, showing the new check', await until(() => /Sixteen volts/.test(document.querySelector('#checksBody').innerText), null, 6000), await body());

    // the Checks pane view, on the pane that holds flight_mode and accel_x
    const vm = () => p.evaluate(() => [...document.querySelectorAll('#stack section.pane')].find((s) => s.querySelector('.chip[data-ch="2"]')).querySelector('[data-vmenu]').click());
    await vm();
    check('Checks is a pane view', (await p.$$eval('.menu [data-vm]', (e) => e.map((x) => x.dataset.vm))).includes('checks'));
    await p.click('.menu [data-vm="checks"]');
    check('a Checks pane with channels shows only the checks that use them', await until(() => { const t = document.querySelector('#stack [data-fchecks]')?.innerText ?? ''; return /RPM below 7500/.test(t) && !/Battery above/.test(t); }), await text('#stack [data-fchecks]'));
    await vm();
    await p.click('.menu [data-vm="chart"]');

    // Add check… from a channel
    await rail('explorer');
    await p.click('#explorer .row.chan[data-ch="1"] .nm', { button: 'right' });
    const menu = await p.$$eval('.chanmenu .mi', (e) => e.map((x) => x.textContent));
    check('the channel menu offers Mark when… and Add check…', menu.includes('Mark when…') && menu.includes('Add check…'), menu);
    await p.evaluate(() => [...document.querySelectorAll('.chanmenu .mi')].find((x) => x.textContent === 'Add check…').click());
    check('Add check… opens a new set with the channel', await until(() => document.querySelectorAll('#recDlg .ckrow').length === 1 && /battery_v/.test(document.querySelector('#recDlg .ckrow [data-kf="name"]').value)));
    await p.click('#recDlg [data-x]');

    // Mark when… adds a rule to Event rules
    await p.click('#explorer .row.chan[data-ch="1"] .nm', { button: 'right' });
    await p.evaluate(() => [...document.querySelectorAll('.chanmenu .mi')].find((x) => x.textContent === 'Mark when…').click());
    check('Mark when… adds a rule row to Event rules', await until(() => document.querySelector('#recDlg [data-sname]')?.value === 'Event rules' && document.querySelector('#recDlg .ckrow.focus [data-kf="kind"]')?.value === 'rule'));
    check('prefilled from the channel', await p.evaluate(() => /battery_v/.test(document.querySelector('#recDlg .ckrow.focus [data-kf="name"]').value)));
    await p.click('#recDlg [data-x]');
    check('nothing was saved by the cancelled edits', (await fake(() => window.__tlmFake.checkSetSaves.length)) === 1);
  },
  {
    files: { '/data/flight.tlm': { channels: ['motor_rpm', 'battery_v', 'flight_mode', 'accel_x', 'gps_sats'] } },
    setupArg: [SET, RULES],
    setup: (fake, [SET, RULES]) => {
      fake.checkSets = [SET, RULES];
      fake.checkOutcomes = {
        'RPM below 7500 in Mission': { verdict: 'fail', runs: [[3, 4.5], [10, 11]], worst: 7995 },
        'Battery above 21 V': { verdict: 'pass' },
        'Peak current under 40 A': { verdict: 'warn', value: 9500, function: 'max', limit: 9000, runs: [] },
        'Air temperature under 30 °C': { verdict: 'fail', runs: [[20, 21]] },
        Shock: { verdict: 'mark', runs: [[5, 5], [6, 6.5], [7, 7]] },
        'ESC hot': { verdict: 'mark', runs: [[12, 14]] },
      };
    },
  },
);
