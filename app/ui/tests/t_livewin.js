// A live source shown in two windows: Show in new window hands the new window the source (held from the start), and
// stopping it where another window shows it asks first.
import { run } from './lib.js';

const TCP = 'tcp://127.0.0.1:9000';

run('livewin', async ({ p, check, text, fake, until }) => {
  await fake((e) => (window.__tlmFake.sources[e] = { channels: ['rpm', 'temp', 'volts'], rate: 1000, already: 40 }), TCP);
  await p.click('[data-rail="live"]');
  await p.click('#liveDlg [data-kind="tcp"]');
  await p.fill('#lvAddr', '127.0.0.1:9000');
  await p.fill('#lvDesc', '/data/frames.toml');
  await p.click('#lvConnect');
  await until(() => document.getElementById('lvBadge')?.textContent === 'LIVE');

  await p.click('#wsTabs .wstab[aria-selected=true]', { button: 'right' });
  await p.click('[data-ws=showwin]');
  check('a window opens', await until(() => window.__tlmFake.windows.length === 1));
  const w = await fake(() => window.__tlmFake.windows[0]);
  check('it is handed the source and holds it from the start', w.handoff.live?.endpoint === TCP && w.hold?.length === 1 && w.hold[0] === w.handoff.live.id, JSON.stringify(w));
  check('the tab stays here', (await p.$$('[data-live-tab]')).length === 1);

  // stopping asks, because the other window shows it too
  await fake(() => {
    window.__tlmFake.holdersOf = ['win-2'];
    window.__tlmFake.answer = false;
  });
  await p.click('#lvSrcBtn');
  await p.click('#lvStop');
  await until(() => !!window.__tlmFake.asked);
  check('Stop asks first', /another window/.test(await fake(() => window.__tlmFake.asked)), await fake(() => window.__tlmFake.asked));
  check('keeping it running leaves it live', (await text('#lvBadge')) === 'LIVE');
  await fake(() => {
    window.__tlmFake.answer = true;
  });
  if (!(await p.isVisible('#lvStop'))) await p.click('#lvSrcBtn');
  await p.click('#lvStop');
  check('stopping anyway ends it', await until(() => document.getElementById('lvBadge')?.textContent === 'STOPPED'));
}, { files: { '/data/flight.tlm': {} }, launch: null });
