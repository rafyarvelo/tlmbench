// The Events panel's filters travel with a saved layout (S5.2-16): Save as… keeps the hidden categories and "Charts
// follow these filters" with the layout; changing them marks the layout tab unsaved; opening the layout in a new tab
// brings them back; Revert puts them back as saved. A layout saved with the default filters keeps none.
import { run } from './lib.js';

const A = '/data/flight.tlm';
const files = { [A]: {} };

run(
  'layoutevents',
  async ({ p, check, fake, until, settingsLS }) => {
    const front = () => p.$eval('#wsTabs .wstab[aria-selected=true]', (t) => t.dataset.tab);
    const tabMenu = async () => p.click(`#wsTabs .wstab[data-tab="${await front()}"] .tm`);
    const saved = async () => (await settingsLS()).workspaces ?? [];
    const pressed = (id) => p.$eval(`#eventsPanel .evcat[data-evcat="${id}"]`, (b) => b.getAttribute('aria-pressed'));
    await fake(() => {
      const t0 = 1767225600000000000n;
      window.__tlmFake.events['/data/flight.tlm'] = [
        { id: 'a', name: 'Spike', start: String(t0 + 1000000000n), duration: '0', category: 'alarm', source: 'user', channels: [] },
        { id: 'b', name: 'Hover', start: String(t0 + 2000000000n), duration: '0', category: 'mode', source: 'user', channels: [] },
      ];
    });
    await fake((f) => window.__tlmFake.drop([f]), A);
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1);

    // a layout saved with the default filters keeps none
    await tabMenu();
    await p.click('.tabmenu [data-ws="saveas"]');
    await p.fill('#wsName', 'Plain');
    await p.press('#wsName', 'Enter');
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Plain');
    check('default filters are not saved', (await saved())[0]?.events == null, (await saved())[0]);

    await p.click('[data-rail="events"]');
    await until(() => document.querySelectorAll('#eventsPanel .evcat[data-evcat]').length >= 2);
    await p.click('#eventsPanel .evcat[data-evcat="alarm"]');
    check('the chip hides alarm', await until(() => document.querySelector('#eventsPanel .evcat[data-evcat="alarm"]')?.getAttribute('aria-pressed') === 'false'));
    check('a filter change marks the layout tab unsaved', await until(() => !!document.querySelector('#wsTabs [aria-selected=true] [data-dirty]')));
    await p.click('#eventsPanel [data-evfollow]');
    await tabMenu();
    await p.click('.tabmenu [data-ws="save"]');
    const ws = await until(async () => true) && (await saved()).find((w) => w.name === 'Plain');
    check('Save layout keeps the hidden categories and the follow switch', ws?.events?.hidden?.join() === 'alarm' && ws.events.follow === false, ws);
    check('and the tab is saved', await until(() => !document.querySelector('#wsTabs [aria-selected=true] [data-dirty]')));

    // Revert: back to the saved filters
    await p.click('#eventsPanel .evcat[data-evcat="mode"]');
    await until(() => !!document.querySelector('#wsTabs [aria-selected=true] [data-dirty]'));
    await tabMenu();
    await p.click('.tabmenu [data-ws="revert"]');
    check('Revert puts the filters back', await until(() => document.querySelector('#eventsPanel .evcat[data-evcat="mode"]')?.getAttribute('aria-pressed') === 'true') && (await pressed('alarm')) === 'false');

    // the file in a tab of its own has its own filters; closing the layout's tab and opening the layout again brings its
    await fake((f) => (window.__tlmFake.pick = f), A);
    await p.keyboard.press('ControlOrMeta+o');
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2);
    check("the file's tab has its own filters", await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm' && document.querySelector('#eventsPanel .evcat[data-evcat="alarm"]')?.getAttribute('aria-pressed') === 'true'));
    const plain = await p.$$eval('#wsTabs .wstab', (t) => t.findIndex((x) => x.querySelector('.nm').textContent === 'Plain') + 1);
    await p.hover(`#wsTabs .wstab:nth-child(${plain})`);
    await p.click(`#wsTabs .wstab:nth-child(${plain}) .tx`);
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1);
    await p.click('#wsTabNew');
    await p.click('.addmenu [data-wsopen="Plain"]');
    check('opening the layout brings its filters', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Plain' && document.querySelector('#eventsPanel .evcat[data-evcat="alarm"]')?.getAttribute('aria-pressed') === 'false'));
    check('and its follow switch', !(await p.$eval('#eventsPanel [data-evfollow]', (b) => b.getAttribute('aria-pressed') === 'true')));
  },
  { files, launch: null },
);
