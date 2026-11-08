// The sidebars: dragging an inner edge to resize (200 px to half the window, double-click resets, saved), collapsing
// Explorer and Measure's panels (saved), Collapse all in ⋯, panels moving between the sidebars (⇄, or dragged by their
// title to reorder or change sides; owner, Nov 7), all saved, and Reset UI layout.
import { run } from './lib.js';

run('sidebars', async ({ p, check, visible, plotAt, settingsLS, until, wait }) => {
  const width = (id) => p.evaluate((id) => document.getElementById(id).getBoundingClientRect().width, id);
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
  await p.click('#measure section[data-panel="stats"] .phtog');
  await wait(200);
  check('a Measure panel collapses, saved', (await settingsLS()).panelsClosed.join() === 'measure.stats' && !(await visible('#mpStats')));
  await p.click('#b-more');
  await p.click('[data-more="panels"]');
  check('Collapse all closes every panel', (await settingsLS()).panelsClosed.length === 4 && !(await visible('#chans')));
  await p.click('#b-more');
  check('the item then offers to expand', /Expand all sidebar panels/.test(await p.innerText('.moremenu')));
  await p.click('[data-more="panels"]');
  check('Expand all opens them', (await settingsLS()).panelsClosed.length === 0 && (await visible('#chans')));

  // ⇄ moves a panel to the other sidebar: Explorer joins Measure on the right
  const order = async () => (await settingsLS()).panelLayout;
  await p.hover('#exToggle');
  await p.click('#swapSides');
  await wait(200);
  check('⇄ moves Explorer to the right sidebar, under Measure\'s panels', (await visible('#measure [data-panel="explorer"] #chans')) && (await order()).right.join() === 'cur,stats,explorer', await order());
  check('the left sidebar then shows what is left there (Settings)', await visible('#side #setPanel'));
  // drag a title: Explorer above Cursors
  await p.dragAndDrop('#measure [data-panel="explorer"] .phd', '#measure section[data-panel="cur"] .phd', { targetPosition: { x: 20, y: 2 } });
  check('dragging a title reorders the panels', (await order()).right.join() === 'explorer,cur,stats', await order());
  // and Stats to the left sidebar
  await p.dragAndDrop('#measure section[data-panel="stats"] .phd', '#side', { targetPosition: { x: 100, y: 400 } });
  check('...or moves one to the other sidebar, shown there', (await order()).left.includes('stats') && (await order()).right.join() === 'explorer,cur' && (await settingsLS()).leftPanel === 'measure' && (await visible('#side #mpStats')), await order());
  check('the rail\'s Measure button shows it there', (await p.getAttribute('[data-rail="measure"]', 'aria-pressed')) === 'true');
  await drag('[data-grip="measure"]', -40);
  check('resizing follows the side', Math.abs((await width('measure')) - 360) <= 2, await width('measure'));

  // everything comes back after a reload, and Reset UI layout undoes it
  await p.reload();
  await wait(800);
  check('sizes and panels come back', Math.abs((await width('measure')) - 360) <= 2 && (await order()).left.includes('stats') && (await visible('#measure [data-panel="explorer"]')), await order());
  await p.click('#b-settings');
  await p.click('#setResetUi');
  await p.click('[data-rail="explorer"]');
  check('Reset UI layout restores sizes and panels', Math.round(await width('side')) === 300 && Math.round(await width('measure')) === 260 && (await order()).left.join() === 'explorer,settings' && (await order()).right.join() === 'cur,stats', [await width('side'), await width('measure'), await order()]);
});
