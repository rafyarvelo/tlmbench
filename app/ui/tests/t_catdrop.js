// Explorer categories (D-ui-09): a channel dragged onto a category row moves into it (Undo puts it back; ⌘Z steps back
// through moves one by one in the same history as pane edits, X4), Cmd/Ctrl-click
// picks several categories together (their channels are the channels picked) and Enter plots them.
import { run } from './lib.js';

run(
  'catdrop',
  async ({ p, check, until, settingsLS, text }) => {
    const cat = (n) => `#explorer .row.cat[data-cat="${n}"]`;
    check('channels are grouped by the start of their names', await until(() => !!document.querySelector('#explorer .row.cat[data-cat="motor"]') && !!document.querySelector('#explorer .row.cat[data-cat="batt"]')));
    // drag the one channel of "other" onto motor
    const lone = await p.$eval('#explorer .row.cat[data-cat="other"] ~ .row.chan .nm', (e) => e.textContent).catch(() => null);
    check('an ungrouped channel exists to move', lone === 'pitch', lone);
    await p.dragAndDrop('#explorer .row.chan[data-ch="4"]', cat('motor'));
    check('dropping it on a category moves it there', await until(() => Object.values(JSON.parse(localStorage.getItem('tlmbench.settings') || '{}').catOverride ?? {}).includes('motor')), (await settingsLS()).catOverride);
    check('and says so, with Undo', /Moved pitch to motor/.test(await text('#wsNote')));
    await p.click('#wsNote button.tbtn');
    check('Undo puts it back', await until(() => !Object.values(JSON.parse(localStorage.getItem('tlmbench.settings') || '{}').catOverride ?? {}).includes('motor')));

    // X4 (Nov 22): category moves are steps of the one Undo history, with the panes' edits: ⌘Z steps back one by one
    const cats = async () => (await settingsLS()).catOverride ?? {};
    const inMotor = async () => Object.entries(await cats()).filter(([, v]) => v === 'motor').map(([k]) => k.split(/[|:]/).pop()).sort().join();
    await p.evaluate(() => window.__tlmApp.setPanes([{ channels: [0, 1] }]));
    await until(() => !!document.querySelector('#stack [data-pane="0"] [data-chip-x]'));
    await p.dragAndDrop('#explorer .row.chan[data-ch="4"]', cat('motor'));
    await until(async () => true);
    const chips0 = (await p.evaluate(() => window.__tlmApp.panes())).flat().length;
    await p.hover('#stack [data-pane="0"] .chip');
    await p.click('#stack [data-pane="0"] .chip [data-chip-x]');
    const chips1 = (await p.evaluate(() => window.__tlmApp.panes())).flat().length;
    await p.dragAndDrop('#explorer .row.chan[data-ch="2"]', cat('motor'));
    check('two channels moved, a chip taken out between', (await until(async () => true)) && /batt_v/.test(await inMotor()) && /pitch/.test(await inMotor()) && chips1 === chips0 - 1, [await inMotor(), chips0, chips1]);
    await p.click('#stack');
    await p.keyboard.press('ControlOrMeta+z');
    check('⌘Z takes back the last move only', await until(() => true) && (await inMotor()).includes('pitch') && !(await inMotor()).includes('batt_v'), await inMotor());
    await p.keyboard.press('ControlOrMeta+z');
    check('then the pane edit before it', (await p.evaluate(() => window.__tlmApp.panes())).flat().length === chips0 && (await inMotor()).includes('pitch'));
    await p.keyboard.press('ControlOrMeta+z');
    check('then the first move', !(await inMotor()), await inMotor());
    await p.keyboard.press('ControlOrMeta+Shift+z');
    check('⇧⌘Z does the move again', (await inMotor()).includes('pitch'), await inMotor());
    await p.keyboard.press('ControlOrMeta+z');
    check('and ⌘Z takes it back', !(await inMotor()), await inMotor());

    // several categories picked together
    await p.click(cat('motor'), { modifiers: ['ControlOrMeta'], position: { x: 60, y: 8 } });
    await p.click(cat('batt'), { modifiers: ['ControlOrMeta'], position: { x: 60, y: 8 } });
    check('Cmd/Ctrl-click picks categories', (await p.getAttribute(cat('motor'), 'aria-selected')) === 'true' && (await p.getAttribute(cat('batt'), 'aria-selected')) === 'true');
    check('their channels are the channels picked', await until(() => /4 selected|\b4\b/.test(document.querySelector('#selBar')?.textContent ?? '')), await text('#selBar'));
    await p.focus(cat('batt'));
    await p.keyboard.press('Enter');
    check('Enter plots them all', await until(() => document.querySelectorAll('#stack [data-pane] .chip, #stack [data-pane] [data-chip]').length >= 0) && (await p.evaluate(() => window.__tlmApp.panes().flat().length)) >= 4, await p.evaluate(() => window.__tlmApp.panes()));
  },
  {
    files: { '/data/flight.tlm': { channels: ['motor_rpm', 'motor_rpm_target', 'batt_v', 'batt_a', 'pitch'] } },
    launch: '/data/flight.tlm',
  },
);
