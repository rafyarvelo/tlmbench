// Tabs between windows: dragging a tab out of the strip makes a window of its own where it was let go, or joins the window
// whose strip is there; a tab another window hands over opens here. Letting go over the strip only reorders, and a
// cancelled drag does nothing.
import { run } from './lib.js';

const A = '/data/flight.tlm';
const B = '/data/bench run.csv';
const C = '/data/third.tlm';

run('tabdrag', async ({ p, check, fake, until, wait }) => {
  for (const f of [A, B, C]) {
    await fake((x) => (window.__tlmFake.pick = x), f);
    await p.keyboard.press('ControlOrMeta+o');
    await until((x) => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => n.textContent === x), f.split('/').pop());
  }
  const names = () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent).join());
  const drag = (tab, end) =>
    p.evaluate(
      ([tab, end]) => {
        const el = document.querySelectorAll('#wsTabs .wstab')[tab];
        el.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer: new DataTransfer() }));
        el.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer: new DataTransfer(), ...end }));
      },
      [tab, end],
    );
  check('three tabs', (await names()) === 'flight.tlm,bench run.csv,third.tlm', await names());

  // over the strip, or cancelled: nothing leaves
  await drag(1, { clientX: 300, clientY: 20, screenX: 400, screenY: 100 });
  await drag(1, { clientX: 0, clientY: 400, screenX: 0, screenY: 0 });
  await wait(150);
  check('let go on the strip or cancelled, the tab stays', (await names()) === 'flight.tlm,bench run.csv,third.tlm' && (await fake(() => window.__tlmFake.windows.length)) === 0);

  // dragged out and let go in empty space: a window of its own there
  await drag(1, { clientX: 300, clientY: 400, screenX: 700, screenY: 500 });
  check('a window opens for it', await until(() => window.__tlmFake.windows.length === 1));
  const w = await fake(() => window.__tlmFake.windows[0]);
  check('where it was let go, with that file', w.frame.x === 580 && w.frame.y === 480 && w.handoff.session.files.join() === B, JSON.stringify(w));
  check('and the tab left this strip', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent).join() === 'flight.tlm,third.tlm'));

  // let go over another window's strip: it joins that window
  await fake(() => (window.__tlmFake.windowAt = 'win-2'));
  await drag(1, { clientX: 300, clientY: 400, screenX: 1500, screenY: 40 });
  check('it is handed to that window, no new one', await until(() => window.__tlmFake.sent?.to === 'win-2') && (await fake(() => window.__tlmFake.windows.length)) === 1);
  check('and leaves this strip', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent).join() === 'flight.tlm'));

  // a tab handed over opens here
  await fake((c) => window.__tlmFake.otherWindowTab({ session: { files: [c], active: c } }), C);
  check('a tab from another window opens here', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent).join() === 'flight.tlm,third.tlm'));
}, { files: { [A]: {}, [B]: {}, [C]: {} }, launch: null });
