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

    // the chips decide underneath
    await p.click('#eventsPanel .evcat[data-evcat="alarm"]');
    check('hiding a category in the panel hides it on the charts', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 1)), await drawn());
    await p.click('#eventsPanel [data-evcatall]');

    // a category's Charts switch
    await p.click('[data-rail="settings"]');
    check('Settings lists the categories', await until(() => document.querySelectorAll('#evCatSet [data-evcatrow]').length === 5));
    await p.click('#evCatSet [data-evcatrow="alarm"] .catchart input');
    check('a category with Charts off is not marked', await until(() => window.__tlmApp.eventsDrawn().every((n) => n === 1)), await drawn());
    await p.click('#evCatSet [data-evcatrow="alarm"] .catchart input');
    await p.click('#catAdd');
    check('Add makes a category', await until(() => document.querySelectorAll('#evCatSet [data-evcatrow]').length === 6));
    await p.click('#evCatSet [data-catdel]:last-of-type');
    await p.click('[data-rail="explorer"]');

    // the pointer readout names the event under it
    const on = await plotAt(0, 0.55);
    await p.mouse.move(on.x, on.y);
    check('the pointer readout names the event under it', await until(() => /Hover test/.test(document.querySelector('#hoverTxt')?.textContent ?? '')), await text('#hoverTxt'));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
