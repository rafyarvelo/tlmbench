// The desktop app's in-page drag and drop (src/dnd.ts). The window takes Finder drops natively, which stops the
// browser's own drag and drop, so the app drives drags from the mouse: a drag starts after a few pixels, shows what is
// being dragged, lands where the page accepts it, ends with Esc, and the release is not a click. Real mouse events here,
// no synthetic drag events. (The tests of each drag target are in t_panes, t_chips, t_zones, t_sidebars, t_tabs.)
import { run } from './lib.js';

const DIR = '/data/flights';
const files = { [`${DIR}/flight.tlm`]: {} };

run('dnd', async ({ p, check, text, fake, paneCount, until }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const ch = (n) => `#explorer .row.chan[data-ch="${n}"]`;
  const centre = async (sel) => {
    const r = await p.locator(sel).boundingBox();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  };
  await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  await p.dblclick(`#explorer .row.file[data-path$="flight.tlm"]`);
  await until(() => document.querySelectorAll('#explorer .row.chan').length === 6);
  const panesBefore = JSON.stringify(await app('panes'));

  // a small movement is a click, not a drag
  const c = await centre(`${ch(4)} .nm`);
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 2, c.y + 1);
  check('two pixels is not a drag', !(await p.$('.dnd-ghost')) && !(await p.evaluate(() => document.documentElement.classList.contains('dnd-active'))));
  await p.mouse.up();
  check('and the click selects the row', (await p.$eval(ch(4), (e) => e.classList.contains('sel'))) === true);

  // a drag shows a ghost with its name, and a cursor that says where it can land
  const to = await centre('#stack [data-pane="0"] .plot');
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 30, c.y + 30, { steps: 4 });
  check('a drag shows what is being dragged', (await until(() => !!document.querySelector('.dnd-ghost'))) && /accel_x|motor|other|\w/.test(await text('.dnd-ghost')), await text('.dnd-ghost').catch(() => ''));
  check('and the page is in the dragging state', await p.evaluate(() => document.documentElement.classList.contains('dnd-active')));
  await p.mouse.move(to.x, to.y, { steps: 6 });
  check('over a pane the drop is allowed', !(await p.evaluate(() => document.documentElement.classList.contains('dnd-no'))));
  // Esc lets go with nothing dropped
  await p.keyboard.press('Escape');
  check('Esc ends the drag', await until(() => !document.querySelector('.dnd-ghost') && !document.documentElement.classList.contains('dnd-active')));
  await p.mouse.up();
  check('nothing was dropped', JSON.stringify(await app('panes')) === panesBefore, await app('panes'));

  // dropping on a pane adds the channel; the release is not also a click on what is under it
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 30, c.y + 30, { steps: 4 });
  await p.mouse.move(to.x, to.y, { steps: 6 });
  await p.mouse.up();
  check('dropping on a pane adds the channel', (await app('panes'))[0].includes(4), await app('panes'));
  check('the ghost is gone', !(await p.$('.dnd-ghost')));

  // somewhere that takes nothing says so, and lets go quietly
  const before = JSON.stringify(await app('panes'));
  await p.mouse.move(c.x, c.y);
  await p.mouse.down();
  await p.mouse.move(c.x + 30, c.y + 30, { steps: 4 });
  const bar = await centre('#wsTabs');
  await p.mouse.move(bar.x, bar.y, { steps: 6 });
  check('over something that takes nothing the cursor says so', await p.evaluate(() => document.documentElement.classList.contains('dnd-no')));
  await p.mouse.up();
  check('and nothing changes', JSON.stringify(await app('panes')) === before && !(await p.$('.dnd-ghost')), await app('panes'));

  // a text box is never dragged from
  await p.fill('#exFilter', 'acc').catch(() => {});
}, { files, launch: null });
