// Shared views: the + menu lists the views in the folder's .tlmbench/views/ (one that can't be read is named and
// greyed); opening one opens its file with its layout and time range in a read-only tab (Revert; Save as… makes your
// copy); Share with this folder… writes one (the file by its path in the folder, the time range, who shared it); a
// folder that can't be written offers Export file…; a file outside the folder can't be shared; a shared view's tab
// comes back after a reload.
import { run } from './lib.js';

const ROOT = '/rec';
const A = '/rec/flights/a.tlm';
const OUT = '/other/c.tlm';
const T0 = 1767225600000000000n;
const REVIEW = {
  schema: 1,
  name: 'Flight review',
  by: 'a.chen',
  files: [{ path: 'flights/a.tlm', slot: 1 }],
  view: [10, 20],
  panes: [{ channels: ['accel_x'] }, { channels: ['roll'], view: 'table' }],
};

run(
  'shared',
  async ({ p, check, fake, until, settingsLS, wait, text }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    const names = () => p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.querySelector('.nm').textContent + (x.getAttribute('aria-selected') === 'true' ? '*' : '')));
    const front = () => p.$eval('#wsTabs .wstab[aria-selected=true]', (t) => t.dataset.tab);
    const tabMenu = async () => p.click(`#wsTabs .wstab[data-tab="${await front()}"] .tm`);
    const menuItems = () => p.$$eval('.tabmenu [data-ws]', (b) => b.map((x) => x.dataset.ws));
    const noteText = () => p.$eval('#wsNote .txt', (e) => e.textContent).catch(() => '');
    await fake((v) => (window.__tlmFake.views['/rec'] = v), { 'flight-review.tlmview.json': REVIEW, 'broken.tlmview.json': '{' });

    // the + menu lists the folder's shared views
    await p.click('#wsTabNew');
    await until(() => document.querySelectorAll('.addmenu [data-shopen]').length === 2);
    const rows = await p.$$eval('.addmenu [data-shopen]', (b) => b.map((x) => `${x.querySelector('.nm').textContent}:${x.querySelector('.hint').textContent}:${x.disabled}`));
    check('shared views listed', rows.join() === "broken.tlmview.json:can't be read:true,Flight review:by a.chen:false", rows);

    // opening one: its file, its layout, its time range
    await p.click('.addmenu [data-shopen="flight-review.tlmview.json"]');
    check('a tab for the view, on its file', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Flight review') && (await names()).join() === 'a.tlm,Flight review*', await names());
    check('its layout', JSON.stringify(await app('panes')) === '[[0],[2]]' && (await app('modes')).join() === 'chart,table', [await app('panes'), await app('modes')]);
    const v = await app('view');
    check('its time range', v.t0 === String(T0 + 10000000000n) && v.t1 === String(T0 + 20000000000n), v);
    await tabMenu();
    check('read-only: no Save layout or Delete', (await menuItems()).join() === 'saveas,revert,share,export,newwin', await menuItems());
    check('Update shared copy', /Update shared copy/.test(await p.$eval('.tabmenu [data-ws="share"]', (e) => e.textContent)));
    await p.keyboard.press('Escape');

    // Save as… makes it yours
    await tabMenu();
    await p.click('.tabmenu [data-ws="saveas"]');
    check('Save as… starts from its name', (await p.inputValue('#wsName')) === 'Flight review');
    await p.fill('#wsName', 'My review');
    await p.press('#wsName', 'Enter');
    check('your copy', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'My review') && (await settingsLS()).workspaces.some((w) => w.name === 'My review'), await names());
    await tabMenu();
    check('now with Save layout and Delete', (await menuItems()).includes('save') && (await menuItems()).includes('delete'), await menuItems());
    await p.keyboard.press('Escape');

    // a shared layout that plots a recipe you lack offers to add it (D-cfg-07)
    await fake((v) => (window.__tlmFake.views['/rec']['power.tlmview.json'] = v), { schema: 1, name: 'Power review', by: 'a.chen', files: [{ path: 'flights/a.tlm', slot: 1 }], panes: [{ channels: ['accel_x'] }, { channels: ['Power'] }], recipes: [{ name: 'Power', expr: 'accel_x * accel_y', unit: 'W', category: 'Derived', decimals: 1 }] });
    await p.click('#wsTabNew');
    await until(() => !!document.querySelector('.addmenu [data-shopen="power.tlmview.json"]'));
    await p.click('.addmenu [data-shopen="power.tlmview.json"]');
    check('opening it says which recipes you lack and offers to add them', await until(() => /plots a recipe you don't have: Power/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')) && /Add recipe/.test(await p.$eval('#wsNote button.tbtn', (e) => e.textContent).catch(() => '')), await noteText());
    await p.click('#wsNote button.tbtn');
    check('Add recipe puts it in the library', await until(() => window.__tlmFake.recipes.some((r) => r.name === 'Power' && r.expr === 'accel_x * accel_y' && !r.automatic)), await fake(() => window.__tlmFake.recipes));
    await p.click('#wsTabs .wstab[data-tip="/rec/flights/a.tlm"]');

    // a recording that was renamed or moved: the shared events are filed under its old name, and one click updates them (D-cfg-04)
    await fake(() => (window.__tlmFake.sharedRenames = [{ file: 'events/a.toml', from: 'old/a.tlm', to: 'flights/a.tlm' }]));
    await p.click('#wsTabs .wstab[data-tip="/rec/flights/a.tlm"]');
    await p.click('#wsTabs .wstab:not([aria-selected=true])');
    await p.click('#wsTabs .wstab[data-tip="/rec/flights/a.tlm"]');
    check('the folder\'s events under an old name are offered an update', await until(() => /filed under its old name, old\/a\.tlm/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')) && /Update the folder/.test(await p.$eval('#wsNote button:not(.ibtn)', (e) => e.textContent).catch(() => '')), await noteText());
    await p.click('#wsNote button:not(.ibtn)');
    check('one click writes them, and says how many', await until(() => /Updated 1 shared events file/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')) && (await fake(() => window.__tlmFake.renamesApplied.length)) === 1, await noteText());
    check('it asked about the open recordings by their path in the folder', JSON.stringify(await fake(() => window.__tlmFake.renamesApplied[0].recordings.map((r) => r.name))) === '["flights/a.tlm"]', await fake(() => window.__tlmFake.renamesApplied));

    // Share with this folder…
    await p.click('#wsTabs .wstab[data-tip="/rec/flights/a.tlm"]');
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'a.tlm');
    await tabMenu();
    await p.click('.tabmenu [data-ws="share"]');
    await p.fill('#wsName', 'Motors, first look');
    await p.press('#wsName', 'Enter');
    const shared = await until(() => window.__tlmFake.views['/rec']['motors-first-look.tlmview.json']);
    const view = await fake(() => window.__tlmFake.views['/rec']['motors-first-look.tlmview.json']);
    check('written to .tlmbench/views/', shared && view.name === 'Motors, first look' && view.by === 'tester', view);
    check('with its file by path in the folder, and the time range', JSON.stringify(view.files) === '[{"path":"flights/a.tlm","slot":1}]' && view.view[0] === 0 && view.view[1] > 199, view);
    check('the note says where', /Shared as \.tlmbench\/views\/motors-first-look\.tlmview\.json/.test(await noteText()), await noteText());

    // a folder that can't be written: Export file… instead
    await fake(() => (window.__tlmFake.readOnly = ['/rec']));
    await fake(() => (window.__tlmFake.savePick = '/Users/me/motors.tlmview.json'));
    await tabMenu();
    await p.click('.tabmenu [data-ws="share"]');
    await p.fill('#wsName', 'Again');
    await p.press('#wsName', 'Enter');
    check("it says it can't write", await until(() => /Couldn't share "Again"/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')), await noteText());
    await p.locator('#wsNote button', { hasText: 'Export file…' }).click();
    const ex = await until(() => window.__tlmFake.exported[0]);
    const exported = await fake(() => window.__tlmFake.exported[0]);
    check('Export file… writes where you chose', ex && exported.path === '/Users/me/motors.tlmview.json' && exported.view.name === 'Again', exported);

    // a folder known to be read-only when it opens: the tab menu offers Export file… in place of Share (sheet A6)
    await fake(() => Object.assign(window.__tlmFake, { savePick: '/Users/me/ro.tlmview.json', pickFolder: '/rec' }));
    await p.click('#exFolder');
    await p.click('.exmenu [data-ex="open-folder"]');
    await until(() => window.__tlmFake.cfgCalls.filter((c) => c.cmd === 'use').length >= 2);
    await wait(200);
    await tabMenu();
    const roItem = await text('.tabmenu [data-ws="share"]');
    check('a read-only folder: the tab menu says so and offers Export file… instead of Share', /Export file…/.test(roItem) && /read-only/.test(roItem) && !/Share with/.test(roItem), roItem);
    check('with no second Export file… item', !(await p.$('.tabmenu [data-ws="export"]')));
    await p.click('.tabmenu [data-ws="share"]');
    check('one click exports the layout where you chose', await until(() => window.__tlmFake.exported.some((e) => e.path === '/Users/me/ro.tlmview.json')));
    await fake(() => (window.__tlmFake.readOnly = []));
    await p.click('#exFolder');
    await p.click('.exmenu [data-ex="open-folder"]');
    await until(() => window.__tlmFake.cfgCalls.filter((c) => c.cmd === 'use').length >= 3);

    // a file outside the folder
    await fake((f) => window.__tlmFake.drop([f]), OUT);
    await until((f) => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === f, OUT);
    await fake(() => (window.__tlmFake.readOnly = []));
    await tabMenu();
    await p.click('.tabmenu [data-ws="share"]');
    await p.fill('#wsName', 'Outside');
    await p.press('#wsName', 'Enter');
    check("a file outside the folder isn't shared", await until(() => /isn't in it/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')), await noteText());

    // the shared view's tab comes back after a reload
    await p.click('#wsTabNew');
    await p.click('.addmenu [data-shopen="flight-review.tlmview.json"]');
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Flight review');
    await wait(300);
    const before = await names();
    // the fake core starts afresh on a reload: the folder keeps its views
    await p.addInitScript((v) => (window.__tlmFake.views['/rec'] = v), await fake(() => window.__tlmFake.views['/rec']));
    await p.reload();
    await wait(1000);
    check('after a reload', JSON.stringify(await names()) === JSON.stringify(before), [await names(), before]);
    await tabMenu();
    check('still the shared one', (await p.$eval('.tabmenu [data-ws="share"]', (e) => e.textContent)).includes('Update shared copy'));
  },
  { files: { [A]: {}, [OUT]: {} }, launch: null, settings: { folder: ROOT } },
);
