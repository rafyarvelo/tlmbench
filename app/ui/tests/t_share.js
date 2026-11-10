// Shared configuration: Explorer's folder menu opens Share…, which lists every object by kind against the folder's
// .tlmbench/ (new ones are shared by default, the folder's own are taken, what differs waits for a choice); Apply
// writes and takes; a read-only folder offers Export… instead; the folder's recipes are marked; a pair of files
// opens with the alignment it was left with (kept in the cache, found again by the pair).
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { channels: ['lat', 'lon'], units: ['deg', 'deg'], samples: 20000, rate: 100 } };
const ROWS = [
  { kind: 'views', key: 'views/flight-review.tlmview.json', name: 'Flight review', state: 'new', detail: null },
  { kind: 'recipes', key: 'recipes/power.toml', name: 'Power', state: 'changed', detail: null },
  { kind: 'recipes', key: 'recipes/smooth.toml', name: 'Smooth', state: 'same', detail: null },
  { kind: 'events', key: 'events/flight-tlm.toml', name: 'flight.tlm', state: 'changed', detail: '1 of yours not shared, 2 of theirs not yours' },
  { kind: 'categories', key: 'categories.toml', name: 'Event categories', state: 'folder', detail: null },
  { kind: 'widgets', key: 'widgets/level-bar', name: 'Level bar (code)', state: 'folder', detail: null },
];

