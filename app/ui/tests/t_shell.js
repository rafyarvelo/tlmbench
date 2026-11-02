// The shell: the rail and the left sidebar, Settings (Basic/All, search, apply and save, reset), the top bar's
// mouse modes and ⋯ menu, Measure docking by hand and by itself, the status bar, and the note for errors.
import { run } from './lib.js';

run('shell', async ({ p, check, text, visible, fake, plotAt, settingsLS, until, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const rows = () => p.evaluate(() => [...document.querySelectorAll('#setPanel .row, #setPanel label.switch')].filter((r) => r.offsetParent).map((r) => r.querySelector('span').childNodes[0].textContent.trim()));
  const bg = () => p.evaluate(() => getComputedStyle(document.body).backgroundColor);

  // rail: Explorer is open; its icon (or Ctrl+B) hides and shows the sidebar
  check('Explorer open at first', (await visible('#chans')) && (await p.getAttribute('[data-rail="explorer"]', 'aria-pressed')) === 'true');
  check('the open file in Explorer', (await text('.exfile')) === 'flight.tlm', await text('.exfile'));
  const w0 = await p.evaluate(() => document.getElementById('center').clientWidth);
  await p.click('[data-rail="explorer"]');
  check('Explorer icon hides the sidebar', !(await visible('#side')) && (await p.evaluate(() => document.getElementById('center').clientWidth)) > w0);
  await p.keyboard.press('ControlOrMeta+b');
  check('Ctrl+B brings it back', await visible('#chans'));

  // Settings in the sidebar: Basic, All, search
  await p.click('#b-settings');
  check('Settings replaces Explorer', (await visible('#setPanel')) && !(await visible('#chans')));
  const basic = await rows();
  check('Basic: the everyday settings', basic.length === 4 && basic.includes('Theme') && !basic.includes('Zoom speed'), basic);
  await p.click('#setPanel [data-sv="all"]');
  check('All settings shows more', (await rows()).length === 7, await rows());
  await p.click('#setPanel [data-sv="basic"]');
  await p.fill('#setPanel input[type=search]', 'wheel');
  check('search finds an All-only setting by its explanation', (await rows()).join() === 'Zoom speed', await rows());
  await p.fill('#setPanel input[type=search]', 'cursors');
  check('search finds choices too', (await rows()).join() === 'Mouse mode,Open Measure when both cursors are placed', await rows());
  await p.fill('#setPanel input[type=search]', 'xyzzy');
  check('no match message', await visible('.setnone'));
  await p.fill('#setPanel input[type=search]', '');
  check('clearing restores Basic', (await rows()).length === basic.length);
  await p.click('#setPanel .info >> nth=0');
  check('ⓘ shows the explanation', /dragging across a chart/.test(await text('#setPanel .hint')), await text('#setPanel .hint'));

  // changes apply and save
  const dark = await bg();
  await p.selectOption('#setPanel [data-set="theme"]', 'light');
  check('theme applies', (await bg()) !== dark && (await p.getAttribute('html', 'data-theme')) === 'light', await bg());
  check('theme saves', (await settingsLS()).theme === 'light');
  await p.selectOption('#setPanel [data-set="mode"]', 'pan');
  check('mouse mode from Settings', (await app('mode')) === 'pan' && (await p.getAttribute('#m-pan', 'aria-pressed')) === 'true');
  await p.click('#m-cursor');
  check('mouse mode from the top bar shows in Settings', (await p.inputValue('#setPanel [data-set="mode"]')) === 'cursor');
  await p.click('#setPanel label.switch:has([data-set="overviewTop"])');
  check('overview above the panes', await until(() => !!document.querySelector('#ovTop .overview')) && (await settingsLS()).overviewTop === true);

  // ⋯ menu: performance details, Esc closes
  await p.click('#b-more');
  check('⋯ lists the rest', /Open recording….*Overview above the panes.*Show performance details.*Settings…/s.test(await text('.moremenu')), await text('.moremenu'));
  check('a ticked setting shows its tick', (await p.getAttribute('.moremenu [role=menuitemcheckbox] >> nth=0', 'aria-checked')) === 'true');
  await p.click('.moremenu >> text=Show performance details');
  check('status bar with the last view', await until(() => /points · (M4|raw) · \d+ ms/.test(document.getElementById('status')?.textContent ?? '')), await text('#status'));
  await p.click('#b-more');
  await p.keyboard.press('Escape');
  check('Esc closes ⋯', !(await visible('.moremenu')));

  // Reset to defaults: settings go, the file's panes and the sidebars stay
  await p.click('#addPane');
  await p.click('#setReset');
  const s = await settingsLS();
  check('reset to defaults', s.theme === 'system' && s.mode === 'box' && !s.overviewTop && !s.showPerf && s.layouts['/data/flight.tlm'], s);
  check('reset applies', (await p.getAttribute('html', 'data-theme')) === null && !(await visible('#statusBar')));
  check('Settings stays open', await visible('#setPanel'));

  // Measure: by hand from the rail (a hint without cursors), by itself with both cursors
  await p.click('[data-rail="measure"]');
  check('Measure by hand shows how to place cursors', (await visible('#measure')) && /press C/.test(await text('#measure')), await text('#measure'));
  await p.click('[data-rail="measure"]');
  check('and closes again', !(await visible('#measure')));
  await p.keyboard.press('c');
  for (const fx of [0.2, 0.5]) {
    const pt = await plotAt(0, fx);
    await p.mouse.click(pt.x, pt.y);
  }
  check('both cursors open it', await until(() => !document.getElementById('measure').hidden));
  await p.click('[data-rail="measure"]');
  check('closed by hand, it stays closed', !(await visible('#measure')));
  const pt = await plotAt(0, 0.7);
  await p.mouse.move(pt.x, pt.y);
  await until(() => window.__tlmApp.hoverT() !== null);
  await p.keyboard.press('b');
  await wait(200);
  check('…while the cursors move', !(await visible('#measure')));
  await p.click('#b-clear');
  check('Clear removes the cursors and its button', (await app('cursors')).A === null && !(await visible('#b-clear')));
  await p.click('#setPanel label.switch:has([data-set="measureAuto"])');
  await p.click('[data-rail="explorer"]');
  for (const fx of [0.2, 0.5]) {
    const q = await plotAt(0, fx);
    await p.mouse.click(q.x, q.y);
  }
  check('with the setting off, Measure waits to be opened', (await app('cursors')).B !== null && !(await visible('#measure')));

  // a file that can't be opened: a note, dismissed with ×
  await fake(() => window.__tlmFake.drop(['/data/missing.tlm']));
  check('a note says why', await until(() => /Couldn't open missing\.tlm/.test(document.getElementById('wsNote')?.textContent ?? '')), await text('#wsNote'));
  check('the file open stays open', (await text('#wsTabs .nm')) === 'flight.tlm' && (await until(() => window.__tlmFake.open.size === 1)));
  await p.click('#wsNote button');
  check('× dismisses it', !(await visible('#wsNote')));

  // the sidebar and settings come back after a reload
  await p.click('[data-rail="explorer"]');
  await p.reload();
  await wait(800);
  check('sidebar state comes back', !(await visible('#side')) && (await settingsLS()).measureAuto === false);
});
