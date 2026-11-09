// Events: the rail's Events panel lists what you added to the open recording (kept by the core per recording); a marker
// at cursor A and a range A–B are added with a click each; a name and category are edited inline and deleted there;
// the list filters by text and by category chips (click hides, double-click shows only it, All and None), groups, and
// frames an event on a click.
import { run } from './lib.js';

run(
  'events',
  async ({ p, check, text, until, fake, plotAt, settingsLS }) => {
    const app = (f, a) => p.evaluate(([f, a]) => window.__tlmApp[f](a), [f, a]);
    const rows = () => p.$$eval('#eventsPanel .ev[data-ev]', (e) => e.map((x) => x.textContent.replace(/\s+/g, ' ').trim()));
    await p.click('[data-rail="events"]');
    check('the Events panel opens from the rail', await until(() => !!document.querySelector('#eventsPanel')));
    check('with nothing yet it says how to add', /No events/.test(await text('#eventsPanel')), await text('#eventsPanel'));

    await p.click('#evMarker');
    check('a marker without cursor A says to place it', /Place cursor A first/.test(await text('#evMsg')), await text('#evMsg'));

    // cursors A and B
    await p.keyboard.press('c');
    const a = await plotAt(0, 0.3);
    const b = await plotAt(0, 0.6);
    await p.mouse.click(a.x, a.y);
    await p.mouse.click(b.x, b.y);
    check('both cursors placed', await until(() => !!window.__tlmApp.cursors().A && !!window.__tlmApp.cursors().B));

    await p.click('#evMarker');
    check('Marker at A adds an instant', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 1), await rows());
    const marker = (await fake(() => window.__tlmFake.eventSaves.at(-1)));
    check('saved by the core with A as its start and no duration', marker.duration === '0' && marker.name === 'Marker' && marker.category === 'alarm' && marker.source === 'user', marker);
    check('and opens for editing', await until(() => !!document.querySelector('#eventsPanel [data-evf="name"]')));
    await p.fill('#eventsPanel [data-evf="name"]', 'Liftoff');
    await p.dispatchEvent('#eventsPanel [data-evf="name"]', 'change');
    check('a name is edited inline and saved', await until(() => /Liftoff/.test(document.querySelector('#eventsPanel .ev')?.textContent ?? '')) && (await fake(() => window.__tlmFake.eventSaves.at(-1).name)) === 'Liftoff');
    await p.selectOption('#eventsPanel [data-evf="cat"]', 'milestone');
    check('so is its category', await until(() => window.__tlmFake.eventSaves.at(-1).category === 'milestone'));
    await p.click('#eventsPanel [data-evdone]');

    await p.click('#evRange');
    check('Range A–B adds one with a duration', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 2));
    const range = await fake(() => window.__tlmFake.eventSaves.at(-1));
    check('its start is the earlier cursor and it lasts until the later', range.name === 'Range' && BigInt(range.duration) > 0n && BigInt(range.start) <= BigInt(marker.start) + BigInt(range.duration), range);
    await p.click('#eventsPanel [data-evdone]');
    check('the list is in start order, the instant saying so', (await rows()).length === 2 && /instant/.test((await rows()).join('|')), await rows());
    check('the core keeps them (a reload of the list gives the same)', (await fake(() => Object.values(window.__tlmFake.events)[0].length)) === 2);

    // filters
    await p.fill('#evQ', 'lift');
    check('the filter narrows by name', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 1) && /1 of 2/.test(await text('#evSum')), await text('#evSum'));
    await p.fill('#evQ', '');
    check('the chips count each category', (await p.$$eval('#eventsPanel .evcat[data-evcat]', (e) => e.map((x) => x.textContent.replace(/\s+/g, ' ').trim()))).join() === 'Alarm 1,Milestone 1', await p.$$eval('#eventsPanel .evcat[data-evcat]', (e) => e.map((x) => x.textContent)));
    await p.click('#eventsPanel .evcat[data-evcat="alarm"]');
    check('a chip click hides that category', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 1), await rows());
    await p.click('#eventsPanel [data-evcatall]');
    check('All shows everything again', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 2));
    await p.dblclick('#eventsPanel .evcat[data-evcat="alarm"]');
    check('a double-click shows only that category', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 1 && /Range/.test(document.querySelector('#eventsPanel .ev').textContent)), await rows());
    await p.click('#eventsPanel [data-evcatnone]');
    check('None hides every category', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 0));
    await p.click('#eventsPanel [data-evcatall]');

    // grouping and Collapse all
    await p.click('#evCollapse');
    check('Collapse all groups an ungrouped list by category and closes the groups', await until(() => document.querySelectorAll('#eventsPanel [data-evg][aria-expanded="false"]').length === 2 && document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 0), await text('#eventsPanel'));
    await p.click('#evExpand');
    check('Expand all opens them', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 2));
    await p.selectOption('#evGroup', '');

    // a click frames the event
    const before = await app('view');
    await p.click('#eventsPanel .ev[data-ev]');
    check('a click on an event moves the view to it', await until((b) => JSON.stringify(window.__tlmApp.view()) !== b, JSON.stringify(before)));

    // delete
    await p.click('#eventsPanel [data-evedit]');
    await p.click('#eventsPanel [data-evdel]');
    check('Delete removes it', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 1));
    check('and the core forgets it', (await fake(() => Object.values(window.__tlmFake.events)[0].length)) === 1);
    check('categories are in settings', ((await settingsLS()).eventCats ?? []).length === 6);
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
