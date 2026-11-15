// Reset app… and Import profiles in Settings (owner decision 1, sheet A3: Clear cache kept the import profile, so the import
// dialog never came back). Clear cache says what it keeps; Settings lists the import profiles and forgets one; Reset app…
// lists what goes, all ticked, and resets what is ticked: the core removes profiles, recipes, sets, events and alignments
// from its cache, and settings, layouts and widgets go back to the defaults.
import { run } from './lib.js';

run('reset', async ({ p, check, until, fake, settingsLS }) => {
  await fake(() => {
    const key = (h) => [...h].sort().join('\u0000');
    window.__tlmFake.profiles = { [key(['time', 'rpm'])]: { header: ['time', 'rpm'], profile: {} }, [key(['t', 'volts', 'amps'])]: { header: ['t', 'volts', 'amps'], profile: {} } };
  });
  await p.click('[data-rail="settings"]');
  check('Clear cache says it keeps settings, profiles, recipes and sets', /Keeps your settings, import profiles, recipes and sets/.test((await p.getAttribute('#cacheClear', 'data-tip')) ?? ''));
  await p.locator('#profileSet').scrollIntoViewIfNeeded();
  check('Settings lists the import profiles by their columns', await until(() => document.querySelectorAll('#profileSet [data-profile]').length === 2));
  await p.click('#profileSet [data-profile="time,rpm"] [data-profdel]');
  check('one can be forgotten on its own', await until(() => document.querySelectorAll('#profileSet [data-profile]').length === 1) && (await fake(() => Object.keys(window.__tlmFake.profiles).length)) === 1);

  // change some settings, then Reset app…
  await p.selectOption('[data-set="theme"]', 'light').catch(() => {});
  const recentBefore = ((await settingsLS()).recent ?? []).length;
  check('there are recent files to keep', recentBefore > 0, recentBefore);
  await p.click('#setResetApp');
  check('Reset app… opens a dialog', await until(() => !!document.querySelector('#resetDlg')));
  const boxes = await p.$$eval('#resetDlg [data-reset]', (e) => e.map((x) => `${x.dataset.reset}:${x.checked}`));
  check('it lists what goes, all ticked but what the app remembers', boxes.join() === 'settings:true,remembered:false,profiles:true,recipes:true,checks:true,layouts:true,events:true,widgets:true,alignments:true,cache:true', boxes);
  await p.uncheck('#resetDlg [data-reset="widgets"]');
  await p.uncheck('#resetDlg [data-reset="cache"]');
  await p.click('#resetGo');
  check('the core removes what its cache holds of the ticked ones', await until(() => (window.__tlmFake.resets ?? []).length === 1) && (await fake(() => window.__tlmFake.resets[0].join())) === 'profiles,recipes,checks,events,alignments', await fake(() => window.__tlmFake.resets));
  check('what the app remembers stays: the recent files', ((await settingsLS()).recent ?? []).length === recentBefore, (await settingsLS()).recent);
  await p.locator('#profileSet').scrollIntoViewIfNeeded();
  check('the import profiles are gone', await until(() => document.querySelectorAll('#profileSet [data-profile]').length === 0));
  check('settings are back to the defaults', ((await settingsLS()).theme ?? 'dark') !== 'light');
  check('and it says so', await until(() => /^Reset: 7 items/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')));

  // ticked on its own, what the app remembers goes and the settings stay (sheet R3)
  await p.selectOption('[data-set="theme"]', 'light');
  await p.click('#setResetApp');
  for (const b of await p.$$('#resetDlg [data-reset]')) await b.uncheck();
  await p.check('#resetDlg [data-reset="remembered"]');
  await p.click('#resetGo');
  await until(() => !document.querySelector('#resetDlg'));
  const after = await settingsLS();
  check('Things the app remembers clears the recent files, and keeps the settings', (after.recent ?? []).length === 0 && after.theme === 'light', [after.recent, after.theme]);
});
