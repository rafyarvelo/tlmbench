// Selecting table columns: click one, Shift-click a range (header or checkbox), Ctrl/Cmd-click to add; with click-to-sort on, Shift-click extends a selection.
const T = require('./lib');
T.run('colsel', async ({ p, check, wait, text }) => {
  const hdr = n => p.locator(`#stack .tth[data-cid="${n}"]`).first();
  const ids = await p.evaluate(() => [...document.querySelectorAll('#stack .tth[data-cid]')].map(h => h.dataset.cid).filter(id => !['n', 't', 'ev'].includes(id)));
  const n = async () => +((/(\d+) selected/.exec(await text('#stack .tselbar') || '') || [0, 0])[1]);
  await hdr(ids[0]).click({ position: { x: 30, y: 10 } }); await wait(200);
  await hdr(ids[3]).click({ position: { x: 30, y: 10 }, modifiers: ['Shift'] }); await wait(200);
  check('Shift-click on a header selects the range', (await n()) === 4, await n());
  await hdr(ids[5]).locator('.tck').click({ modifiers: ['Shift'] }); await wait(200);
  check('Shift-click on a checkbox extends it', (await n()) === 6, await n());
  await hdr(ids[8]).click({ position: { x: 30, y: 10 }, modifiers: ['ControlOrMeta'] }); await wait(200);
  check('Ctrl-click adds one', (await n()) === 7, await n());
}, { settings: { fileOpenNew: 'table', fileOpen: 'table' }, wait: 5000 });
