// Icons and the pinned strip (D-gauge-06): Settings › Icons takes an SVG of your own (refusing what isn't one), an event
// category and a pin can use it, a layout or recipe pinned from Layouts or the Recipes manager is one click in the top
// bar, and an unpinned one goes; a layout's right-click and the pin's own right-click pin and unpin; a shared layout pins
// too (X1, Nov 22); the SVG box is wide and resizable.
import { run } from './lib.js';

const SVG = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="8"/></svg>';

run(
  'pins',
  async ({ p, check, fake, until, settingsLS }) => {
    await p.click('[data-rail="settings"]');
    check('Settings has an Icons section', await until(() => !!document.querySelector('#iconSet')));
    await p.fill('#iconName', 'Ring');
    await p.fill('#iconSvg', 'not an svg');
    await p.click('#iconAdd');
    check('something that is not an SVG is refused', await until(() => /not an SVG/.test(document.querySelector('#iconErr')?.textContent ?? '')) && !(await settingsLS()).userIcons?.length);
    await p.fill('#iconSvg', SVG);
    await p.click('#iconAdd');
    check('an SVG is kept, with a preview', await until(() => !!document.querySelector('#iconSet [data-icon="Ring"] svg')) && (await settingsLS()).userIcons?.[0]?.name === 'Ring');
    await p.fill('#iconName', 'Ring');
    await p.fill('#iconSvg', SVG);
    await p.click('#iconAdd');
    check('a name is used once', await until(() => /already an icon called Ring/.test(document.querySelector('#iconErr')?.textContent ?? '')));

    await p.selectOption('#evCatSet [data-evcatrow="alarm"] [data-caticon]', 'Ring');
    check('an event category can have an icon', (await settingsLS()).eventCats.find((c) => c.id === 'alarm')?.icon === 'Ring');

    await p.click('[data-rail="layouts"]');
    await p.hover('#layoutsPanel .lyrow');
    await p.click('#layoutsPanel [data-lypin="Review"]');
    check('a layout pinned from Layouts is in the top bar', await until(() => !!document.querySelector('#pinStrip [data-pin="Review"]')) && (await settingsLS()).pins?.[0]?.kind === 'layout');
    await p.click('#pinStrip [data-pin="Review"]');
    check('one click opens it in a tab', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Review'));

    await fake(() => window.__tlmFake.recipes.push({ name: 'Power', expr: 'accel_x * accel_y', unit: 'W', category: 'Derived', decimals: 1, automatic: false }));
    await p.click('[data-rail="explorer"]');
    await p.click('#exMore');
    await p.click('.exmenu [data-ex="recipes"]');
    await p.click('#recipesDlg [data-recipe="Power"] [data-pin]');
    await p.keyboard.press('Escape');
    check('a recipe can be pinned too', await until(() => !!document.querySelector('#pinStrip [data-pin="Power"]')));
    await p.click('#pinStrip [data-pin="Power"]');
    check('its pin adds the recipe to the file in front', await until(() => [...document.querySelectorAll('#explorer .row.chan[data-derived] .nm')].some((e) => e.textContent === 'Power')));

    await p.click('[data-rail="settings"]');
    await p.selectOption('#iconSet [data-pin="Review"] [data-pinicon]', 'Ring');
    check('a pin takes an icon from the list, shown in the bar', await until(() => (document.querySelector('#pinStrip [data-pin="Review"]')?.innerHTML ?? '').includes('<circle')));
    await p.click('#iconSet [data-unpin="Review"]');
    check('Unpin takes it off the bar', await until(() => !document.querySelector('#pinStrip [data-pin="Review"]')));
    await p.click('#iconSet [data-icondel="Ring"]');
    check('deleting an icon clears it from categories', (await settingsLS()).eventCats.find((c) => c.id === 'alarm')?.icon == null && !(await settingsLS()).userIcons.length);

    // X1 (Nov 22): the SVG box is wide and resizable, with a preview; Settings leads to Your widgets
    const box = await p.$eval('#iconSvg', (e) => ({ h: e.getBoundingClientRect().height, w: e.getBoundingClientRect().width, r: getComputedStyle(e).resize, pw: e.parentElement.getBoundingClientRect().width }));
    check('the icon SVG box is the section\'s width, several lines tall, and resizes', box.h >= 70 && box.w >= box.pw - 2 && box.r === 'vertical', box);
    await p.fill('#iconSvg', SVG);
    check('with a preview as you paste', await until(() => !!document.querySelector('[data-iconprev] svg circle')));
    await p.fill('#iconSvg', '');
    await p.click('#setWidgets');
    check('Settings › Widgets › Edit… opens Your widgets', await until(() => !!document.querySelector('#widgetDlg')));
    await p.keyboard.press('Escape');

    // a layout's right-click menu pins and unpins; the pinned button's right-click unpins
    await p.click('[data-rail="layouts"]');
    await p.click('#layoutsPanel [data-lopen="Review"]', { button: 'right' });
    check('a layout\'s right-click menu has Pin', await until(() => !!document.querySelector('.lymenu [data-lym="pin"]')));
    await p.click('.lymenu [data-lym="pin"]');
    check('which pins it', await until(() => !!document.querySelector('#pinStrip [data-pin="Review"]')));
    check('a pinned layout shows its pin in the list', await p.$eval('#layoutsPanel [data-lypin="Review"]', (e) => getComputedStyle(e).visibility === 'visible' && e.getAttribute('aria-pressed') === 'true'));
    await p.click('#layoutsPanel [data-lopen="Review"]', { button: 'right' });
    await p.click('.lymenu [data-lym="unpin"]');
    check('and then Unpin', await until(() => !document.querySelector('#pinStrip [data-pin="Review"]')));
    await p.click('#layoutsPanel [data-lopen="Review"]', { button: 'right' });
    await p.click('.lymenu [data-lym="pin"]');
    await until(() => !!document.querySelector('#pinStrip [data-pin="Review"]'));
    await p.click('#pinStrip [data-pin="Review"]', { button: 'right' });
    await p.click('.pinmenu [data-pm="unpin"]');
    check('a pinned button\'s right-click unpins it', await until(() => !document.querySelector('#pinStrip [data-pin="Review"]')));

    // a layout shared in the folder pins too, and its pin opens it
    await fake(() => { window.__tlmFake.pickFolder = '/data'; window.__tlmFake.views['/data'] = { 'review.tlmview.json': { schema: 1, name: 'Flight review', by: 'a.chen', panes: [{ channels: ['accel_x'] }], files: [{ path: 'flight.tlm', slot: 1 }] } }; });
    await p.click('[data-rail="explorer"]');
    await p.click('#exFolder');
    await p.click('[data-rail="layouts"]');
    await until(() => !!document.querySelector('[data-lshared="review.tlmview.json"]'));
    await p.click('[data-lshared="review.tlmview.json"]', { button: 'right' });
    await p.click('.lymenu [data-lym="pin"]');
    check('a shared layout can be pinned', await until(() => !!document.querySelector('#pinStrip [data-pin="review.tlmview.json"]')) && (await settingsLS()).pins.some((x) => x.kind === 'shared'));
    check('named for its layout', (await p.getAttribute('#pinStrip [data-pin="review.tlmview.json"]', 'aria-label')) === 'Flight review');
    await p.click('#pinStrip [data-pin="review.tlmview.json"]');
    check('its pin opens it', await until(() => [...document.querySelectorAll('#wsTabs .nm')].some((n) => n.textContent === 'Flight review')));
    await p.click('[data-lypin="review.tlmview.json"]');
    check('its pin button in the list unpins it', await until(() => !document.querySelector('#pinStrip [data-pin="review.tlmview.json"]')));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm', settings: { workspaces: [{ schema: 1, name: 'Review', panes: [{ channels: ['accel_x'] }] }] } },
);
