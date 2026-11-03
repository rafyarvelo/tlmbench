// The sidebars: dragging an inner edge to resize (200 px to half the window, double-click resets, saved), collapsing
// Explorer and Measure's panels (saved), Collapse all in ⋯, ⇄ trading Explorer's and Measure's sides, Reset UI layout.
import { run } from './lib.js';

run('sidebars', async ({ p, check, visible, plotAt, settingsLS, until, wait }) => {
  const width = (id) => p.evaluate((id) => document.getElementById(id).getBoundingClientRect().width, id);
  const left = (id) => p.evaluate((id) => Math.round(document.getElementById(id).getBoundingClientRect().left), id);
  const drag = async (sel, dx) => {
    const r = await p.evaluate((s) => { const b = document.querySelector(s).getBoundingClientRect(); return { x: b.left + b.width / 2, y: b.top + 200 }; }, sel);
    await p.mouse.move(r.x, r.y);
    await p.mouse.down();
    await p.mouse.move(r.x + dx, r.y, { steps: 5 });
    await p.mouse.up();
  };

  check('Explorer is 300 px at first', Math.round(await width('side')) === 300, await width('side'));
  await drag('[data-grip="left"]', 100);
  check('dragging its edge widens it, saved', Math.abs((await width('side')) - 400) <= 2 && Math.abs((await settingsLS()).leftWidth - 400) <= 2, await width('side'));
  await drag('[data-grip="left"]', -500);
  check('not under 200 px', Math.round(await width('side')) === 200, await width('side'));
  await drag('[data-grip="left"]', 1500);
  check('not past half the window', (await width('side')) <= 720 && (await width('side')) > 600, await width('side'));
  await p.dblclick('[data-grip="left"]');
  check('double-click resets it', Math.round(await width('side')) === 300, await width('side'));

  // Measure opens by hand and resizes from its left edge
  await p.click('[data-rail="measure"]');
  check('Measure is 260 px', Math.round(await width('measure')) === 260, await width('measure'));
  await drag('[data-grip="measure"]', -60);
  check('dragging left widens Measure', Math.abs((await width('measure')) - 320) <= 2, await width('measure'));

  // collapse panels
  await p.click('#exToggle');
  check('Explorer collapses to its title, saved', !(await visible('#chans')) && (await visible('#exToggle')) && (await settingsLS()).panelsClosed.includes('explorer'));
  await p.click('#exToggle');
  check('and expands again', await visible('#chans'));
  await p.click('#measure details[data-panel="stats"] > summary');
  await wait(200);
  check('a Measure panel collapses, saved', (await settingsLS()).panelsClosed.join() === 'measure.stats' && !(await p.evaluate(() => document.querySelector('#measure details[data-panel="stats"]').open)));
  await p.click('#b-more');
  await p.click('[data-more="panels"]');
  check('Collapse all closes every panel', (await settingsLS()).panelsClosed.length === 3 && !(await visible('#chans')));
  await p.click('#b-more');
  check('the item then offers to expand', /Expand all sidebar panels/.test(await p.innerText('.moremenu')));
  await p.click('[data-more="panels"]');
  check('Expand all opens them', (await settingsLS()).panelsClosed.length === 0 && (await visible('#chans')));

  // swap sides
  const exL = await left('side'), meL = await left('measure');
  await p.click('#swapSides');
  await wait(200);
  check('⇄ puts Explorer on the right and Measure on the left', (await left('side')) > exL + 300 && (await left('measure')) < meL - 300, [await left('side'), await left('measure')]);
  await drag('[data-grip="left"]', -80);
  check('resizing follows the side', Math.abs((await width('side')) - 380) <= 2, await width('side'));

  // everything comes back after a reload, and Reset UI layout undoes it
  await p.reload();
  await wait(800);
  check('sizes and sides come back', Math.abs((await width('side')) - 380) <= 2 && (await settingsLS()).sidesSwapped === true);
  await p.click('#b-settings');
  await p.click('#setResetUi');
  await p.click('[data-rail="explorer"]');
  check('Reset UI layout restores sizes and sides', Math.round(await width('side')) === 300 && Math.round(await width('measure')) === 260 && (await settingsLS()).sidesSwapped === false, [await width('side'), await width('measure')]);
});
