// Explorer: select-first rows, multi-select and its bar, title-row icons, filter, categories, expand/collapse, ⋯ visible, drag to a new pane.
const T = require('./lib');
T.run('explorer', async ({ p, check, paneCount, text, expandTree, menu, wait, settingsLS }) => {
  const chans = () => p.evaluate(() => document.querySelectorAll('.chan').length);
  check('⋯ is visible', (await p.evaluate(() => getComputedStyle(document.getElementById('exMore')).opacity)) === '1');
  check('title-row icons', (await p.evaluate(() => ['exAddShown', 'exDerive', 'exCats', 'exExpand', 'exCollapse'].every(id => !!document.getElementById(id)))));

  // select first: a click selects, it doesn't plot
  await expandTree();
  const n0 = await paneCount();
  const plotted = () => p.evaluate(() => document.querySelectorAll('.chan.on').length);
  const on0 = await plotted();
  await p.click('.chan[data-ch="gyro_z"] .nm'); await wait(200);
  check('click selects a channel', await p.evaluate(() => document.querySelector('.chan[data-ch="gyro_z"]').classList.contains('rsel')));
  check('...without plotting it', (await plotted()) === on0);
  await p.click('.chan[data-ch="yaw"] .nm', { modifiers: ['ControlOrMeta'] }); await wait(150);
  check('Ctrl-click adds to the selection', /2 channels/.test(await text('#exSelBar')), await text('#exSelBar'));
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => /^Chart$/.test(b.textContent)).click()); await wait(800);
  check('selection → new chart', (await paneCount()) === n0 + 1);
  await p.keyboard.press('Escape'); await wait(100);
  check('Esc clears the selection', !(await p.isVisible('#exSelBar')));

  // filter + Add the channels shown
  await p.fill('#fileFilter', 'temp'); await wait(400);
  check('filter narrows the tree', (await chans()) > 0 && (await chans()) < 12, await chans());
  check('+ says how many', /Add the \d+ matching channels/.test(await p.getAttribute('#exAddShown', 'data-tip')));
  await p.fill('#fileFilter', ''); await wait(300);

  // categories toggle (icon) mirrors the setting
  await p.click('#exCats'); await wait(300);
  check('flat list', (await p.evaluate(() => document.querySelectorAll('.ghead[data-group]').length)) === 0);
  check('setting follows', (await settingsLS()).groupChannels === false);
  await p.click('#exCats'); await wait(300);
  check('categories back', (await p.evaluate(() => document.querySelectorAll('.ghead[data-group]').length)) > 0);

  // expand / collapse all
  await p.click('#exCollapse'); await wait(300);
  check('collapse all', (await chans()) === 0, await chans());
  await p.click('#exExpand'); await wait(300);
  check('expand all', (await chans()) >= 52, await chans());

  // ⋯ keeps the rarer actions
  await p.click('#exMore'); await wait(200);
  const m = await menu();
  check('⋯ menu', m.some(x => /new chart/.test(x)) && m.some(x => /Recipes and profiles/.test(x)) && !m.some(x => /Show categories/.test(x)), m);
  await p.keyboard.press('Escape');

  // a selected row keeps its whole name; its buttons appear on hover
  await p.click('.chan[data-ch="accel_z"] .nm'); await p.mouse.move(900, 600); await wait(200);
  check('selected row: buttons hidden until hover', await p.evaluate(() => { const h = document.querySelector('.chan[data-ch="accel_z"] .hov'); return getComputedStyle(h).opacity === '0' || getComputedStyle(h).visibility === 'hidden' || getComputedStyle(h).display === 'none'; }));
  // drag a channel to a pane's right edge → a new pane beside it
  const n1 = await paneCount();
  const b1 = await p.locator('#stack section.pane').first().boundingBox();
  await p.dragAndDrop('.chan[data-ch="esc_temp_3"] .nm', '#stack section.pane >> nth=0', { targetPosition: { x: b1.width - 10, y: 80 } });
  await wait(600);
  check('drop on an edge makes a pane', (await paneCount()) === n1 + 1, [n1, await paneCount()]);

});
