// The native menu bar (owner decision 8): built from one command table, so menus and shortcuts can't drift. The window sends the
// table to the core; a menu click runs the same action as the shortcut; a text field keeps ⌘Z; no menu item binds a plain key.
import { run } from './lib.js';

run(
  'menubar',
  async ({ p, check, until, fake }) => {
    const mac = await p.evaluate(() => /Mac/.test(navigator.platform));
    const cmd = mac ? 'Meta' : 'Control';
    const info = await p.evaluate(() => window.__tlmApp.commandInfo());
    const table = info.commands;
    check('the table has the six menus', ['File', 'Edit', 'View', 'Pane', 'Window', 'Help'].every((m) => table.some((c) => c.menu === m)), table.map((c) => c.menu));
    check('every entry has an action', table.every((c) => info.run.includes(c.id)), table.filter((c) => !info.run.includes(c.id)).map((c) => c.id));
    check('and every action is in the table', info.run.every((id) => table.some((c) => c.id === id)), info.run.filter((id) => !table.some((c) => c.id === id)));
    check('ids are unique', new Set(table.map((c) => c.id)).size === table.length);
    check('an accelerator always has ⌘ (Ctrl) or ⌥, so a text field keeps every plain key', table.filter((c) => c.accel).every((c) => /(CmdOrCtrl|Cmd|Ctrl|Alt)\+/.test(c.accel)), table.filter((c) => c.accel).map((c) => c.accel));
    check('a single-key shortcut is shown in the label, never bound', table.filter((c) => c.key).every((c) => !c.accel));
    check('no two commands share an accelerator', new Set(table.filter((c) => c.accel).map((c) => c.accel)).size === table.filter((c) => c.accel).length);

    const sent = await until(() => !!window.__tlmFake.menuSet);
    check('the window sends the table to the core', sent);
    const set = await fake(() => window.__tlmFake.menuSet);
    check('with every entry, in order, and its accelerator', set.length === table.length && set.every((e, i) => e.id === table[i].id && e.menu === table[i].menu && (e.accel ?? null) === (table[i].accel ?? null)), set.map((e) => e.id));
    check('a single-key shortcut is in the label: Fit (F)', set.find((e) => e.id === 'fit')?.label === 'Fit to the data (F)', set.find((e) => e.id === 'fit')?.label);
    check('the core is told each item\'s state', await until((n) => (window.__tlmFake.menuState ?? []).length === n, table.length));
    const st = (id) => fake((i) => (window.__tlmFake.menuState ?? []).find((u) => u.id === i), id);
    check('Event markers is checked while markers are on', (await st('events'))?.checked === true, await st('events'));

    // a click runs the command
    const evOn = () => p.evaluate(() => window.__tlmApp.settingsNow().eventsOn);
    const before = await evOn();
    await fake(() => window.__tlmFake.menuClick('events'));
    check('a menu click runs the same action as the shortcut: Event markers toggles', await until((b) => window.__tlmApp.settingsNow().eventsOn !== b, before), [before, await evOn()]);
    await p.keyboard.press('e');
    check('and E toggles it back (a single key stays with the keyboard handler)', await until((b) => window.__tlmApp.settingsNow().eventsOn === b, before), [before, await evOn()]);

    // a key and the menu click for the same key a moment later run once
    const f0 = await p.evaluate(() => window.__tlmApp.focusOn());
    await p.keyboard.press(`${cmd}+.`);
    await fake(() => window.__tlmFake.menuClick('focus'));
    check('a shortcut and its menu click right after it run once', (await p.evaluate(() => window.__tlmApp.focusOn())) === !f0, await p.evaluate(() => window.__tlmApp.focusOn()));
    await p.keyboard.press(`${cmd}+.`);

    // ⌘Z in a text field is the field's own undo; the menu's Undo sends it there
    await p.click('#b-more');
    await p.keyboard.press('Escape');
    await p.evaluate(() => {
      const i = document.createElement('input');
      i.id = 'probe';
      document.body.appendChild(i);
      window.__undoCalls = [];
      const ex = document.execCommand.bind(document);
      document.execCommand = (c, ...a) => (window.__undoCalls.push(c), ex(c, ...a));
      i.focus();
    });
    await fake(() => window.__tlmFake.menuClick('undo'));
    check('Undo in a text field is the field\'s own', await until(() => (window.__undoCalls ?? []).includes('undo')), await p.evaluate(() => window.__undoCalls));
    await p.keyboard.press(`${cmd}+z`);
    check('and ⌘Z there is left to the field', (await p.evaluate(() => window.__undoCalls.length)) >= 1 && !(await p.evaluate(() => /Nothing to undo/.test(document.body.textContent ?? ''))));
    await p.evaluate(() => document.getElementById('probe')?.remove());
    await p.locator('body').click({ position: { x: 5, y: 5 } }).catch(() => {});
    await fake(() => window.__tlmFake.menuClick('undo'));
    check('Undo elsewhere is the app\'s: "Nothing to undo." with nothing to undo', await until(() => /Nothing to undo/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')), await p.evaluate(() => document.querySelector('#wsNote')?.textContent));
  },
);
