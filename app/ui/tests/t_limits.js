// Limits: the channel editor's Limits tab sets warn and alarm limits by channel name (kept in settings); value tiles
// are green inside them, yellow past warn and red past alarm, show them as text, and open the Limits tab on a click;
// common channels start with sensible limits until a channel has an entry of its own.
import { run } from './lib.js';

run(
  'limits',
  async ({ p, check, until, settingsLS }) => {
    await p.click('#stack section.pane [data-vmenu]');
    await p.click('.menu [data-vm="values"]');
    check('a values pane', await until(() => !!document.querySelector('#stack [data-tiles] [data-tile]')));
    const tile = (ch) => `#stack [data-tile="${ch}"]`;
    const level = (ch) => p.$eval(tile(ch), (e) => e.dataset.level);
    const chans = await p.$$eval('#stack [data-tile]', (e) => e.map((x) => x.dataset.tile));
    check('without limits a tile has no colour', await until(() => document.querySelector('#stack [data-tile="0"]').dataset.level === ''), await level(chans[0]));
    check('it says how to set them', /set limits/i.test(await p.$eval(tile(chans[0]), (e) => e.textContent)));

    await p.click(tile(chans[0]));
    check('a click on a tile opens the editor on Limits', await until(() => document.querySelector('#chEditor [data-tab="limits"]')?.getAttribute('aria-selected') === 'true'));
    await p.fill('#chEditor [data-lim="ah"]', '1000');
    await p.fill('#chEditor [data-lim="wh"]', '900');
    check('limits are kept by channel name', await until(() => true) && (await settingsLS()).limits?.accel_x?.ah === 1000, JSON.stringify((await settingsLS()).limits));
    check('inside its limits the tile is green', await until((c) => document.querySelector(`#stack [data-tile="${c}"]`).dataset.level === 'ok', chans[0]), await level(chans[0]));
    check('the tile lists its limits', /alarm ≥ 1000/.test(await p.$eval(tile(chans[0]), (e) => e.textContent)));

    await p.fill('#chEditor [data-lim="wh"]', '-1000');
    check('past warn it turns yellow', await until((c) => document.querySelector(`#stack [data-tile="${c}"]`).dataset.level === 'warn', chans[0]), await level(chans[0]));
    await p.fill('#chEditor [data-lim="ah"]', '-2000');
    check('past alarm it turns red', await until((c) => document.querySelector(`#stack [data-tile="${c}"]`).dataset.level === 'alarm', chans[0]), await level(chans[0]));

    await p.click('#chEditor [data-clear]');
    check('No limits empties them and stays empty', await until((c) => document.querySelector(`#stack [data-tile="${c}"]`).dataset.level === '', chans[0]) && JSON.stringify((await settingsLS()).limits?.accel_x) === '{}');

    // a channel with starting limits: roll's tile is inside them
    const roll = await p.$$eval('#stack [data-tile]', (e) => e.find((x) => /roll/.test(x.textContent))?.dataset.tile);
    check('a common channel starts with limits', roll == null || (await until((c) => document.querySelector(`#stack [data-tile="${c}"]`).dataset.level === 'ok', roll)));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
