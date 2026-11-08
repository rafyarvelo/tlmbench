// A session per folder (owner, Nov 7): its tabs, open files, and each file's time range and cursors. Opening another
// folder puts the session away and brings that folder's; the start page lists them under Recent sessions (the
// current one marked), a click opens one; the folder's session comes back at launch.
import { run } from './lib.js';

const A = '/data/flights';
const B = '/data/bench';
const files = {
  [`${A}/one.tlm`]: {},
  [`${A}/two.csv`]: { channels: ['volts', 'amps'], units: ['V', 'A'] },
  [`${B}/esc.csv`]: { channels: ['rpm', 'temp'], units: ['rpm', '°C'] },
};

run(
  'sessions',
  async ({ p, check, text, fake, until, plotAt, wait, settingsLS }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    const tabs = () => p.$$eval('#wsTabs .wstab .nm', (t) => t.map((x) => x.textContent));
    const folder = async (d) => {
      await fake((d) => (window.__tlmFake.pickFolder = d), d);
      await p.click('#exFolder');
    };
    const openRow = async (n) => {
      await p.dblclick(`#explorer .row.file[data-path$="${n}"]`);
      await until((n) => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === n, n);
    };

    // folder A: two files, the first zoomed in with cursors
    await folder(A);
    await until(() => document.querySelectorAll('#explorer .row.file').length === 2);
    await openRow('one.tlm');
    const c = await plotAt(0);
    await p.mouse.move(c.x, c.y);
    await p.mouse.wheel(0, -500);
    await wait(300);
    await p.keyboard.press('c');
    const a = await plotAt(0, 0.3);
    const b = await plotAt(0, 0.7);
    await p.mouse.click(a.x, a.y);
    await p.mouse.click(b.x, b.y);
    const view = await app('view');
    const cursors = await app('cursors');
    await openRow('two.csv');
    await wait(600);

    // folder B: A's tabs go away (kept in its session), B starts its own
    await fake((d) => (window.__tlmFake.pickFolder = d), B);
    await p.click('#exFolder');
    await p.click('.exmenu [data-ex="open-folder"]');
    check('opening another folder lists it', await until(() => document.querySelector('#exFolder')?.textContent.includes('bench')));
    check('...and puts the first folder\'s tabs away', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 0), await tabs());
    const saved = (await settingsLS()).sessions[A];
    check('the first folder\'s session is saved: its files, the front one, views and cursors', saved && saved.files.join() === `${A}/one.tlm,${A}/two.csv` && saved.active === `${A}/two.csv` && saved.views[`${A}/one.tlm`].t0 === view.t0 && saved.views[`${A}/one.tlm`].A === cursors.A, saved);
    await openRow('esc.csv');

    // the start page lists both sessions, the current one marked; a click on A brings it back
    await p.click('#b-start');
    const rows = await p.$$eval('.start.overlay [data-session]', (r) => r.map((x) => x.dataset.session + ' ' + x.innerText.replace(/\s+/g, ' ')));
    check('Recent sessions lists each folder, the current one first and marked', rows.length === 2 && rows[0].startsWith(B) && /current/.test(rows[0]) && rows[1].startsWith(A) && /2 files · one\.tlm, two\.csv/.test(rows[1]), rows);
    await p.click(`.start.overlay [data-session="${A}"]`);
    check('a click opens that folder', await until(() => document.querySelector('#exFolder')?.textContent.includes('flights')));
    check('...with its tabs, the same one in front', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2 && document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'two.csv'), await tabs());
    await p.click('#wsTabs .wstab:has-text("one.tlm")');
    await wait(300);
    check('...each file at its time range, with its cursors', JSON.stringify([(await app('view')).t0, (await app('view')).t1]) === JSON.stringify([view.t0, view.t1]) && (await app('cursors')).A === cursors.A && (await app('cursors')).B === cursors.B, [await app('view'), await app('cursors')]);
    check('B\'s session is kept for later', (await settingsLS()).sessions[B]?.files.join() === `${B}/esc.csv`);

    // the folder's session comes back at launch
    await wait(600);
    await p.reload();
    await wait(1200);
    check('at launch the folder\'s session opens again', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2), await tabs());
    await p.click('#wsTabs .wstab:has-text("one.tlm")');
    await wait(300);
    check('...at its time range', (await app('view')).t0 === view.t0, await app('view'));
  },
  { files, launch: null, settings: { showStart: false } },
);
