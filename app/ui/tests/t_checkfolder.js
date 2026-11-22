// The Report tab (S5.3-15): Run on folder… opens a tab of its own in the strip that runs one set on every recording in
// Explorer's folder: files × groups in a grid; each check's pass rate, sorted and filtered by group, with a trend in date
// order (a summary value as a line; a point opens its file); results as CSV; kept with the set and in the session, back
// as it was without running; a click on a cell opening that file at its first violation in the group.
import { run } from './lib.js';

const DIR = '/data/flights';
const CH = ['motor_rpm', 'battery_v', 'flight_mode'];
const files = {
  [`${DIR}/flight_0412.tlm`]: { channels: CH },
  [`${DIR}/flight_0418.tlm`]: { channels: CH },
  [`${DIR}/flight_0415.tlm`]: { channels: CH },
  [`${DIR}/gps_logger.csv`]: { channels: ['lat', 'lon'] },
  // in a sub-folder: the run goes into folders too (S5.3-15)
  [`${DIR}/archive/flight_0301.tlm`]: { channels: CH },
};
const day = (d) => String(BigInt(Date.UTC(2026, 9, d)) * 1000000n);
const SET = {
  schema: 1,
  name: 'Flight acceptance',
  runs_on_any_file: false,
  info_groups: [],
  check: [
    { name: 'RPM below 7500', kind: 'check', group: 'Propulsion', expect: 'motor_rpm < 7500', severity: 'fail', grace: 0, settle: 0, enabled: true },
    { name: 'Battery above 21 V', kind: 'check', group: 'Power', expect: 'battery_v > 21', severity: 'fail', grace: 0, settle: 0, enabled: true },
  ],
};

