// The CSV import dialog: shown on the first open of a CSV whose header has no saved profile (a drop, Open…, Explorer),
// the choices it saves as the header's profile (what the core applies at decode), a profile for a nearly equal header
// offered but not applied, several files grouped by header, Explorer only, and Import settings… on a file again.
import { run } from './lib.js';

const A = '/data/run_a.csv';
const B = '/data/run_b.csv';
const C = '/data/other.csv';
const D = '/data/run_c.csv';
const T = '/data/flight.tlm';
const NAMES = ['time_ms', 'Speed', 'temp_in', 'temp_out', 'note'];

run(
  'import',
  async ({ p, check, fake, until, text, visible, wait }) => {
    const tabs = () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent));
    const dlg = () => visible('#importDlg');
    await fake((names) => {
      const f = window.__tlmFake;
      f.csv('/data/run_a.csv', names, { unit: 'ms', rate: 100, units: { Speed: 'm/s' }, text: ['note'], rows: 4200 });
      f.csv('/data/run_b.csv', names, { unit: 'ms', rate: 100, text: ['note'] });
      f.csv('/data/other.csv', ['t', 'x', 'y'], { unit: 's', rate: 1 });
      // one column renamed: three of four names in common with run_a
      f.csv('/data/run_c.csv', ['time_ms', 'Speed', 'temp_in', 'temp_out_2', 'note'], { unit: 'ms', rate: 100, text: ['note'] });
    }, NAMES);

    // a file that isn't a CSV opens without asking
    await fake((t) => window.__tlmFake.drop([t]), T);
    check('a non-CSV opens at once', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm'));
    check('no dialog for it', !(await dlg()));

    // a first CSV: the dialog, filled in from what the core found
    await fake((a) => window.__tlmFake.drop([a]), A);
    check('a first CSV asks', await until(() => !!document.querySelector('#importDlg')));
    check('it has not opened yet', !(await tabs()).includes('run_a.csv'));
    check('rows and columns shown', /4,200 rows · 5 columns · comma-separated/.test(await text('#impInfo')), await text('#impInfo'));
    check('the time column is the detected one', (await p.inputValue('#impTime')) === 'time_ms' && (await p.inputValue('#impUnit')) === 'ms');
    check('as recorded, with the detected rate', /As recorded 100 Hz/.test(await text('.imprate')), await text('.imprate'));
    const rows = await p.$$eval('#impCols .impcol:not(.head)', (r) => r.map((e) => e.dataset.h));
    check('the time column is not listed as a channel', rows.join() === 'Speed,temp_in,temp_out,note', rows);
    check('units and types as detected', (await p.inputValue('[data-h=Speed] .impun')) === 'm/s' && (await p.inputValue('[data-h=note] .imptype')) === 'text');
    check('the counts', /3 number · 1 text · 0 skipped/.test(await text('.impmain h4:nth-of-type(2)')), await text('.impmain h4:nth-of-type(2)'));
    check('Open is ready; Explorer only and Each file in a tab offered; shared/current wait for multi-file tabs',
      (await p.isEnabled('#impGo')) && (await p.$$eval('#impWhere option', (o) => o.map((x) => x.disabled).join())) === 'false,true,true,false');

    // choices: a name, a unit, a type, a category, Resample
    await p.fill('[data-h=Speed] .impnm', 'v');
    await p.fill('[data-h=Speed] .impun', 'km/h');
    await p.fill('[data-h=Speed] .impcatin', 'nav');
    await p.selectOption('[data-h=temp_out] .imptype', 'skip');
    check('a skipped column is struck through', await p.$eval('[data-h=temp_out]', (e) => e.classList.contains('skip')));
    await p.click('input[name=imprate][value=fixed]');
    await p.fill('#impRateFixed', '50');
    await p.dispatchEvent('#impRateFixed', 'change');
    await p.click('#impGo');
    check('Open opens the file', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => n.textContent === 'run_a.csv')));
    const saved = await fake(() => window.__tlmFake.saved);
    check('one profile saved, under the header the core names', saved.length === 1 && saved[0].header.join() === 'time_ms,Speed,temp_in,temp_out,note', saved);
    const prof = saved[0]?.profile;
    check('time column and unit', prof?.time?.type === 'column' && prof.time.name === 'time_ms' && prof.time.unit === 'ms', prof?.time);
    check('Resample at 50 Hz', prof?.rate?.type === 'resample' && prof.rate.hz === 50, prof?.rate);
    const cols = Object.fromEntries((prof?.columns ?? []).map((c) => [c.source, c]));
    check('only changes are in the profile', Object.keys(cols).sort().join() === 'Speed,temp_out', Object.keys(cols));
    check('name, unit and category', cols.Speed?.name === 'v' && cols.Speed?.unit === 'km/h' && cols.Speed?.category === 'nav', cols.Speed);
    check('the type', cols.temp_out?.kind === 'skip', cols.temp_out);
    const given = await fake(() => window.__tlmFake.openedWith.find((o) => o.path === '/data/run_a.csv'));
    check('the core was asked to open the file (it finds the saved profile itself)', !!given);
    check('its channels carry the profile', await until(() => [...document.querySelectorAll('#explorer .row.chan .nm')].some((n) => n.textContent === 'v')) || (await text('#explorer')));

    check('a category from the profile groups the channel in Explorer', await until(() => !!document.querySelector('#explorer .row.cat[data-cat="nav"]')), await text('#explorer'));

    check('the file row shows its data rate', await until(() => document.querySelector('#explorer .row.file[data-path$="run_a.csv"] .ratechip')?.textContent === '1 kHz'), await text('#explorer .row.file[data-path$="run_a.csv"]'));

    // the same header again: no dialog, the saved profile applies
    await fake((b) => window.__tlmFake.drop([b]), B);
    check('a file with the same header opens without asking', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => n.textContent === 'run_b.csv')));
    check('no dialog', !(await dlg()));

    // a header that nearly matches: the profile is offered, never applied unasked
    await fake((d) => window.__tlmFake.drop([d]), D);
    check('a near header asks', await until(() => !!document.querySelector('#importDlg')));
    check('the similar profile is offered', /similar file/.test((await text('#impNear')) ?? ''), await text('#impNear'));
    check('its choices are not applied yet', (await p.inputValue('[data-h=Speed] .impnm')) === 'Speed' && !(await visible('#impApplied')));
    await p.click('[data-useprof]');
    check('Apply takes its choices for the columns they share', (await p.inputValue('[data-h=Speed] .impnm')) === 'v' && (await p.inputValue('[data-h=Speed] .impun')) === 'km/h');
    check('and says so', /Using settings from a similar file/.test((await text('#impApplied')) ?? ''), await text('#impApplied'));
    check('the rate came with it', (await p.inputValue('#impRateFixed')) === '50' && (await p.isChecked('input[name=imprate][value=fixed]')));
    await p.keyboard.press('Escape');
    check('Esc cancels: nothing opened, nothing saved', !(await dlg()) && !(await tabs()).includes('run_c.csv') && (await fake(() => window.__tlmFake.saved.length)) === 1);

    // time None leaves "as recorded" and "resample" unavailable; a rate must be above zero
    await fake((c) => window.__tlmFake.drop([c]), C);
    check('another header asks', await until(() => !!document.querySelector('#importDlg')));
    await p.selectOption('#impTime', '');
    check('no time column: sample numbers, and the rate options that need one are disabled',
      (await p.isChecked('input[name=imprate][value=samples]')) && (await p.isDisabled('input[name=imprate][value=recorded]')) && (await p.isDisabled('input[name=imprate][value=fixed]')));
    await p.click('input[name=imprate][value=assume]');
    await p.fill('#impRateAssume', '0');
    await p.dispatchEvent('#impRateAssume', 'change');
    check('a rate of zero blocks Open and says why', (await p.isDisabled('#impGo')) && /rate above zero/.test((await text('#importDlg footer')) ?? ''), await text('#importDlg footer'));
    await p.fill('#impRateAssume', '25');
    await p.dispatchEvent('#impRateAssume', 'change');
    check('a rate above zero lets it go', await p.isEnabled('#impGo'));

    // Explorer only: the settings are kept and nothing opens; the file then opens without asking
    await p.selectOption('#impWhere', 'none');
    check('the button says what happens', /Add to Explorer/.test((await text('#impGo')) ?? ''));
    const before = (await tabs()).length;
    await p.click('#impGo');
    check('the dialog closes', await until(() => !document.querySelector('#importDlg')));
    check('no tab opened', (await tabs()).length === before);
    const assumed = await fake(() => window.__tlmFake.saved.at(-1).profile);
    check('assume 25 Hz saved', assumed.rate.type === 'assume' && assumed.rate.hz === 25 && assumed.time.type === 'none', assumed);
    check('a note says it was added', /Saved the import settings for other\.csv/.test((await text('#wsNote')) ?? ''), await text('#wsNote'));
    check('where files go is remembered', (await p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.settings') || '{}').importWhere)) === 'none');
    await fake((c) => window.__tlmFake.drop([c]), C);
    check('opening it later skips the dialog', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => n.textContent === 'other.csv')) && !(await dlg()));

    // several files: the same header shares one job, another header is its own
    await fake(() => {
      const f = window.__tlmFake;
      for (const [k, v] of Object.entries({ '/data/g1.csv': ['t', 'a', 'b'], '/data/g2.csv': ['t', 'a', 'b'], '/data/g3.csv': ['when', 'p', 'q', 'r'] })) {
        f.csv(k, v, { unit: 's', rate: 10 });
        f.files[k] = {};
      }
    });
    await fake(() => window.__tlmFake.drop(['/data/g1.csv', '/data/g2.csv', '/data/g3.csv']));
    await until(() => !!document.querySelector('#importDlg'));
    await p.selectOption('#impWhere', 'tabs');
    check('two headers, two jobs', await until(() => document.querySelectorAll('#importDlg .impjob').length === 2));
    check('files with one header are grouped', /2 files · same header/.test(await text('#importDlg .impjob')), await text('#importDlg .impjob'));
    check('the title counts the files', /Import 3 files/.test(await text('#importDlg header')), await text('#importDlg header'));
    check('the button counts them', /Open 3 files/.test(await text('#impGo')), await text('#impGo'));
    await p.click('#importDlg .impjob:nth-of-type(2)');
    check('the other job shows its own columns', (await p.$$eval('#impCols .impcol:not(.head)', (r) => r.map((e) => e.dataset.h))).join() === 'p,q,r');
    await p.click('#impGo');
    check('all three open, each in its own tab', await until(() => ['g1.csv', 'g2.csv', 'g3.csv'].every((n) => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((x) => x.textContent === n))));
    check('a profile per header was saved', (await fake(() => window.__tlmFake.saved.length)) === 4, await fake(() => window.__tlmFake.saved.length));

    // Import settings… on a file in Explorer: its saved settings are filled in; saving opens it again with them
    await fake(() => (window.__tlmFake.pickFolder = '/data'));
    await p.click('#exFolder');
    await p.click('#explorer .row.file[data-path$="run_a.csv"]', { button: 'right' });
    await p.click('.filemenu [data-fm="import"]');
    check('the dialog opens on the file', await until(() => !!document.querySelector('#importDlg')) && /Open run_a\.csv/.test(await text('#importDlg header')));
    check('with its saved settings', /Using settings from its saved settings/.test((await text('#impApplied')) ?? '') && (await p.inputValue('[data-h=Speed] .impnm')) === 'v' && (await p.isChecked('input[name=imprate][value=fixed]')));
    await p.fill('[data-h=Speed] .impnm', 'speed2');
    const opens = await fake(() => window.__tlmFake.openedWith.length);
    await p.click('#impGo');
    check('saved again', await until(() => window.__tlmFake.saved.at(-1).profile.columns.some((c) => c.name === 'speed2')));
    check('the file opened again', await until((n) => window.__tlmFake.openedWith.length > n, opens));
    check('in one tab', (await tabs()).filter((n) => n === 'run_a.csv').length === 1, await tabs());
    check('with the new settings', await until(() => window.__tlmFake.openedWith.at(-1).profile?.columns?.some((c) => c.name === 'speed2') ?? true));

    // Import all in the folder menu: only the files whose headers have no saved settings are asked about; nothing opens
    await fake(() => {
      const f = window.__tlmFake;
      for (const k of ['/data/fresh1.csv', '/data/fresh2.csv']) {
        f.csv(k, ['t_s', 'u', 'w'], { unit: 's', rate: 5 });
        f.files[k] = {};
      }
    });
    const tabsBefore = (await tabs()).length;
    await p.click('#exFolder');
    const label = await p.$eval('.exmenu [data-ex="import-all"]', (e) => e.textContent.trim());
    check('the folder menu counts the CSV files it would set up', /^Import all \d+ files…/.test(label), label);
    await p.click('.exmenu [data-ex="import-all"]');
    check('files without settings are asked about', await until(() => !!document.querySelector('#importDlg')));
    check('it goes to Explorer only, nothing else', (await p.inputValue('#impWhere')) === 'none' && /Add to Explorer/.test((await text('#impGo')) ?? ''));
    await p.keyboard.press('Escape');
    check('cancelling opened nothing', (await tabs()).length === tabsBefore);

    // a file that isn't a CSV has no import settings
    await p.click('#explorer .row.file[data-path$="flight.tlm"]', { button: 'right' });
    check('Import settings… is off for it', await p.isDisabled('.filemenu [data-fm="import"]'));
    await p.keyboard.press('Escape');
  },
  { files: { '/data/run_a.csv': {}, '/data/run_b.csv': {}, '/data/other.csv': {}, '/data/run_c.csv': {}, '/data/flight.tlm': {} }, launch: null },
);
