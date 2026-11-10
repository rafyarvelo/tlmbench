// Reopening restores the windows of the last run: the first is put back, the others open with their tabs.
import { run } from './lib.js';

const A = '/data/flight.tlm';
const B = '/data/bench run.csv';

run('winrestore', async ({ p, check, until, fake }) => {
  check('the first window is put back where it was', await until(() => !!window.__tlmFake.placed) && (await fake(() => window.__tlmFake.placed.x)) === 40);
  check('another window opens with its tabs and place', await until(() => window.__tlmFake.windows.length === 1));
  const w = await fake(() => window.__tlmFake.windows[0]);
  check('its session and frame are handed over', w.handoff.session.files.join() === B && w.frame.x === 900, JSON.stringify(w));
  const saved = await p.evaluate(() => Object.keys(window.__tlmSettings.get('windows')).join());
  check('the old entry is replaced, not kept', saved === 'main', saved);
}, {
  files: { [A]: {}, [B]: {} },
  settings: { windows: { main: { folder: '', frame: { x: 40, y: 50, w: 1000, h: 700 } }, 'win-7': { folder: '', session: { files: ['/data/bench run.csv'], active: '/data/bench run.csv' }, frame: { x: 900, y: 60, w: 800, h: 600 } } } },
});
