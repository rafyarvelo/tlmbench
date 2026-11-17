// Event markers on charts and the overview: drawn for events in view; the top bar's E (and the key) hides every marker;
// a pane's ⋯ menu hides them in that pane only (kept with the layout); a category's Charts switch and the Events
// panel's chips decide underneath (unless unlinked); the pointer readout names the event under it; Settings has the
// categories (add, rename, recolour, delete).
import { run } from './lib.js';

run(
  'eventmarks',
  async ({ p, check, text, until, fake, plotAt, settingsLS, wait }) => {
    const app = (f, a) => p.evaluate(([f, a]) => window.__tlmApp[f](a), [f, a]);
    // an instant and a range the core already has for the file
    await fake(() => {
      const t0 = BigInt(window.__tlmApp.view().t0);
      const t1 = BigInt(window.__tlmApp.view().t1);
      const at = (f) => String(t0 + ((t1 - t0) * BigInt(f)) / 100n);
      window.__tlmFake.events['/data/flight.tlm'] = [
        { id: 'a', name: 'Spike', start: at(30), duration: '0', category: 'alarm', source: 'user', channels: [] },
        { id: 'b', name: 'Hover test', start: at(50), duration: String(((t1 - t0) * 10n) / 100n), category: 'mode', source: 'user', channels: [] },
      ];
    });
    // the list is read when the file is shown: switch to the Events panel and add one to reload
    await p.click('[data-rail="events"]');
    await p.keyboard.press('c');
    const a = await plotAt(0, 0.8);
    await p.mouse.click(a.x, a.y);
    await p.click('#evMarker');
    check('three events (two seeded, one added)', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 3));
    await p.keyboard.press('Escape');
    const drawn = () => app('eventsDrawn');
    check('every chart marks the events in view', await until(() => window.__tlmApp.eventsDrawn().length >= 3 && window.__tlmApp.eventsDrawn().every((n) => n === 3)), await drawn());

    // E hides every marker
    await p.mouse.move(5, 5);
    await p.keyboard.press('e');
    check('E hides every marker', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 0)), await drawn());
    check('and the top bar button shows it off', (await p.getAttribute('#b-events', 'aria-pressed')) === 'false');
    await p.click('#b-events');
    check('the top bar button brings them back', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 3)), await drawn());

    // one pane only
    await p.click('#stack [data-pane="0"] [data-pmenu]');
    await p.locator('.panemenu .mi', { hasText: /^Event markers in this pane$/ }).click();
    check('a pane\'s menu hides the markers in that pane only', await until(() => window.__tlmApp.eventsDrawn()[0] === 0 && window.__tlmApp.eventsDrawn()[1] === 3), await drawn());
    check('and it is kept with the layout', JSON.stringify((await settingsLS()).layouts ?? {}).includes('"events":false'));
    await p.click('#stack [data-pane="0"] [data-pmenu]');
    await p.locator('.panemenu .mi', { hasText: /^Event markers in this pane$/ }).click();
    check('turning it back on', await until(() => window.__tlmApp.eventsDrawn()[0] === 3));

    // one pane only (owner decision 5, sheet E1): with E off, a pane's own toggle (among its hover controls) shows them there
    await p.keyboard.press('e');
    check('E off hides them in panes without a choice of their own', await until(() => window.__tlmApp.eventsDrawn()[1] === 0 && window.__tlmApp.eventsDrawn()[2] === 0), await drawn());
    await p.hover('#stack [data-pane="1"] .phead');
    await p.click('#stack [data-pane="1"] [data-pevents]');
    check('the pane\'s Events toggle shows them in that pane only', await until(() => window.__tlmApp.eventsDrawn()[1] === 3 && window.__tlmApp.eventsDrawn()[2] === 0), await drawn());
    check('the toggle says so', (await p.getAttribute('#stack [data-pane="1"] [data-pevents]', 'aria-pressed')) === 'true');
    await p.keyboard.press('e');
    check('E on again: panes without a choice follow it', await until(() => window.__tlmApp.eventsDrawn()[2] === 3), await drawn());
    await p.hover('#stack [data-pane="1"] .phead');
    await p.click('#stack [data-pane="1"] [data-pevents]');
    check('and the pane\'s own Off stays off', await until(() => window.__tlmApp.eventsDrawn()[1] === 0 && window.__tlmApp.eventsDrawn()[2] === 3), await drawn());
    await p.click('#stack [data-pane="1"] [data-pevents]');

    // the chips decide underneath
    await p.click('#eventsPanel .evcat[data-evcat="alarm"]');
    check('hiding a category in the panel hides it on the charts', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 1)), await drawn());
    // "Charts follow these filters" (S5.2-16): off, the hidden chip no longer hides marks on the charts, in this tab only
    await p.click('#eventsPanel [data-evfollow]');
    check('with Charts follow these filters off, the charts mark the hidden category again', await until(() => window.__tlmApp.eventsDrawn().some((n) => n > 1)), await drawn());
    check('the tab\'s filters are kept with the session (S5.2-16)', await until(async () => true) && (await (async () => { for (let k = 0; k < 20; k++) { const e = Object.values((await settingsLS()).sessions ?? {}).map((x) => x.events?.['/data/flight.tlm']).find(Boolean); if (e && e.follow === false && e.hidden.includes('alarm')) return true; await p.waitForTimeout(150); } return false; })()), JSON.stringify(Object.values((await settingsLS()).sessions ?? {}).map((x) => x.events)));
    await p.click('#eventsPanel [data-evfollow]');
    check('on again, they follow the chips', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 1)), await drawn());
    await p.click('#eventsPanel [data-evcatall]');

    // a category's Charts switch
    await p.click('[data-rail="settings"]');
    check('Settings lists the categories', await until(() => document.querySelectorAll('#evCatSet [data-evcatrow]').length === 7));
    await p.click('#evCatSet [data-evcatrow="alarm"] .catchart input');
    check('a category with Charts off is not marked', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 1)), await drawn());
    await p.click('#evCatSet [data-evcatrow="alarm"] .catchart input');
    await p.click('#catAdd');
    check('Add makes a category', await until(() => document.querySelectorAll('#evCatSet [data-evcatrow]').length === 8));
    await p.click('#evCatSet [data-catdel]:last-of-type');
    await p.click('[data-rail="explorer"]');

    // the pointer readout names the event under it
    const on = await plotAt(0, 0.55);
    await p.mouse.move(on.x, on.y);
    check('the pointer readout names the event under it', await until(() => /Hover test/.test(document.querySelector('#hoverTxt')?.textContent ?? '')), await text('#hoverTxt'));
    // a tooltip on the marker: name, category, start and duration (D-ui-11)
    check('hovering a range marker shows a tooltip with its name, category, start and length', await until(() => /Hover test/.test(document.querySelector('#evTip')?.textContent ?? '') && /Mode change/.test(document.querySelector('#evTip')?.textContent ?? '') && /s/.test(document.querySelector('#evTip')?.textContent ?? '') && !/instant/.test(document.querySelector('#evTip')?.textContent ?? '')), await text('#evTip'));
    const off = await plotAt(0, 0.95);
    await p.mouse.move(off.x, off.y);
    check('it goes when the pointer leaves the marker', await until(() => !document.querySelector('#evTip')));
    // an instant is hit within a few pixels: look along its place
    for (const f of [0.3, 0.297, 0.303, 0.294, 0.306, 0.29, 0.31]) {
      const inst = await plotAt(0, f);
      await p.mouse.move(inst.x, inst.y);
      if (await until(() => !!document.querySelector('#evTip'), null, 400)) break;
    }
    check('an instant says so', await until(() => /Spike/.test(document.querySelector('#evTip')?.textContent ?? '') && /instant/.test(document.querySelector('#evTip')?.textContent ?? '')), await text('#evTip'));
    await p.mouse.move(5, 5);
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