run(
  'share',
  async ({ p, check, fake, until, text, visible, settingsLS }) => {
    const poll = async (fn, ms = 5000) => {
      const t = Date.now();
      while (Date.now() - t < ms) {
        if (await fn()) return true;
        await p.waitForTimeout(50);
      }
      return false;
    };
    const calls = (cmd) => p.evaluate((c) => window.__tlmFake.cfgCalls.filter((x) => x.cmd === c), cmd);
    const rowsOf = () => p.$$eval('#shareList [data-share-row]', (r) => r.map((x) => `${x.dataset.shareRow}|${x.dataset.state}|${x.querySelector('select')?.value ?? '-'}`));
    const openShare = async () => {
      await p.click('#exFolder');
      await p.click('.exmenu [data-ex="share"]');
      await until(() => !!document.querySelector('#shareDlg'));
    };
    await fake(
      (r) => {
        window.__tlmFake.pickFolder = r.dir;
        window.__tlmFake.cfgRows = r.rows;
        window.__tlmFake.cfgRecipes = ['Power'];
        const cat = { id: 'alarm', name: 'Alarm (shared)', color: '#ff0000', show_on_charts: true };
        window.__tlmFake.cfgFrom = { categories: [cat], widgets: [] };
        window.__tlmFake.cfgPull = { from: { categories: [cat], widgets: [{ id: 'level-bar', name: 'Level bar (code)', code: '({build(){},update(){}})', by: 'j.kim' }] }, views: [] };
      },
      { dir: DIR, rows: ROWS },
    );
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length === 2);
    check('the folder is read for shared configuration when it opens', (await calls('use')).some((c) => c.folder === DIR));
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');

    // Share… from the folder menu
    await openShare();
    check('Share… opens the dialog', true);
    await poll(async () => (await rowsOf()).length >= 4);
    const rows = await rowsOf();
    check('rows by kind, what is the same left out', rows.length === 5 && !rows.some((r) => r.includes('smooth')), rows);
    check(
      "yours the folder lacks: Share; the folder's own: Use; what differs: Leave",
      rows.join() === 'views:views/flight-review.tlmview.json|new|share,events:events/flight-tlm.toml|changed|,categories:categories.toml|folder|use,recipes:recipes/power.toml|changed|,widgets:widgets/level-bar|folder|use',
      rows,
    );
    check('events say what a merge would do', /1 of yours not shared, 2 of theirs not yours/.test(await text('[data-share-row="events:events/flight-tlm.toml"]')));
    check('a changed recipe offers both sides', (await p.$$eval('[data-share-row="recipes:recipes/power.toml"] option', (o) => o.map((x) => x.value))).join() === 'share,use,');
    check('the button says what it will do', (await text('#shareGo')) === 'Share 1, use 2', await text('#shareGo'));
    await p.check('#shareSame');
    check('the ones that are the same can be shown', (await rowsOf()).length === 6);

    // pick the recipe to share too, then Apply
    await p.selectOption('[data-share-row="recipes:recipes/power.toml"] select', 'share');
    await p.click('#shareGo');
    await poll(async () => (await calls('share')).length === 1);
    const sh = (await calls('share'))[0];
    check('only the picked rows are written', [...sh.keys].sort().join() === 'recipes:recipes/power.toml,views:views/flight-review.tlmview.json', sh.keys);
    check(
      'with the app\'s own settings: layouts by file name, categories',
      sh.own.views.some((v) => v.file === 'flight-review.tlmview.json' && v.view.name === 'Flight review') && sh.own.categories.length >= 5 && sh.own.categories[0].show_on_charts === true && Array.isArray(sh.own.widgets),
    );
    check('the open recording is named by its path in the folder', sh.own.recordings.some((r) => r.name === 'flight.tlm' && typeof r.file === 'number'), sh.own.recordings);
    const pl = (await calls('pull'))[0];
    check("the folder's are taken", [...pl.keys].sort().join() === 'categories:categories.toml,widgets:widgets/level-bar', pl?.keys);
    check('it says what it did', await until(() => /Wrote 2 files to \.tlmbench\/; took 2 from the folder/.test(document.querySelector('#shareDone')?.textContent ?? '')), await text('#shareDone').catch(() => ''));
    const ls = await settingsLS();
    check('a category from the folder replaces yours by id', ls.eventCats.find((c) => c.id === 'alarm')?.name === 'Alarm (shared)', ls.eventCats);
    check('a widget from the folder arrives marked shared, asking before its code runs', ls.customWidgets.some((w) => w.id === 'sw_level-bar' && w.shared && w.by === 'j.kim' && w.code), ls.customWidgets);
    check('then the rows are read again: what was picked is the same now; the events left alone still differ', await poll(async () => (await rowsOf()).filter((r) => !r.startsWith('events')).every((r) => r.includes('|same|'))) && (await rowsOf()).some((r) => r.startsWith('events') && r.includes('|changed|')), await rowsOf());
    await p.keyboard.press('Escape');
    check('Esc closes it', !(await visible('#shareDlg')));

    // Settings › Shared configuration
    await p.click('[data-rail="settings"]');
    await p.waitForSelector('#sharedSet');
    check('Settings says the folder is read and counts the categories it shares', /Reading the folder/.test(await text('#sharedWhere')) && /1 event category comes from the folder/.test(await text('#sharedCats')), await text('#sharedSet'));
    await p.click('#setShare');
    check('its Share… button opens the dialog', await until(() => !!document.querySelector('#shareDlg')));
    await p.keyboard.press('Escape');
    await p.click('[data-rail="explorer"]');

    // the folder's recipes are marked in the manager
    await fake(() => window.__tlmFake.recipes.push({ name: 'Power', expr: 'accel_x * 2', unit: 'W', category: 'Derived', decimals: 1, automatic: false }));
    await p.click('#exMore');
    await p.click('.exmenu [data-ex="recipes"]');
    check('the manager marks a recipe the folder shares', await until(() => /Shared/.test(document.querySelector('#recipesDlg [data-recipe="Power"]')?.textContent ?? '')));
    await p.keyboard.press('Escape');

    // a read-only folder offers Export…
    await fake(() => {
      window.__tlmFake.cfgWritable = false;
      window.__tlmFake.cfgRows = [{ kind: 'recipes', key: 'recipes/power.toml', name: 'Power', state: 'new', detail: null }];
      window.__tlmFake.pickFolder = '/out/config';
    });
    await openShare();
    await until(() => !!document.querySelector('#shareRo'));
    check('it says the folder looks read-only and offers Export', /read-only/.test(await text('#shareRo')) && /Export/.test(await text('#shareDlg header')), await text('#shareRo'));
    check('nothing can be shared: the row is left alone', (await p.$$('[data-share-row] select')).length === 0 && (await p.$eval('#shareGo', (b) => b.disabled)));
    await p.click('#shareExport');
    await poll(async () => (await calls('export')).length === 1);
    check('Export… writes to the folder you choose', (await calls('export'))[0].out === '/out/config' && (await until(() => /Wrote 2 files to \/out\/config/.test(document.querySelector('#shareDone')?.textContent ?? ''))), await text('#shareDone').catch(() => ''));
    await p.keyboard.press('Escape');

    // a folder that looks writable but isn't is caught on write, and Export… takes over
    await fake((d) => {
      window.__tlmFake.cfgWritable = true;
      window.__tlmFake.readOnly.push(d);
    }, DIR);
    await openShare();
    await poll(async () => (await rowsOf()).length === 1);
    await p.click('#shareGo');
    check('the failed write is reported and Export… appears', (await until(() => /Read-only/.test(document.querySelector('#shareErr')?.textContent ?? ''))) && (await until(() => !!document.querySelector('#shareExport'))), await text('#shareErr').catch(() => ''));
    await p.keyboard.press('Escape');
    await fake((d) => (window.__tlmFake.readOnly = window.__tlmFake.readOnly.filter((x) => x !== d)), DIR);

    // a pair of files opens with the alignment it was left with
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    await p.click('#explorer .row.slot[data-slot="2"]');
    await p.selectOption('[data-falign="2"]', 'manual');
    await p.fill('[data-foff="2"]', '1.5');
    await p.press('[data-foff="2"]', 'Tab');
    await until(() => window.__tlmFake.timemaps.length === 1);
    const tm = await fake(() => window.__tlmFake.timemaps[0]);
    check('the alignment is kept for the pair', tm.method === 'manual' && tm.seconds === 1.5 && tm.folder === DIR && tm.knots.length === 1 && tm.locked === false, tm);
    // close the second file and add it again: the saved map is found
    await fake(
      ([a, b, s]) => {
        window.__tlmFake.aligns.length = 0;
        window.__tlmFake.timemapsByPath = { [`${a}|${b}`]: { method: 'manual', seconds: s, channel: '', locked: true, knots: [['0', String(s * 1e9)]], pairs: [] } };
      },
      [A, B, 2],
    );
    await p.click('#explorer .row.slot[data-slot="2"] [data-closeslot]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 0);
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    await p.waitForTimeout(300);
    if (!(await p.$('[data-lock="2"]'))) await p.click('#explorer .row.slot[data-slot="2"]');
    await p.waitForSelector('[data-lock="2"]');
    check('adding the file again opens it as it was left: locked, 2 s after the primary, nothing measured', (await p.isChecked('[data-lock="2"]')) && Math.abs(+(await p.inputValue('[data-foff="2"]')) - 2) < 0.01 && (await fake(() => window.__tlmFake.aligns.length)) === 0, await p.inputValue('[data-foff="2"]'));
  },
  { files, launch: null, settings: { workspaces: [{ schema: 1, name: 'Flight review', panes: [{ channels: ['accel_x'] }] }] } },
);
