// Derived channels: Derive… from the selection bar (A − B for two channels), the live check naming the character,
// the recipe saved to the library and attached to the file, the ƒ row and its pane, Edit recipe… (a rename keeps the
// file's recipe), Remove from this file with Undo, an automatic recipe reaching another file, Remove from every file.
import { run } from './lib.js';

run(
  'derived',
  async ({ p, check, text, until, fake, visible, settingsLS, wait }) => {
    const A = '/data/flight.tlm';
    const B = '/data/other.tlm';
    const ch = (n) => `#explorer .row.chan[data-ch="${n}"]`;
    const dlg = '#deriveDlg';
    const rows = () => p.$$eval('#explorer .row.chan', (e) => e.sort((a, b) => a.dataset.ch - b.dataset.ch).map((x) => x.querySelector('.nm').textContent));
    const field = (k) => p.inputValue(`${dlg} [data-r="${k}"]`);
    const status = () => text('#recStat');

    // ---- two channels selected: A − B
    await p.click(`${ch(0)} .nm`);
    await p.click(`${ch(1)} .nm`, { modifiers: ['ControlOrMeta'] });
    check('the selection bar offers Derive…', /Derive…/.test(await text('#selBar')), await text('#selBar'));
    await p.click('#selBar [data-sel="derive"]');
    check('the dialog opens', await until(() => !!document.querySelector('#deriveDlg')));
    check('with two channels the templates are A − B, A ÷ B, A × B, A + B', (await p.$$eval(`${dlg} [data-r="tpl"] option`, (o) => o.map((x) => x.textContent))).join('|') === 'A − B|A ÷ B|A × B|A + B');
    check('the expression starts as A − B', (await field('expr')) === 'accel_x - accel_y', await field('expr'));
    check('and is named after the channels', (await field('name')) === 'accel_x − accel_y', await field('name'));
    check('it is checked against the file: a preview', await until(() => /samples · from -2.5 to 2.5/.test(document.querySelector('#recStat')?.textContent ?? '')), await status());
    await p.selectOption(`${dlg} [data-r="tpl"]`, 'prod');
    check('a template fills the expression', (await field('expr')) === 'accel_x * accel_y', await field('expr'));

    // autocomplete: functions and channels for the word at the cursor, accepted with Tab
    await p.fill(`${dlg} [data-r="expr"]`, '');
    await p.focus(`${dlg} [data-r="expr"]`);
    await p.keyboard.type('sm');
    check('typing a function\'s start suggests it', await until(() => [...document.querySelectorAll('#deriveSugg [data-sugg]')].some((e) => e.dataset.sugg === 'sma(')), await p.$$eval('#deriveSugg li', (e) => e.map((x) => x.textContent)));
    await p.keyboard.press('Tab');
    check('Tab accepts it, bracket and all', (await field('expr')) === 'sma(', await field('expr'));
    await p.keyboard.type('mo');
    check('and a channel\'s start suggests the channels', await until(() => [...document.querySelectorAll('#deriveSugg [data-sugg]')].map((e) => e.dataset.sugg).join().startsWith('motor_rpm,motor_rpm_target')), await p.$$eval('#deriveSugg li', (e) => e.map((x) => x.textContent)));
    await p.keyboard.press('ArrowDown');
    await p.keyboard.press('Enter');
    check('the arrow keys choose, Enter accepts', (await field('expr')) === 'sma(motor_rpm_target', await field('expr'));
    await p.fill(`${dlg} [data-r="expr"]`, 'accel_x * accel_y');
    await p.selectOption(`${dlg} [data-r="tpl"]`, 'prod');

    // the check names the character
    await p.fill(`${dlg} [data-r="expr"]`, 'accel_x * (accel_y');
    check('a missing bracket is named at its character', await until(() => /Expected "\)" \(at character 19\)/.test(document.querySelector('#recStat')?.textContent ?? '')), await status());
    check('and the dialog can\'t be saved', await p.$eval(`${dlg} [data-ok]`, (b) => b.disabled));
    await p.fill(`${dlg} [data-r="expr"]`, 'foo(accel_x)');
    check('an unknown function is named', await until(() => /There's no function called foo/.test(document.querySelector('#recStat')?.textContent ?? '')), await status());
    await p.fill(`${dlg} [data-r="expr"]`, 'nope + 1');
    check('so is a missing channel', await until(() => /There's no channel called nope/.test(document.querySelector('#recStat')?.textContent ?? '')), await status());
    await p.fill(`${dlg} [data-r="expr"]`, 'mean(accel_x)');
    check('a one-number expression says it is flat', await until(() => /One number for the whole file/.test(document.querySelector('#recStat')?.textContent ?? '')), await status());
    await p.fill(`${dlg} [data-r="expr"]`, 'accel_x - accel_y');
    await p.fill(`${dlg} [data-r="name"]`, 'Tilt');
    await p.fill(`${dlg} [data-r="unit"]`, 'g');
    await until(() => !document.querySelector(`${dlg} [data-ok]`).disabled);

    await p.click(`${dlg} [data-ok]`);
    check('Add saves the recipe in the library', await until(() => window.__tlmFake.recipeSaves.length === 1));
    const saved = (await fake(() => window.__tlmFake.recipeSaves))[0];
    check('with its expression, unit, category and decimals; not automatic', saved.recipe.name === 'Tilt' && saved.recipe.expr === 'accel_x - accel_y' && saved.recipe.unit === 'g' && saved.recipe.category === 'Derived' && saved.recipe.decimals === 3 && saved.recipe.automatic === false && saved.replacing === null, saved);
    check('the dialog closes', await until(() => !document.querySelector('#deriveDlg')));
    check('the core shows the file with it', await until(() => window.__tlmFake.derivedApplied.length === 1 && window.__tlmFake.derivedApplied[0].recipes.join() === 'Tilt'));
    check('Explorer lists it after the recording\'s channels, marked ƒ', (await rows()).join() === 'accel_x,accel_y,roll,pitch,motor_rpm,motor_rpm_target,Tilt' && (await p.$$(`${ch(6)} .dglyph`)).length === 1, await rows());
    check('and plots it in the selected pane', /Tilt/.test((await text('#stack')) ?? ''), await text('#stack'));
    check('it is attached to this file in settings', (await settingsLS()).fileRecipes?.[A]?.join() === 'Tilt', (await settingsLS()).fileRecipes);

    // ---- Edit recipe… (a rename)
    await p.click(`${ch(6)}`, { button: 'right' });
    check('a derived channel\'s right-click menu adds Edit recipe… and Remove from this file or every file', (await p.$$eval('.chanmenu .mi [data-x], .chanmenu .mi', (e) => e.map((x) => x.textContent))).slice(2).join() === 'Edit recipe…,Derive from this…,Remove from this file,Remove from every file', await p.$$eval('.chanmenu .mi', (e) => e.map((x) => x.textContent)));
    await p.click('.chanmenu [data-cm="edit"]');
    check('Edit opens with its expression', await until(() => !!document.querySelector('#deriveDlg')) && (await field('expr')) === 'accel_x - accel_y' && (await field('name')) === 'Tilt');
    check('editing has no template picker', (await p.$$(`${dlg} [data-r="tpl"]`)).length === 0);
    await p.fill(`${dlg} [data-r="name"]`, 'Lean');
    await p.fill(`${dlg} [data-r="expr"]`, 'abs(accel_x - accel_y)');
    await until(() => !document.querySelector(`${dlg} [data-ok]`).disabled);
    await p.click(`${dlg} [data-ok]`);
    check('Save replaces the recipe by its old name', await until(() => window.__tlmFake.recipeSaves.length === 2) && (await fake(() => window.__tlmFake.recipeSaves))[1].replacing === 'Tilt');
    check('the file keeps it under the new name', await until(() => [...document.querySelectorAll('#explorer .row.chan')].some((r) => r.dataset.ch === '6' && r.querySelector('.nm').textContent === 'Lean')) && (await settingsLS()).fileRecipes?.[A]?.join() === 'Lean', await rows());
    check('and the pane follows the channel', /Lean/.test((await text('#stack')) ?? '') && !/Tilt/.test((await text('#stack')) ?? ''), await text('#stack'));

    // ---- Remove from this file, then Undo
    await p.click(ch(6), { button: 'right' });
    await p.click('.chanmenu [data-cm="rm-file"]');
    check('Remove from this file takes the row away', await until(() => document.querySelectorAll('#explorer .row.chan').length === 6), await rows());
    check('and says so, with Undo', /Removed Lean from flight.tlm/.test(await text('#wsNote')) && /Undo/.test(await text('#wsNote')), await text('#wsNote'));
    check('the recipe stays in the library', (await fake(() => window.__tlmFake.recipes.map((r) => r.name))).join() === 'Lean');
    await p.click('#wsNote button');
    check('Undo brings it back', await until(() => document.querySelectorAll('#explorer .row.chan').length === 7), await rows());

    // ---- a recipe for any file, from the title row's ƒ button, reaches another file
    await fake((b) => (window.__tlmFake.files[b] = {}), B);
    await p.click(ch(2) + ' .nm');
    await p.click('#exDerive');
    check('the ƒ button in the title row opens the dialog from the selected channel', await until(() => !!document.querySelector('#deriveDlg')) && (await field('expr')) === 'delta(roll)', await field('expr'));
    await p.selectOption(`${dlg} [data-r="tpl"]`, 'abs');
    await p.fill(`${dlg} [data-r="name"]`, 'Roll size');
    await p.check(`${dlg} input[value="auto"]`);
    await until(() => !document.querySelector(`${dlg} [data-ok]`).disabled);
    await p.click(`${dlg} [data-ok]`);
    check('it is saved automatic', await until(() => window.__tlmFake.recipeSaves.length === 3) && (await fake(() => window.__tlmFake.recipeSaves))[2].recipe.automatic === true);
    check('the file shows it too, after Lean (channels keep their numbers)', await until(() => document.querySelectorAll('#explorer .row.chan').length === 8) && (await rows()).slice(6).join() === 'Lean,Roll size', await rows());
    await fake((b) => window.__tlmFake.drop([b]), B);
    check('another file opens with it', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2));
    const open2 = (await fake(() => window.__tlmFake.opened)).filter((o) => o.path === '/data/other.tlm');
    check('(the core was asked for the file\'s recipes, none attached)', open2.length === 1);
    check('the second file lists only the automatic recipe', await until(() => document.querySelector('#explorer .row.chan[data-derived] .nm')?.textContent === 'Roll size') && (await rows()).slice(6).join() === 'Roll size', await rows());

    // ---- Remove from every file deletes the recipe
    await p.click('#explorer .row.chan[data-derived]', { button: 'right' });
    await p.click('.chanmenu [data-cm="rm-all"]');
    check('Remove from every file deletes the recipe', await until(() => !(window.__tlmFake.recipes.some((r) => r.name === 'Roll size'))));
    check('and the row goes', await until(() => document.querySelectorAll('#explorer .row.chan[data-derived]').length === 0), await rows());
    await p.click('#wsNote button');
    check('Undo restores the recipe and the row', await until(() => window.__tlmFake.recipes.some((r) => r.name === 'Roll size')) && (await until(() => document.querySelectorAll('#explorer .row.chan[data-derived]').length === 1)));

    // ---- Esc closes the dialog without saving
    await p.click(ch(0) + ' .nm');
    await p.click('#selBar [data-sel="derive"]');
    await until(() => !!document.querySelector('#deriveDlg'));
    const before = (await fake(() => window.__tlmFake.recipeSaves)).length;
    await p.keyboard.press('Escape');
    check('Esc closes the dialog and saves nothing', !(await visible('#deriveDlg')) && (await fake(() => window.__tlmFake.recipeSaves)).length === before);
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
