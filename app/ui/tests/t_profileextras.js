// What an import profile carries besides the decode (design: Import profiles): the dialog's "Also for files with this
// header" section picks check sets (D-check-01), recipes (S5.1-10), a starter layout (D-derive-01) and the channel prefix
// rule (D-share-01). They are saved in the profile; the core hands the profile back when the file opens and the page
// uses it: the sets run, the recipes' channels are there, the layout is the starter, a tab of several files is named by
// the rule.
import { run } from './lib.js';

const SET = { schema: 1, name: 'Bench acceptance', runs_on_any_file: false, info_groups: [], check: [{ name: 'Accel in range', kind: 'check', group: 'Motion', expect: 'accel_x < 100', severity: 'fail', grace: 0, settle: 0, enabled: true }] };

run(
  'profileextras',
  async ({ p, check, fake, until, text, paneCount, visible, settingsLS }) => {
    const tabs = () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent));
    await fake((set) => {
      const f = window.__tlmFake;
      f.csv('/data/solo.csv', ['t', 'a', 'b'], { unit: 's', rate: 10 });
      f.csv('/data/alpha_1.csv', ['when', 'p', 'q'], { unit: 's', rate: 10 });
      f.csv('/data/beta_2.csv', ['when', 'p', 'q'], { unit: 's', rate: 10 });
      for (const k of ['solo', 'alpha_1', 'beta_2']) f.files[`/data/${k}.csv`] = {};
      f.checkSets = [set];
      f.recipes.push({ name: 'Power', expr: 'accel_x * accel_y', unit: 'W', category: 'Derived', decimals: 1, automatic: false });
    }, SET);

    // one file: a recipe, a set and a starter layout in the profile
    await fake(() => window.__tlmFake.drop(['/data/solo.csv']));
    check('the dialog asks', await until(() => !!document.querySelector('#importDlg')));
    check('the extras are folded away until wanted', !(await p.$eval('#impMore', (e) => e.open)));
    await p.click('#impMore summary');
    check('the sets and recipes are offered', (await until(() => !!document.querySelector('[data-iset="Bench acceptance"]') && !!document.querySelector('[data-irecipe="Power"]'))));
    await p.check('[data-iset="Bench acceptance"]');
    await p.check('[data-irecipe="Power"]');
    await p.selectOption('#impLayout', 'One chart');
    await p.click('#impGo');
    check('the file opens', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'solo.csv'));
    const prof = await fake(() => window.__tlmFake.saved.at(-1).profile);
    check('the profile holds them', prof.check_sets?.join() === 'Bench acceptance' && prof.recipes?.join() === 'Power' && prof.layout === 'One chart', prof);
    check('its recipe made a derived channel', await until(() => [...document.querySelectorAll('#explorer .row.chan[data-derived] .nm')].some((e) => e.textContent === 'Power')));
    check('it opens with the profile\'s layout, not the starter charts', (await paneCount()) === 1, await paneCount());
    await p.click('[data-rail="checks"]');
    check('the set runs on it without being attached by hand', await until(() => /Bench acceptance/.test(document.querySelector('#checksBody')?.innerText ?? '') && !document.querySelector('[data-cother="Bench acceptance"]')), await text('#checksBody'));
    await p.click('[data-rail="explorer"]');

    // two files in one tab: the prefix rule names them
    await fake(() => window.__tlmFake.drop(['/data/alpha_1.csv', '/data/beta_2.csv']));
    await until(() => !!document.querySelector('#importDlg'));
    await p.selectOption('#impWhere', 'shared');
    await p.click('#impMore summary');
    await p.selectOption('#impPrefix', 'separator');
    check('the dialog shows what the first file would be called', (await text('#impPrefixSample')) === 'alpha', await text('#impPrefixSample'));
    await p.selectOption('#impPrefix', 'pattern');
    await p.fill('#impPrefixText', '_(\\d)');
    check('a pattern takes its first group', (await text('#impPrefixSample')) === '1', await text('#impPrefixSample'));
    await p.selectOption('#impPrefix', 'separator');
    await p.click('#impGo');
    check('the rule is saved with the profile', await until(() => window.__tlmFake.saved.at(-1)?.profile.prefix?.type === 'separator'), await fake(() => window.__tlmFake.saved.at(-1)));
    check('the tab is named by it', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => n.textContent === 'alpha + beta')), await tabs());
    check('and the second file\'s channels read with the prefix', await until(() => /beta · /.test(document.body.innerText)), (await text('#explorer')).slice(0, 300));

    // Derive… on a CSV offers "Files with this import profile" (S5.1-10): the recipe goes into the profile
    if (!(await visible('#explorer'))) await p.click('[data-rail="explorer"]');
    await p.click('#wsTabs .wstab .nm:text-is("solo.csv")');
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'solo.csv');
    await p.click('#explorer .row.chan[data-ch="0"] .nm');
    await p.click('#explorer .row.chan[data-ch="1"] .nm', { modifiers: ['ControlOrMeta'] });
    await p.click('#selBar [data-sel="derive"]');
    await until(() => !!document.querySelector('#deriveDlg'));
    await p.fill('#deriveDlg [data-r="name"]', 'Ratio');
    await p.fill('#deriveDlg [data-r="expr"]', 'accel_x / accel_y');
    await p.click('#deriveDlg input[name="rsc"][value="profile"]');
    await p.click('#deriveDlg [data-ok]');
    check('Derive… with this profile puts the recipe in the profile', await until(() => (window.__tlmFake.saved.at(-1)?.profile.recipes ?? []).includes('Ratio') && window.__tlmFake.saved.at(-1).header.join() === 't,a,b'), await fake(() => window.__tlmFake.saved.at(-1)));

    // the Recipes manager's Import profiles tab (S5.1-12): rename, recipes, the tab in front as its layout, delete
    await p.click('#exMore');
    await p.click('.exmenu [data-ex="recipes"]');
    await p.click('[data-mtab="profiles"]');
    check('the tab lists the profiles by their columns', await until(() => document.querySelectorAll('#profList [data-prof]').length === 2), await text('#recipesDlg'));
    const row = '#profList [data-prof="t,a,b"]';
    await p.fill(`${row} [data-pname]`, 'Bench logs');
    await p.dispatchEvent(`${row} [data-pname]`, 'change');
    check('a profile can be named', await until(() => window.__tlmFake.saved.at(-1)?.profile.name === 'Bench logs'), await fake(() => window.__tlmFake.saved.at(-1)));
    check('its recipe chip is on, as the import dialog left it', (await p.getAttribute(`${row} [data-precipe="Power"]`, 'aria-pressed')) === 'true');
    await p.click(`${row} [data-precipe="Power"]`);
    check('a recipe comes off the profile', await until(() => !(window.__tlmFake.saved.at(-1)?.profile.recipes ?? []).includes('Power') && window.__tlmFake.saved.at(-1)?.header.join() === 't,a,b'), await fake(() => window.__tlmFake.saved.at(-1)));
    check('the layout it opens with is shown', (await p.inputValue(`${row} [data-player]`)) === 'One chart');
    await p.click(`${row} [data-ptab]`);
    check('Use the tab in front keeps its panes as the profile\'s layout, named for it', await until(() => window.__tlmFake.saved.at(-1)?.profile.layout === 'Bench logs layout') && ((await settingsLS()).workspaces ?? []).some((w) => w.name === 'Bench logs layout'), await fake(() => window.__tlmFake.saved.at(-1)));
    await p.click(`${row} [data-pdel]`);
    check('Delete forgets the profile', await until(() => document.querySelectorAll('#profList [data-prof]').length === 1) && !(await fake(() => Object.values(window.__tlmFake.profiles).some((x) => x.header.join() === 't,a,b'))));
  },
  {
    files: { '/data/solo.csv': {}, '/data/alpha_1.csv': {}, '/data/beta_2.csv': {} },
    launch: null,
    settings: { workspaces: [{ schema: 1, name: 'One chart', panes: [{ channels: ['accel_x'] }] }] },
  },
);
