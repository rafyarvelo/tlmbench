// Events, checks and rules on every file of a tab (S5.6-12, owner Nov 15): each file's events show on the tab's
// timeline through its time map, labelled with the file, with file chips to filter and a group per file; a new event
// goes to file 1 unless the selected pane shows only another file's channels, and the edit form moves it to another
// file (kept by that file in its own clock); every file's sets run on that file, the Checks panel shows one file's
// results at a time with its violations on the tab's timeline, and the session keeps a hidden file chip.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { samples: 20000, rate: 100 } };
const T0 = 1767225600000000000n;
const S = 1000000000n;
const SET = { schema: 1, name: 'Limits', runs_on_any_file: true, info_groups: [], check: [{ name: 'Roll small', kind: 'check', group: 'Attitude', expect: 'roll < 1', severity: 'fail', grace: 0, settle: 0, enabled: true }] };

run(
  'slotevents',
  async ({ p, check, fake, until }) => {
    const evs = () => p.evaluate(() => [...document.querySelectorAll('#eventsPanel .ev')].map((x) => x.querySelector('.evname').textContent + '@' + (x.querySelector('[data-evfilelbl]')?.textContent ?? '')));
    const list = () => p.evaluate(() => window.__tlmApp.eventList());
    await fake(
      ([a, b, t0, set]) => {
        const f = window.__tlmFake;
        f.pickFolder = '/data/runs';
        f.events[a] = [{ id: 'a1', name: 'Takeoff', start: String(BigInt(t0) + 2000000000n), duration: '0', category: '', source: 'user', channels: [] }];
        f.events[b] = [{ id: 'b1', name: 'Fix', start: String(BigInt(t0) + 3000000000n), duration: '1000000000', category: '', source: 'user', channels: [] }];
        // file 2 runs 10 s later on the tab's timeline
        f.alignResults.clock = { knots: [['0', '10000000000']] };
        f.checkSets = [set];
        f.checkOutcomes = { 'Roll small': { verdict: 'fail', runs: [[4, 5]] } };
      },
      [A, B, String(T0), SET],
    );
    await p.evaluate(() => window.__tlmSettings.set('panelsClosed', []));
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    check('one file: its own events, no file label', (await until(() => [...document.querySelectorAll('#eventsPanel .ev .evname')].some((x) => x.textContent === 'Takeoff'))) && !(await p.$('#eventsPanel [data-evfilelbl]')), await evs());
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);

    const mine = async () => (await evs()).filter((e) => !e.startsWith('Roll small'));
    check("both files' events show, labelled with their file", await until(() => [...document.querySelectorAll('#eventsPanel .ev .evname')].some((x) => x.textContent === 'Fix')), await evs());
    check('in time order on the tab', (await mine()).join() === 'Takeoff@flight,Fix@gps', await evs());
    const fix = (await list()).find((e) => e.name === 'Fix');
    check("file 2's event is read through its map (3 s in its clock, 13 s on the tab)", BigInt(fix.start) === T0 + 13n * S && fix.duration === '1000000000', fix);
    check('and knows its file', fix.slot === 2 && fix.file === 'gps', fix);

    // file chips
    await p.click('[data-rail="events"]');
    check('a chip per file', (await p.$$('#eventsPanel [data-evfile]')).length === 2);
    await p.click('#eventsPanel [data-evfile="2"]');
    check("a file chip hides that file's events", await until(() => ![...document.querySelectorAll('#eventsPanel [data-evfilelbl]')].some((x) => x.textContent === 'gps')), await evs());
    await p.click('#eventsPanel [data-evfile="2"]');
    await until(() => [...document.querySelectorAll('#eventsPanel [data-evfilelbl]')].some((x) => x.textContent === 'gps'));

    // group by file
    await p.selectOption('#evGroup', 'file');
    const heads = await p.$$eval('#eventsPanel .evgroup', (g) => g.map((x) => x.dataset.evg));
    check('group by file has a group per file', heads.join() === 'flight,gps', heads);
    await p.selectOption('#evGroup', '');

    // a new marker from a pane of file 2's channels goes to file 2, in its own clock
    const ch2 = await p.evaluate(() => window.__tlmApp.panes && [...document.querySelectorAll('#explorer .row.chan')].map((r) => +r.dataset.ch).filter((c) => c >= 6)[0]);
    await p.evaluate((ch) => window.__tlmApp.setPanes([{ channels: [ch] }]), ch2);
    await p.evaluate((t) => window.__tlmApp.setCursor('A', t), String(T0 + 20n * S));
    await fake(() => (window.__tlmFake.eventSaves.length = 0));
    await p.click('#evMarker');
    check("a marker from file 2's pane is kept by file 2", await until(() => window.__tlmFake.events['/data/runs/gps.tlm'].length === 2), await fake(() => window.__tlmFake.events));
    const saved = await fake(() => window.__tlmFake.eventSaves.at(-1));
    check('in its own clock (20 s on the tab is 10 s in it)', BigInt(saved.start) === T0 + 10n * S, saved);
    check('the saved event has no tab-only fields', !('slot' in saved) && !('file' in saved), saved);
    check('the new marker is open to edit, with a file picker', await until(() => !!document.querySelector('#eventsPanel [data-evf="file"]')));
    await p.selectOption('#eventsPanel [data-evf="file"]', '1');
    check('the file picker moves it to file 1', await until(() => window.__tlmFake.events['/data/runs/gps.tlm'].length === 1 && window.__tlmFake.events['/data/runs/flight.tlm'].length === 2), await fake(() => window.__tlmFake.events));
    const moved = await fake(() => window.__tlmFake.events['/data/runs/flight.tlm'].find((e) => e.name === 'Marker'));
    check('at the same place on the tab: file 1 keeps it at 20 s', BigInt(moved.start) === T0 + 20n * S, moved);
    const tab = (await list()).find((e) => e.name === 'Marker');
    check('and the list shows it as file 1\'s', tab?.slot === 1 && BigInt(tab.start) === T0 + 20n * S, tab);

    // checks ran on both files
    const runs = await fake(() => window.__tlmFake.checkRuns.map((r) => r.path));
    check("every file's sets ran on that file", runs.includes(A) && runs.includes(B), runs);
    await p.click('[data-rail="checks"]');
    check('the Checks panel picks a file', await until(() => !!document.querySelector('[data-ckfile]')));
    const opts = await p.$$eval('[data-ckfile] option', (o) => o.map((x) => x.textContent));
    check('one option per file', opts.join() === '1 · flight,2 · gps', opts);
    const fails = (await list()).filter((e) => e.name === 'Roll small');
    check("both files' failing checks are events, each labelled", fails.length === 2 && fails.map((e) => e.file).sort().join() === 'flight,gps', fails);
    const r2 = (await p.evaluate(() => window.__tlmApp.checkRuns()))[B]?.Limits?.outcomes?.[0]?.runs?.[0];
    check("file 2's violation is on the tab's timeline (4 s in its clock, 14 s on the tab)", r2 && BigInt(r2.start) === T0 + 14n * S, r2);
    await p.selectOption('[data-ckfile]', B);
    check('picking file 2 shows its results', await until(() => !!document.querySelector('[data-cset="Limits"]')));

    // the session keeps a hidden file
    await p.click('[data-rail="events"]');
    await p.click('#eventsPanel [data-evfile="2"]');
    check('the session keeps a hidden file', await until(() => window.__tlmSettings.get('sessions')?.['/data/runs']?.events?.['/data/runs/flight.tlm']?.files?.join() === 'gps'), await p.evaluate(() => window.__tlmSettings.get('sessions')?.['/data/runs']?.events));
  },
  { files, launch: null },
);
