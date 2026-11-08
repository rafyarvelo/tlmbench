// Live: the Connect dialog (TCP, UDP, a growing file; a descriptor for sockets), a live tab following now (LIVE,
// REC time, window presets, no overview), a view you choose holding it (scrubbing back, Jump to live), the Sources
// panel's stream health, Stop, switching tabs, and closing.
import { run } from './lib.js';

const TCP = 'tcp://127.0.0.1:9000';
run('live', async ({ p, check, text, visible, fake, plotAt, until, settingsLS, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const span = (v) => Number(BigInt(v.t1) - BigInt(v.t0)) / 1e9;
  await fake((e) => (window.__tlmFake.sources[e] = { channels: ['rpm', 'temp', 'volts'], rate: 1000, already: 40 }), TCP);

  await p.click('[data-rail="live"]');
  check('Live opens the Connect dialog', await p.isVisible('#liveDlg'));
  check('TCP needs a descriptor', await p.isDisabled('#lvConnect'));
  check('laid out as the prototype\'s: the kinds on the left, each with what it\'s for', (await p.$$eval('#liveDlg .lvkind', (b) => b.map((x) => x.querySelector('b').textContent))).join() === 'Serial port,UDP,TCP,Tail a file' && (await p.getAttribute('#liveDlg [data-kind="tcp"]', 'aria-pressed')) === 'true');
  check('...over a scrim, with a × to close', (await p.isVisible('.scrim')) && (await p.isVisible('#liveDlg .dclose')));
  await p.click('#liveDlg [data-kind="serial"]');
  await p.fill('#lvPort', '/dev/tty.usbserial-1');
  await p.fill('#lvDesc', '/data/frames.toml');
  await p.selectOption('#lvBaud', '921600');
  await p.click('#lvConnect');
  check('a serial port, with its baud rate', await until(() => /serial:\/\/\/dev\/tty\.usbserial-1\?baud=921600/.test(document.getElementById('lvErr')?.textContent ?? '')), await text('#lvErr'));
  await p.click('#liveDlg [data-kind="tcp"]');
  await p.fill('#lvAddr', '127.0.0.1:9999');
  await p.fill('#lvDesc', '/data/frames.toml');
  await p.click('#lvConnect');
  check('a refused connection says so', await until(() => /connection refused/.test(document.getElementById('lvErr')?.textContent ?? '')), await text('#lvErr'));
  await p.fill('#lvAddr', '127.0.0.1:9000');
  await p.click('#lvConnect');
  check('connecting opens a live tab', await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === 'tcp://127.0.0.1:9000') && !(await p.isVisible('#liveDlg')));
  check('a pulsing dot on the live tab and on the rail', (await p.$$('[data-live-tab]')).length === 1 && (await p.$$('[data-live-dot]')).length === 1);
  check('the live strip: LIVE and the time recorded', (await text('#lvBadge')) === 'LIVE' && /REC 4\d\.\d+ s/.test(await text('#lvRec')), await text('#liveStrip'));
  check('no overview in a live tab', !(await p.$('.overview')));
  check('every channel drawn', await until(() => window.__tlmApp.drawn().every((d) => d && d.length)));

  // following now: the view's end moves with the recording, over the window
  const v1 = await app('view');
  await wait(600);
  const v2 = await app('view');
  check('the view follows now', BigInt(v2.t1) > BigInt(v1.t1) && Math.abs(span(v2) - 30) < 0.01, [v1, v2]);
  await p.click('#liveStrip [data-lwin="10"]');
  check('a window preset', Math.abs(span(await app('view')) - 10) < 0.01 && (await p.getAttribute('#liveStrip [data-lwin="10"]', 'aria-pressed')) === 'true');

  // a view you choose holds; scrub back; Jump to live
  const c = await plotAt(0);
  await p.mouse.move(c.x, c.y);
  await p.mouse.wheel(0, -300);
  await wait(300);
  check('zooming holds the view', (await text('#lvFollow')) === 'Jump to live');
  const held = await app('view');
  await wait(400);
  check('…while recording goes on', JSON.stringify(await app('view')) === JSON.stringify(held) && /REC/.test(await text('#lvRec')));
  await p.evaluate(() => { const r = document.getElementById('lvScrub'); r.value = '500'; r.dispatchEvent(new Event('input', { bubbles: true })); });
  check('scrubbing back says how far', /^−\d+\.\d+ s/.test(await text('#lvAt')), await text('#lvAt'));
  await p.click('#lvFollow');
  check('Jump to live follows again', (await text('#lvFollow')) === 'Following' && (await until(() => !document.getElementById('lvAt'))));

  // Sources: stream health and the recording; Stop
  await p.click('#lvSrcBtn');
  check('Sources shows the stream health', /1,000 records\/s|99\d records\/s/.test(await text('#lvRate')) && (await text('#lvDrop')) === '0' && (await text('#lvFile')) === 'source-1.tlm', await text('#lvSources'));
  await p.click('#lvStop');
  check('Stop ends the recording', await until(() => document.getElementById('lvBadge')?.textContent === 'ENDED') && !(await visible('#lvStop')));

  // another tab hides the strip; coming back, it is there
  await fake(() => window.__tlmFake.drop(['/data/flight.tlm']));
  await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === '/data/flight.tlm');
  check('a file tab has no live strip', !(await visible('#liveStrip')) && !!(await p.$('.overview')));
  await p.click('#wsTabs .wstab[data-tip="tcp://127.0.0.1:9000"]');
  check('back in the live tab', await until(() => !!document.getElementById('liveStrip')) && (await text('#lvBadge')) === 'ENDED');
  check('live tabs are not reopened at launch', !(await settingsLS()).sessions[''].files.includes(TCP), (await settingsLS()).sessions);
  await p.hover('#wsTabs .wstab[data-tip="tcp://127.0.0.1:9000"]');
  await p.click('#wsTabs .wstab[data-tip="tcp://127.0.0.1:9000"] .tx');
  check('closing the tab closes the source', await until((e) => ![...window.__tlmFake.open.values()].some((o) => o.r.path === e), TCP));
}, { files: { '/data/flight.tlm': {} }, launch: null });
