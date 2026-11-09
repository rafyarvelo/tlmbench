// Run on folder…: one set on every recording in Explorer's folder, files × groups in a grid, a Report tab with each
// check's pass rate and a trend in date order, results as CSV, and a click on a cell opening that file at its first
// violation in the group.
import { run } from './lib.js';

const DIR = '/data/flights';
const CH = ['motor_rpm', 'battery_v', 'flight_mode'];
const files = {
  [`${DIR}/flight_0412.tlm`]: { channels: CH },
  [`${DIR}/flight_0418.tlm`]: { channels: CH },
  [`${DIR}/flight_0415.tlm`]: { channels: CH },
  [`${DIR}/gps_logger.csv`]: { channels: ['lat', 'lon'] },
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
  async ({ p, check, until, fake, text }) => {
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length === 4);
    await p.click('[data-rail="checks"]');
    await p.click('#checksBody [data-cfolder]');
    check('the dialog offers the sets', await until(() => document.querySelectorAll('#folderDlg [data-ffold-set] option').length === 1));
    await p.click('#folderDlg [data-frun]');
    const done = await until(() => document.querySelectorAll('#folderDlg .cgrid tbody tr').length === 4 && document.querySelector('#folderDlg [data-frun]').textContent === 'Run', null, 10000);
    check('the folder run checks every recording', done);
    const grid = await p.evaluate(() => [...document.querySelectorAll('#folderDlg .cgrid tbody tr')].map((tr) => [...tr.children].map((td) => td.innerText.replace(/\s+/g, ' ').trim()).join(' | ')));
    check('files × groups', grid.length === 4 && grid.some((r) => /flight_0418/.test(r)) && grid.some((r) => /gps_logger/.test(r)), grid);
    check('the core ran the set once on each, by path', (await fake(() => window.__tlmFake.checkRuns.filter((r) => r.folder).length)) === 4);
    check('a failing file shows Fail in its group', grid.some((r) => /flight_0418.*Fail/.test(r)), grid);
    check("a file without the channels says it can't run", grid.some((r) => /gps_logger.*can't run/i.test(r)), grid.find((r) => /gps_logger/.test(r)));

    // Report
    await p.click('#folderDlg [data-ftab="report"]');
    const rep = await p.evaluate(() => [...document.querySelectorAll('#folderDlg .creport tbody tr')].map((tr) => ({ t: tr.innerText.replace(/\s+/g, ' ').trim(), pts: tr.querySelectorAll('.tspark circle').length })));
    check('the Report tab lists each check with its pass rate', rep.length === 2 && rep.every((r) => /\d+ of \d+ files|not run/.test(r.t)), rep);
    check('RPM passed in 2 of 3 files (the one without channels did not run)', /RPM below 7500 .* 2 of 3 files/.test(rep[0].t), rep[0].t);
    check('a trend with a point per file, oldest first', rep[0].pts === 4 && /2026-10-12 → 2026-10-18/.test(await text('#folderDlg .rbar')), [rep.map((r) => r.pts), await text('#folderDlg .rbar')]);
    await p.selectOption('#folderDlg [data-rgroup]', 'Power');
    check('filter by group', await p.evaluate(() => [...document.querySelectorAll('#folderDlg .creport tbody tr td:nth-child(2)')].every((td) => td.textContent === 'Power')));

    // CSV
    await fake(() => (window.__tlmFake.savePick = '/tmp/results.csv'));
    await p.click('#folderDlg [data-fcsv]');
    const w = await until(() => window.__tlmFake.written, null, 3000);
    check('results as CSV go where the person chose', !!w && w.path === '/tmp/results.csv' && /^file,group,check,kind,verdict/.test(w.text) && /flight_0418.*RPM below 7500,check,fail/.test(w.text), w && w.text.slice(0, 200));

    // a cell opens its file at the violation
    await p.click('#folderDlg [data-ftab="grid"]');
    await p.evaluate(() => [...document.querySelectorAll('#folderDlg .cgrid tbody tr')].find((tr) => /flight_0418/.test(tr.textContent)).querySelector('[data-go]').click());
    check('clicking a cell opens that file', await until(() => /flight_0418/.test(document.querySelector('#wsTabs [aria-selected="true"]')?.innerText ?? ''), null, 6000));
    check('and closes the dialog', !(await p.$('#folderDlg')));
    check('at its first violation in that group', await until(() => window.__tlmApp.view().t0 !== window.__tlmApp.view().first, null, 5000), await p.evaluate(() => window.__tlmApp.view()));
  },
  {
    files,
    launch: `${DIR}/flight_0412.tlm`,
    setupArg: [SET, DIR, day(12), day(15), day(18), day(16)],
    setup: (fake, [SET, DIR, d12, d15, d18, d16]) => {
      fake.checkSets = [SET];
      fake.checkOutcomes = {};
      fake.checkFileTimes = { [`${DIR}/flight_0412.tlm`]: d12, [`${DIR}/flight_0415.tlm`]: d15, [`${DIR}/flight_0418.tlm`]: d18, [`${DIR}/gps_logger.csv`]: d16 };
      const cant = (c) => ({ verdict: 'cant', message: `There's no channel called ${c}` });
      fake.checkFileOutcomes = {
        [`${DIR}/flight_0418.tlm`]: { 'RPM below 7500': { verdict: 'fail', runs: [[4, 5]] } },
        [`${DIR}/gps_logger.csv`]: { 'RPM below 7500': cant('motor_rpm'), 'Battery above 21 V': cant('battery_v') },
      };
    },
    settings: { checkAttach: { files: {}, layouts: {} } },
  },
);
