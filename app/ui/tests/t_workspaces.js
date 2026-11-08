// Workspaces: Save as… in a tab's menu saves its layout (channels by name) and the tab then shows it; edits mark it
// unsaved without touching the file's own layout; Save layout and Revert changes; the + menu opens a layout on the
// file in front (its missing channels named), starts an empty one and deletes one (with Undo); a file stays open
// while any tab shows it; workspace tabs come back after a reload.
import { run } from './lib.js';

const A = '/data/flight.tlm';
const B = '/data/other.tlm';
const files = { [A]: {}, [B]: { channels: ['accel_x', 'volts'] } };

run(
  'workspaces',
  async ({ p, check, fake, until, settingsLS, wait }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    const names = () => p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.querySelector('.nm').textContent + (x.querySelector('[data-dirty]') ? '°' : '') + (x.getAttribute('aria-selected') === 'true' ? '*' : '')));
    const front = () => p.$eval('#wsTabs .wstab[aria-selected=true]', (t) => t.dataset.tab);
    const tabMenu = async () => p.click(`#wsTabs .wstab[data-tab="${await front()}"] .tm`);
    const saved = async () => (await settingsLS()).workspaces;
    const calls = (c) => fake((c) => window.__tlmFake.calls.filter((x) => x === c).length, c);
    const duplicatePane = async () => {
      await p.hover('#stack [data-pane="0"] .phead');
      await p.click('#stack [data-pane="0"] [data-pmenu]');
      await p.locator('.panemenu .mi', { hasText: /^Duplicate$/ }).click();
    };
    await fake((f) => window.__tlmFake.drop([f]), A);
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1);
    const fileLayout = JSON.stringify((await settingsLS()).layouts?.[A] ?? null);

    // Save as…: a name is needed; then the tab shows the layout
    await tabMenu();
    await p.click('.tabmenu [data-ws="saveas"]');
    await p.click('.tabmenu .wsname [type=submit]');
    check('a name is needed', /name/.test(await p.$eval('.tabmenu .wserr', (e) => e.textContent)));
    await p.fill('#wsName', 'Review');
    await p.press('#wsName', 'Enter');
    check('the tab shows the layout', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm').textContent === 'Review'), await names());
    const ws = (await saved())[0];
    check('Explorer still lists the file', (await p.$$eval('#openFiles .ofnm', (b) => b.map((x) => x.textContent))).join() === 'flight.tlm');
    check('saved with channel names', ws?.name === 'Review' && ws.panes.length === 3 && ws.panes[0].channels[0] === 'accel_x', ws);

    // edits are unsaved changes; the file's own layout stays as it was
    await duplicatePane();
    check('an edit marks it unsaved', await until(() => !!document.querySelector('#wsTabs [aria-selected=true] [data-dirty]')), await names());
    check('...and the window title has a dot', /^• .* — TelemetryBench$/.test(await p.title()), await p.title());
    check("the file's layout is untouched", JSON.stringify((await settingsLS()).layouts?.[A] ?? null) === fileLayout);
    await tabMenu();
    await p.click('.tabmenu [data-ws="save"]');
    check('Save layout', await until(() => !document.querySelector('#wsTabs [data-dirty]')) && (await saved())[0].panes.length === 4, (await saved())[0]);
    await duplicatePane();
    await tabMenu();
    await p.click('.tabmenu [data-ws="revert"]');
    check('Revert changes', await until(() => window.__tlmApp.panes().length === 4 && !document.querySelector('#wsTabs [data-dirty]')), await app('panes'));
    await p.keyboard.press('Meta+s');
    check('⌘S with nothing to save keeps it', (await saved())[0].panes.length === 4);

    // the file again: its own tab, sharing the open file
    await fake((f) => (window.__tlmFake.pick = f), A);
    await p.keyboard.press('Meta+o');
    check('a file tab beside the workspace tab', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2), await names());
    check('the file is not opened twice', (await calls('open')) === 1, await calls('open'));
    check('with its own layout', (await app('panes')).length === 3, await app('panes'));

    // the + menu: an empty layout, and your layouts
    await p.click('#wsTabNew');
    await p.click('.addmenu [data-wsnew]');
    await p.fill('#wsName', 'Review');
    await p.press('#wsName', 'Enter');
    check('names are unique', /already/.test(await p.$eval('.addmenu .wserr', (e) => e.textContent)));
    await p.fill('#wsName', 'Blank');
    await p.press('#wsName', 'Enter');
    check('an empty layout in a tab', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm').textContent === 'Blank') && JSON.stringify(await app('panes')) === '[[]]', await names());
    await p.click('#wsTabNew');
    const rows = await p.$$eval('.addmenu [data-wsopen]', (b) => b.map((x) => x.dataset.wsopen + ':' + x.querySelector('.hint').textContent));
    check('the + menu lists your layouts', rows.join() === 'Review:open in a tab,Blank:open in a tab', rows);
    await p.click('.addmenu [data-wsopen="Review"]');
    check('an open layout comes to the front', (await p.$eval('#wsTabs [aria-selected=true] .nm', (e) => e.textContent)) === 'Review' && (await names()).length === 3, await names());

    // closing a tab keeps the file while another shows it
    await p.click(`#wsTabs .wstab[data-tab="${await front()}"] .tx`);
    check('the file stays open', (await calls('close')) === 0 && (await names()).length === 2, [await calls('close'), await names()]);

    // a layout on another file: its missing channels are named
    await fake((f) => window.__tlmFake.drop([f]), B);
    await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === '/data/other.tlm');
    await p.click('#wsTabNew');
    await p.click('.addmenu [data-wsopen="Review"]');
    check('opens on the file in front', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm').textContent === 'Review') && JSON.stringify((await app('panes'))[0]) === '[0]', await app('panes'));
    const msg = await p.$eval('#wsNote .txt', (e) => e.textContent);
    check('missing channels named', /^other\.tlm has no channels accel_y, roll, pitch, …/.test(msg), msg);

    // after a reload the workspace tabs come back, the one in front too
    await wait(300);
    const before = await names();
    await p.reload();
    await wait(1000);
    check('workspace tabs come back', JSON.stringify(await names()) === JSON.stringify(before), [await names(), before]);

    // Delete layout, with Undo
    await p.click('#wsTabNew');
    await p.hover('.addmenu .lyrow:has([data-wsopen="Review"])');
    await p.click('.addmenu [data-wsdel="Review"]');
    check('deleting closes its tabs', await until(() => ![...document.querySelectorAll('#wsTabs .nm')].some((n) => n.textContent === 'Review')) && !(await saved()).some((w) => w.name === 'Review'), await names());
    await p.locator('button', { hasText: 'Undo' }).click();
    check('Undo brings it back', (await saved()).map((w) => w.name).join() === 'Review,Blank', (await saved()).map((w) => w.name));
  },
  { files, launch: null },
);