run(
  'checkfolder',
  async ({ p, check, until, fake, text, settingsLS }) => {
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.dir, #explorer .row.file').length >= 4);
    await p.click('[data-rail="checks"]');
    await p.click('#checksBody [data-cfolder]');
    // S5.3-15 (owner Nov 17): the report is a tab of its own in the strip
    check('Run on folder opens a Report tab in the strip, in front', await until(() => /Report · Flight acceptance/.test(document.querySelector('#wsTabs [aria-selected="true"]')?.innerText ?? '')));
    check('it fills the centre, with the set to report', !!(await p.$('#reportTab')) && (await p.$$('#reportTab [data-ffold-set] option')).length === 1);
    await p.click('#reportTab [data-frun]');
    const done = await until(() => document.querySelectorAll('#reportTab .cgrid tbody tr').length === 5 && document.querySelector('#reportTab [data-frun]').textContent === 'Run again', null, 10000);
    check('the folder run checks every recording', done);
    const grid = await p.evaluate(() => [...document.querySelectorAll('#reportTab .cgrid tbody tr')].map((tr) => [...tr.children].map((td) => td.innerText.replace(/\s+/g, ' ').trim()).join(' | ')));
    check("files × groups, the sub-folder's too", grid.length === 5 && grid.some((r) => /flight_0301/.test(r)) && grid.some((r) => /flight_0418/.test(r)) && grid.some((r) => /gps_logger/.test(r)), grid);
    check('the core ran the set once on each, by path', (await fake(() => window.__tlmFake.checkRuns.filter((r) => r.folder).length)) === 5);
    check('a failing file shows Fail in its group', grid.some((r) => /flight_0418.*Fail/.test(r)), grid);
    check("a file without the channels says it can't run", grid.some((r) => /gps_logger.*can't run/i.test(r)), grid.find((r) => /gps_logger/.test(r)));

    // pass rates and trends
    await p.click('#reportTab [data-ftab="report"]');
    const rows = () => p.evaluate(() => [...document.querySelectorAll('#reportTab .creport tbody tr')].map((tr) => ({ t: tr.innerText.replace(/\s+/g, ' ').trim(), pts: tr.querySelectorAll('.tspark circle').length, line: !!tr.querySelector('[data-tline]') })));
    const rep = await rows();
    check('each check with its pass rate', rep.length === 2 && rep.every((r) => /\d+ of \d+ files|not run/.test(r.t)), rep);
    const rpm = rep.find((r) => /RPM below 7500/.test(r.t));
    check('RPM passed in 3 of 4 files (the one without channels did not run)', /3 of 4 files/.test(rpm.t), rpm.t);
    check('a trend with a point per file, oldest first', rpm.pts === 5 && /2026-10-12 → 2026-10-18/.test(await text('#reportTab .rbar')), [rep.map((r) => r.pts), await text('#reportTab .rbar')]);
    check("a summary value's trend is a line, with its range", rpm.line && /6500 – 7600/.test(rpm.t), rpm);
    // sort by pass rate: Battery (4 of 4) after RPM (3 of 4) going up
    await p.click('#reportTab [data-rsort="rate"]');
    const byRate = (await rows()).map((r) => r.t.split(' ')[0]);
    await p.click('#reportTab [data-rsort="rate"]');
    const byRate2 = (await rows()).map((r) => r.t.split(' ')[0]);
    check('sorts by pass rate, both ways', byRate.join() === 'RPM,Battery' && byRate2.join() === 'Battery,RPM', [byRate, byRate2]);
    await p.selectOption('#reportTab [data-rgroup]', 'Power');
    check('filter by group', await p.evaluate(() => [...document.querySelectorAll('#reportTab .creport tbody tr td:nth-child(2)')].every((td) => td.textContent === 'Power')));
    await p.selectOption('#reportTab [data-rgroup]', '');

    // CSV
    await fake(() => (window.__tlmFake.savePick = '/tmp/results.csv'));
    await p.click('#reportTab [data-fcsv]');
    const w = await until(() => window.__tlmFake.written, null, 3000);
    check('results as CSV go where the person chose', !!w && w.path === '/tmp/results.csv' && /^file,group,check,kind,verdict/.test(w.text) && /flight_0418.*RPM below 7500,check,fail/.test(w.text), w && w.text.slice(0, 200));

    // the report is kept with the set: another tab in front, then the Report tab again, and it is there without running
    check('the tab says when it was run', !!(await p.$('#reportTab [data-fwhen]')));
    await p.click('[data-rail="explorer"]');
    await p.dblclick('#explorer .row.file[data-path$="flight_0412.tlm"]');
    check('a file opened in front hides the report and shows its panes', await until(() => !document.querySelector('#reportTab') && document.querySelectorAll('#stack [data-pane]').length > 0));
    await p.evaluate(() => [...document.querySelectorAll('#wsTabs [role="tab"], #wsTabs .wstab')].find((t) => /Report/.test(t.innerText))?.click());
    await p.click('#reportTab [data-ftab="grid"]');
    check('back on the Report tab, the report is as it was, not run again', await until(() => document.querySelectorAll('#reportTab .cgrid tbody tr').length === 5) && (await fake(() => window.__tlmFake.checkRuns.filter((r) => r.folder).length)) === 5);
    check('kept in settings with the set', !!(await settingsLS()).folderReports?.['Flight acceptance']?.rows);
    check('and the session keeps the tab', await until(() => window.__tlmSettings.get('sessions')?.['/data/flights']?.reports?.[0]?.set === 'Flight acceptance'), await p.evaluate(() => window.__tlmSettings.get('sessions')?.['/data/flights']));

    // a trend point opens its file
    await p.click('#reportTab [data-ftab="report"]');
    await p.evaluate(() => document.querySelector('#reportTab [data-rrow="RPM below 7500"] [data-tpt$="flight_0415.tlm"]').dispatchEvent(new MouseEvent('click', { bubbles: true })));
    check('a trend point opens that file', await until(() => /flight_0415/.test(document.querySelector('#wsTabs [aria-selected="true"]')?.innerText ?? ''), null, 6000));

    // a cell opens its file at the violation
    await p.evaluate(() => [...document.querySelectorAll('#wsTabs [role="tab"], #wsTabs .wstab')].find((t) => /Report/.test(t.innerText))?.click());
    await p.click('#reportTab [data-ftab="grid"]');
    await p.evaluate(() => [...document.querySelectorAll('#reportTab .cgrid tbody tr')].find((tr) => /flight_0418/.test(tr.textContent)).querySelector('[data-go]').click());
    check('clicking a cell opens that file', await until(() => /flight_0418/.test(document.querySelector('#wsTabs [aria-selected="true"]')?.innerText ?? ''), null, 6000));
    check('the Report tab stays in the strip', [...(await p.$$eval('#wsTabs .nm', (n) => n.map((x) => x.textContent)))].includes('Report · Flight acceptance'));
    check('at its first violation in that group', await until(() => window.__tlmApp.view().t0 !== window.__tlmApp.view().first, null, 5000), await p.evaluate(() => window.__tlmApp.view()));
  },
  {
    files,
    launch: `${DIR}/flight_0412.tlm`,
    setupArg: [SET, DIR, day(12), day(15), day(18), day(16)],
    setup: (fake, [SET, DIR, d12, d15, d18, d16]) => {
      fake.checkSets = [SET];
      fake.checkOutcomes = {};
      fake.checkFileTimes = { [`${DIR}/archive/flight_0301.tlm`]: d12, [`${DIR}/flight_0412.tlm`]: d12, [`${DIR}/flight_0415.tlm`]: d15, [`${DIR}/flight_0418.tlm`]: d18, [`${DIR}/gps_logger.csv`]: d16 };
      const cant = (c) => ({ verdict: 'cant', message: `There's no channel called ${c}` });
      fake.checkFileOutcomes = {
        [`${DIR}/flight_0418.tlm`]: { 'RPM below 7500': { verdict: 'fail', runs: [[4, 5]], value: 7600 } },
        [`${DIR}/flight_0412.tlm`]: { 'RPM below 7500': { value: 6800 } },
        [`${DIR}/flight_0415.tlm`]: { 'RPM below 7500': { value: 7000 } },
        [`${DIR}/archive/flight_0301.tlm`]: { 'RPM below 7500': { value: 6500 } },
        [`${DIR}/gps_logger.csv`]: { 'RPM below 7500': cant('motor_rpm'), 'Battery above 21 V': cant('battery_v') },
      };
    },
    settings: { checkAttach: { files: {}, layouts: {} } },
  },
);
