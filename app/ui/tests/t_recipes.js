// The Recipes manager (Explorer's ⋯ menu): the library's rows, "any file with these channels" on or off, Duplicate,
// Delete with Undo, Edit (the Derive dialog), Export… as the shared TOML text and Import… from it.
import { run } from './lib.js';

run(
  'recipes',
  async ({ p, check, text, until, fake, visible }) => {
    const dlg = '#recipesDlg';
    const names = () => p.$$eval(`${dlg} .rrow`, (e) => e.map((x) => x.dataset.recipe));
    await fake(() => {
      window.__tlmFake.recipes.push(
        { name: 'Power', expr: 'accel_x * accel_y', unit: 'W', category: 'Derived', decimals: 1, automatic: true },
        { name: 'Smooth', expr: 'sma(roll, 20)', unit: 'deg', category: 'Derived', decimals: 2, automatic: false },
      );
    });
    await p.click('#exMore');
    await p.click('.exmenu [data-ex="recipes"]');
    check('Recipes… opens the manager from the ⋯ menu', await until(() => !!document.querySelector('#recipesDlg')));
    check('it lists the library', (await names()).join() === 'Power,Smooth', await names());
    check('with each expression', /accel_x \* accel_y/.test(await text(`${dlg} [data-recipe="Power"]`)) && /sma\(roll, 20\)/.test(await text(`${dlg} [data-recipe="Smooth"]`)));
    check('Power applies to any file, Smooth doesn\'t', (await p.isChecked(`${dlg} [data-recipe="Power"] [data-auto]`)) && !(await p.isChecked(`${dlg} [data-recipe="Smooth"] [data-auto]`)));

    await p.check(`${dlg} [data-recipe="Smooth"] [data-auto]`);
    check('turning it on saves the recipe by its name, automatic', await until(() => window.__tlmFake.recipeSaves.at(-1)?.recipe.automatic === true && window.__tlmFake.recipeSaves.at(-1)?.replacing === 'Smooth'));
    check('and the open file gets it', await until(() => [...document.querySelectorAll('#explorer .row.chan[data-derived] .nm')].some((e) => e.textContent === 'Smooth')));

    await p.click(`${dlg} [data-recipe="Smooth"] [data-dup]`);
    check('Duplicate adds a copy that isn\'t automatic', await until(() => document.querySelectorAll('#recipesDlg .rrow').length === 3) && (await names()).includes('Smooth copy') && !(await p.isChecked(`${dlg} [data-recipe="Smooth copy"] [data-auto]`)), await names());

    await p.click(`${dlg} [data-recipe="Smooth copy"] [data-del]`);
    check('Delete removes it', await until(() => !window.__tlmFake.recipes.some((r) => r.name === 'Smooth copy')) && (await until(() => document.querySelectorAll('#recipesDlg .rrow').length === 2)), await names());
    check('and says so, with Undo', /Deleted recipe Smooth copy/.test(await text('#wsNote')) && /Undo/.test(await text('#wsNote')), await text('#wsNote'));
    await p.evaluate(() => document.querySelector('#wsNote button').click());
    check('Undo restores it', await until(() => window.__tlmFake.recipes.some((r) => r.name === 'Smooth copy')));

    // Export… then Import… (into a library missing one)
    await p.click('#recExport');
    check('Export… shows the shared text', await until(() => /schema = 1/.test(document.querySelector('#recText')?.value ?? '')) && /name = "Power"/.test(await p.inputValue('#recText')), await p.inputValue('#recText').catch(() => ''));
    const exported = await p.inputValue('#recText');
    await p.click(`${dlg} footer button`);
    await p.click(`${dlg} [data-recipe="Power"] [data-del]`);
    await until(() => !window.__tlmFake.recipes.some((r) => r.name === 'Power'));
    await p.click('#recImport');
    check('Import is disabled until there is text', await p.$eval('#recImportGo', (b) => b.disabled));
    await p.fill('#recPaste', 'not a recipe file');
    await p.click('#recImportGo');
    check('a file that isn\'t recipes is refused with the reason', await until(() => /Couldn't import that/.test(document.querySelector('#recErr')?.textContent ?? '')), await text('#recErr'));
    await p.fill('#recPaste', exported);
    await p.click('#recImportGo');
    check('importing the export brings the recipes back', await until(() => window.__tlmFake.recipes.some((r) => r.name === 'Power')) && (await until(() => document.querySelectorAll('#recipesDlg .rrow').length >= 2)), await names());

    // Edit opens the Derive dialog on the recipe
    await p.click(`${dlg} [data-recipe="Power"] [data-edit]`);
    check('Edit opens the Derive dialog on the recipe', await until(() => !!document.querySelector('#deriveDlg')) && (await p.inputValue('#deriveDlg [data-r="name"]')) === 'Power' && !(await visible('#recipesDlg')));
    await p.keyboard.press('Escape');
    check('Esc closes it', !(await visible('#deriveDlg')));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
