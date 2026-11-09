// Value labels: the channel editor's Value labels tab (value = text lines) is kept by channel name; tiles show the text
// where a value has one and the number elsewhere; the core is told the labels expressions can name; Derive…'s
// autocomplete offers a channel's value names after `== '`.
import { run } from './lib.js';

run(
  'labels',
  async ({ p, check, until, fake, settingsLS, text }) => {
    await p.click('#stack section.pane [data-vmenu]');
    await p.click('.menu [data-vm="values"]');
    check('a values pane', await until(() => !!document.querySelector('#stack [data-tiles] [data-tile="0"] .tval')));
    check('the core read the values', await until(() => window.__tlmFake.lastValues?.[0] != null));
    const v = await fake(() => window.__tlmFake.lastValues[0]);
    check('the tile starts as a number', /\d/.test(await text('#stack [data-tile="0"] .tval')), await text('#stack [data-tile="0"] .tval'));

    await p.click('#stack [data-tile="0"]');
    await p.click('#chEditor [data-tab="labels"]');
    await p.fill('#chEditor [data-vl]', `${v} = CRUISE\n3 = LAND`);
    await p.dispatchEvent('#chEditor [data-vl]', 'change');
    check('labels are kept by channel name', Object.keys((await settingsLS()).valueMaps?.accel_x ?? {}).length === 2, JSON.stringify((await settingsLS()).valueMaps));
    check('the tile shows the text', await until(() => /CRUISE/.test(document.querySelector('#stack [data-tile="0"] .tval')?.textContent ?? '')), await text('#stack [data-tile="0"] .tval'));
    check('the core is told the labels', await until(() => (window.__tlmFake.exprEnv?.labels ?? []).some((l) => l.channel === 'accel_x' && l.text === 'CRUISE')), JSON.stringify(await fake(() => window.__tlmFake.exprEnv)));
    check('other channels still show numbers', /\d/.test(await text('#stack [data-tile="1"] .tval')));

    await p.click('#chEditor [data-vlclear]');
    check('No labels brings the number back', await until(() => !/CRUISE/.test(document.querySelector('#stack [data-tile="0"] .tval')?.textContent ?? '')));
    check('and empties the map', !(await settingsLS()).valueMaps?.accel_x);
    await p.click('#chEditor [data-close]');

    // the menu's entry opens the same tab
    await p.click('#stack .chip[data-ch="0"]', { button: 'right' });
    await p.click('.menu [data-cm="labels"]');
    await p.fill('#chEditor [data-vl]', '1 = CRUISE\n2 = CLIMB');
    await p.dispatchEvent('#chEditor [data-vl]', 'change');
    await p.click('#chEditor [data-close]');

    // Derive…'s autocomplete offers them after a comparison
    await p.click('#stack .chip[data-ch="0"]', { button: 'right' });
    await p.click('.menu [data-cm="derive"]');
    await p.fill('#deriveDlg [data-r="expr"]', '');
    await p.focus('#deriveDlg [data-r="expr"]');
    await p.keyboard.type("accel_x == 'CR");
    check('after == \' the channel\'s value names are suggested', await until(() => [...document.querySelectorAll('#deriveSugg [data-sugg]')].map((e) => e.dataset.sugg).join() === "CRUISE'"), await p.$$eval('#deriveSugg li', (e) => e.map((x) => x.textContent)));
    await p.keyboard.press('Escape');
    if (await p.$('#deriveDlg')) await p.keyboard.press('Escape');
    await p.evaluate(() => document.querySelector('#deriveDlg footer .tbtn:not(.primary)')?.click());
    await p.click('#stack .chip[data-ch="0"]', { button: 'right' });
    await p.click('.menu [data-cm="labels"]');
    check('Value labels… opens that tab', await until(() => document.querySelector('#chEditor [data-tab="labels"]')?.getAttribute('aria-selected') === 'true'));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
