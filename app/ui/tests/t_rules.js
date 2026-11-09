// Rules and checks in one editor: the "Event rules" set holds rules that mark events in a category; a row is a Check
// (pass or fail) or a Rule (events); lists cover magnitude, outside and "past its limits"; rows turn on and off; the
// Events panel's Rules… opens the set; editing a rule event opens its row; categories are edited in the editor.
import { run } from './lib.js';

const RULES = {
  schema: 1,
  name: 'Event rules',
  runs_on_any_file: true,
  info_groups: [],
  check: [
    { name: 'Shock', kind: 'rule', group: 'Alarm', expect: 'abs(accel_x) > 1.6', category: 'alarm', severity: 'fail', grace: 0, settle: 0, enabled: true },
    { name: 'Hot', kind: 'rule', group: 'Alarm', expect: 'past_warn(motor_rpm)', category: 'alarm', severity: 'fail', grace: 5, settle: 0, enabled: true },
    { name: 'Band', kind: 'rule', group: 'Signal loss', expect: 'battery_v < 22 || battery_v > 26', category: 'signal', severity: 'fail', grace: 0, settle: 0, enabled: true },
    { name: 'Low battery', kind: 'rule', group: 'Data quality', expect: 'battery_v < 22.4', category: 'quality', severity: 'fail', grace: 5, settle: 0, enabled: false },
  ],
};

