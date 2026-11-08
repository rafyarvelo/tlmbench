// The Layouts sidebar (rail), the .tlmbench entry at the top of Explorer, and the tab menu's header: Layouts lists your
// layouts and the ones shared in the folder, opens or shows them, creates an empty one and saves the current tab as one
// (asking for a name in the panel), deletes with Undo; Explorer's `.tlmbench` row counts the shared views and folds; a
// tab's menu names the tab, says whether it is saved and which file it shows.
import { run } from './lib.js';

const DIR = '/data/work';
const A = `${DIR}/flight.tlm`;
const files = { [A]: {} };
const shared = {
  [DIR]: {
    'review.tlmview.json': { schema: 1, name: 'Flight review', by: 'a.chen', panes: [{ channels: ['accel_x'] }], files: [{ path: 'flight.tlm', slot: 1 }] },
    'power.tlmview.json': { schema: 1, name: 'Power', by: 'r.patel', panes: [{ channels: ['roll'] }], files: [{ path: 'flight.tlm', slot: 1 }] },
  },
};

run(
  'layouts',
  async ({ p, check, fake, until, text, visible, settingsLS }) => {
    await fake(([d, v]) => { window.__tlmFake.pickFolder = d; Object.assign(window.__tlmFake.views, v); }, [DIR, shared]);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length >= 1);

    // Explorer's .tlmbench entry
    check('a .tlmbench row heads the tree with the shared views counted', await until(() => /\.tlmbench.*2 shared views/.test(document.querySelector('#exShared')?.textContent ?? '')), await text('#exShared'));
    check('folded to start with', (await p.getAttribute('#exShared', 'aria-expanded')) === 'false' && !(await visible('[data-shared]')));
    await p.click('#exShared');
    check('a click lists them', (await p.$$('[data-shared]')).length === 2);
    await p.click('[data-shared="review.tlmview.json"]');
    check('and one opens its layout in a tab', await until(() => [...document.querySelectorAll('#wsTabs .nm')].some((n) => n.textContent === 'Flight review')));

    // the rail's Layouts
    await p.click('[data-rail="layouts"]');
    check('the rail opens the Layouts panel', await until(() => !!document.querySelector('#layoutsPanel')?.offsetParent));
    check('shared views are listed, the open one marked', (await p.$$('[data-lshared]')).length === 2 && (await p.$eval('[data-lshared="review.tlmview.json"]', (e) => e.classList.contains('cur'))));
    check('none of yours yet', /None yet/.test(await text('#layoutsPanel')));

    await p.click('#lyNew');
    await p.fill('#lyName', '');
    await p.click('#lyForm [type=submit]');
    check('a name is needed', await until(() => /name/i.test(document.querySelector('.lyerr')?.textContent ?? '')), await text('#lyForm'));
    await p.fill('#lyName', 'Scratch');
    await p.press('#lyName', 'Enter');
    check('a new empty layout opens in its own tab', await until(() => [...document.querySelectorAll('#wsTabs .nm')].some((n) => n.textContent === 'Scratch')));
    check('and is listed as yours', await until(() => !!document.querySelector('[data-lopen="Scratch"]')));

    await p.click('#lySave');
    check('Save current as… starts from "copy"', (await p.inputValue('#lyName')) === 'Scratch copy', await p.inputValue('#lyName'));
    await p.fill('#lyName', 'Scratch 2');
    await p.press('#lyName', 'Enter');
    check('Save current as… adds a layout of its own', await until(() => !!document.querySelector('[data-lopen="Scratch 2"]')) && (await settingsLS()).workspaces.length === 2);

    // a click shows a layout that is open
    await p.click('[data-lshared="review.tlmview.json"]');
    check('a click shows the layout that is open', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Flight review'));
    check('marked as the current one', await p.$eval('[data-lshared="review.tlmview.json"]', (e) => e.classList.contains('cur')));

    // delete
    await p.hover('[data-lopen="Scratch 2"]');
    await p.click('[data-lydel="Scratch 2"]');
    check('Delete removes it from the list', (await until(() => !document.querySelector('[data-lopen="Scratch 2"]'))) && !(await settingsLS()).workspaces.some((w) => w.name === 'Scratch 2'));

    // the tab menu's header
    await p.click('#wsTabs .wstab[aria-selected=true] .tm');
    check("the tab menu is headed by the tab's name", (await text('#tmName')) === 'Flight review', await text('#tmName'));
    check('a shared view says who shared it and where', /Shared in this folder by a\.chen · \.tlmbench\/views\/review\.tlmview\.json/.test(await text('#wsStatus')), await text('#wsStatus'));
    check('"This tab shows" names the file', /This tab shows.*flight\.tlm/.test(await text('#tabFiles')), await text('#tabFiles'));
    await p.click('#tmClose');
    check('× closes the tab', await until(() => ![...document.querySelectorAll('#wsTabs .nm')].some((n) => n.textContent === 'Flight review')));
  },
  { files, launch: A },
);