run(
  'rules',
  async ({ p, check, text, until, fake, settingsLS }) => {
    await p.click('[data-rail="checks"]');
    check('the panel lists the rules', await until(() => /Event rules/.test(document.querySelector('#checksBody')?.innerText ?? '') && /Alarm/.test(document.querySelector('#checksBody').innerText)));
    const grp = await p.$$eval('#checksBody .cgrp', (e) => e.map((x) => x.textContent.replace(/\s+/g, ' ').trim()));
    check('rule groups count their events, with no pass/fail', grp.some((g) => /Alarm/.test(g) && /3 events/.test(g) && !/failed/.test(g)), grp);
    check('rules alone give no badge (only checks do)', (await p.$$('#wsTabs [data-chkfile]')).length === 0);
    check('an off rule stays off', await p.evaluate(() => [...document.querySelectorAll('#checksBody .crow')].some((r) => r.classList.contains('off') && /Low battery/.test(r.textContent))));

    // the Events list has them, from rules
    await p.click('[data-rail="events"]');
    const sum = await until(() => /from rules/.test(document.querySelector('#evSum')?.textContent ?? '') && document.querySelector('#evSum').textContent, null, 4000);
    check('rule events are in the Events list, from rules', /3 from rules/.test(sum), sum);
    check('three of them', (await p.$$eval('#eventsPanel .ev', (e) => e.length)) === 3);

    // the Rules… button opens Event rules; old conditions read as lists
    await p.click('#evRules');
    check('Rules… opens the Event rules set', await until(() => document.querySelector('#recDlg [data-sname]')?.value === 'Event rules'));
    const rows = await p.evaluate(() =>
      [...document.querySelectorAll('#recDlg .ckrow')].map((r) => ({
        name: r.querySelector('[data-kf="name"]').value,
        kind: r.querySelector('[data-kf="kind"]').value,
        cat: r.querySelector('[data-kf="cat"]')?.value,
        agg: r.querySelector('[data-xp="agg"]')?.value,
        op: r.querySelector('[data-xp="op"]')?.value,
        v: r.querySelector('input[data-xp="v"]')?.value,
        v2: r.querySelector('input[data-xp="v2"]')?.value,
        on: r.querySelector('[data-kf="on"]').checked,
        label: r.querySelector('.cklab').textContent,
      })),
    );
    const shock = rows.find((r) => r.name === 'Shock');
    check('each is a Rule row with its category and "Mark when"', rows.length === 4 && rows.every((r) => r.kind === 'rule' && r.cat && r.label === 'Mark when'), rows);
    check('"magnitude above" reads as the magnitude of … >', shock.agg === 'abs' && shock.op === '>' && shock.v === '1.6', shock);
    check('"past its warn limits" reads as a list', rows.find((r) => r.name === 'Hot').op === 'warn');
    const band = rows.find((r) => r.name === 'Band');
    check('"outside a band" reads as outside … and …', band.op === 'outside' && band.v === '22' && band.v2 === '26', band);
    check('an off rule shows off', rows.find((r) => r.name === 'Low battery').on === false);

    // change the level; preview and save
    const row = `#recDlg .ckrow[data-k="${rows.indexOf(shock)}"]`;
    check('a rule previews its events', await until((s) => /marks 3 events/.test(document.querySelector(`${s} [data-kp]`).textContent), row, 6000), await text(`${row} [data-kp]`));
    await p.fill(`${row} input[data-xp="v"]`, '2.4');
    await p.dispatchEvent(`${row} input[data-xp="v"]`, 'change');
    check('with the new level', await until(() => window.__tlmFake.checkPreviews.some((c) => c.expect === 'abs(accel_x) > 2.4')));

    // + Rule adds a row with a category; switching to Check shows Expect and severity
    await p.click('#recDlg [data-kaddr]');
    const n = await p.evaluate(() => document.querySelectorAll('#recDlg .ckrow').length - 1);
    const nrow = `#recDlg .ckrow[data-k="${n}"]`;
    check('+ Rule adds a rule row with a category and no severity', await p.evaluate((s) => !!document.querySelector(`${s} [data-kf="cat"]`) && !document.querySelector(`${s} [data-kf="sev"]`), nrow));
    await p.selectOption(`${nrow} [data-kf="kind"]`, 'check');
    check('switching to Check shows Expect and severity', await p.evaluate((s) => document.querySelector(`${s} .cklab`).textContent === 'Expect' && !!document.querySelector(`${s} [data-kf="sev"]`) && !document.querySelector(`${s} [data-kf="cat"]`), nrow));
    await p.click(`${nrow} [data-kdel]`);
    check('a row can be deleted', await p.evaluate(() => document.querySelectorAll('#recDlg .ckrow').length === 4));

    // categories in the editor
    await p.evaluate(() => document.querySelector('#recDlg [data-ckcats] > summary').click());
    check('the editor lists event categories with counts', await p.evaluate(() => document.querySelectorAll('#recDlg [data-cname]').length >= 5 && /\d+ events? · \d+ rules?/.test(document.querySelector('#recDlg .ckcatrows').textContent)));
    const j = await p.evaluate(() => [...document.querySelectorAll('#recDlg [data-cname]')].findIndex((i) => i.value === 'Signal loss'));
    await p.fill(`#recDlg [data-cname="${j}"]`, 'GPS signal');
    await p.dispatchEvent(`#recDlg [data-cname="${j}"]`, 'change');
    check("renaming a category updates the rules' lists and settings", (await p.evaluate(() => [...document.querySelectorAll('#recDlg [data-kf="cat"] option')].some((o) => o.textContent === 'GPS signal'))) && ((await settingsLS()).eventCats ?? []).some((c) => c.name === 'GPS signal'));
    check('the section stays open after a change', await p.evaluate(() => document.querySelector('#recDlg [data-ckcats]').open));
    await p.click('#recDlg [data-cadd]');
    check('+ Category adds one, ready to name', await until(() => document.activeElement?.matches('[data-cname]') && document.activeElement.value === 'New category'));
    p.once('dialog', (d) => d.accept('Thermal'));
    await p.selectOption('#recDlg .ckrow[data-k="0"] [data-kf="cat"]', '__new');
    check('"+ New category…" in a rule makes one and picks it', await until(() => { const s = document.querySelector('#recDlg .ckrow[data-k="0"] [data-kf="cat"]'); return s.options[s.selectedIndex].textContent === 'Thermal'; }));
    await p.click('#recDlg [data-ssave]');
    check('Save keeps the rules in the core', await until(() => window.__tlmFake.checkSetSaves.length === 1));
    const saved = await fake(() => window.__tlmFake.checkSetSaves[0]);
    check('with the new level', saved.set.check.find((c) => c.name === 'Shock').expect === 'abs(accel_x) > 2.4' && saved.replacing === 'Event rules');

    // editing a rule event opens its row
    await p.click('[data-rail="events"]');
    await p.evaluate(() => document.querySelector('#eventsPanel [data-evedit]').click());
    check('editing a rule event opens its rule', await until(() => !!document.querySelector('#recDlg .ckrow') && document.querySelector('#recDlg [data-sname]').value === 'Event rules'));
    await p.click('#recDlg [data-x]');
  },
  {
    files: { '/data/flight.tlm': { channels: ['motor_rpm', 'battery_v', 'flight_mode', 'accel_x', 'gps_sats'] } },
    setupArg: [RULES],
    setup: (fake, [RULES]) => {
      fake.checkSets = [RULES];
      fake.checkOutcomes = {
        Shock: { verdict: 'mark', runs: [[5, 5], [6, 6.5], [7, 7]] },
        Hot: { verdict: 'mark', runs: [] },
        Band: { verdict: 'mark', runs: [] },
      };
    },
  },
);
