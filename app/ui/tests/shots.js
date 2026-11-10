// Screenshots for the README and docs: the built UI with the fake core. Usage: node shots.js [outdir] (default docs/images)
import { open } from './lib.js';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const out = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), '../../../docs/images');
const files = { '/data/flight.tlm': { samples: 600000, rate: 1000 } };
const shot = async (p, name) => (await p.screenshot({ path: join(out, name) }), console.log(name));

for (const scheme of ['dark', 'light']) {
  const { b, p, server } = await open({ files, colorScheme: scheme, viewport: { width: 1440, height: 720 }, wait: 1200 });
  const at = (i, f) => p.evaluate(([i, f]) => { const r = document.querySelectorAll('#stack section.pane .plot')[i].getBoundingClientRect(); return { x: r.left + 60 + (r.width - 76) * f, y: r.top + r.height / 2 }; }, [i, f]);
  if (scheme === 'dark') {
    await p.click('#m-cursor');
    for (const f of [0.35, 0.62]) { const c = await at(0, f); await p.mouse.click(c.x, c.y); }
    await p.waitForTimeout(500);
    await shot(p, 'app-cursors.png');
    await p.keyboard.press('Escape');
    await p.click('#m-box');
    await p.click('[data-vmenu]');
    await p.waitForTimeout(300);
    await shot(p, 'app-view-menu.png');
    await p.keyboard.press('Escape');
    await p.click('#b-more');
    await p.waitForTimeout(300);
    await shot(p, 'app-more-menu.png');
    await p.keyboard.press('Escape');
    await p.click('#b-settings');
    await p.waitForTimeout(300);
    await shot(p, 'app-settings.png');
    await p.click('[data-rail="explorer"]');
  }
  await p.mouse.move(700, 700);
  await p.waitForTimeout(300);
  await shot(p, scheme === 'dark' ? 'app.png' : 'app-light.png');
  await b.close();
  server.close();
}

// live: the Connect dialog on file playback, and a live tab with its Sources card
{
  const { b, p, server } = await open({ files, viewport: { width: 1440, height: 760 }, launch: null, wait: 800 });
  await p.click('[data-rail="live"]');
  await p.fill('#lvRecording', '/data/flight.tlm');
  await p.fill('#lvName', 'Bench demo');
  await p.waitForTimeout(500);
  await shot(p, 'app-connect.png');
  await p.click('#lvConnect');
  await p.waitForTimeout(3500);
  await p.click('#lvSrcBtn');
  await p.mouse.move(5, 740);
  await p.waitForTimeout(800);
  await shot(p, 'app-live.png');
  await b.close();
  server.close();
}

// several files in one tab: slots in Explorer and file 2's Alignment section after a cross-correlation
{
  const gps = { channels: ['lat', 'lon', 'accel_x'], units: ['deg', 'deg', 'g'], samples: 60000, rate: 100 };
  const two = { '/data/runs/flight.tlm': { samples: 600000, rate: 1000 }, '/data/runs/gps.tlm': gps };
  const { b, p, server } = await open({ files: two, launch: null, viewport: { width: 1440, height: 760 }, wait: 800 });
  await p.evaluate(() => (window.__tlmFake.pickFolder = '/data/runs'));
  await p.click('#exFolder');
  await p.dblclick('#explorer .row.file[data-path$="flight.tlm"]');
  await p.waitForTimeout(800);
  await p.click('#explorer .row.file[data-path$="gps.tlm"]');
  await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
  await p.waitForTimeout(800);
  await p.click('#explorer .row.chan[data-ch="6"] .box');
  await p.evaluate(() => (window.__tlmFake.alignResults.xcorr = { knots: [['0', '-3330000000']], residuals: [['1000000000', 420000], ['9000000000', -310000], ['17000000000', 150000], ['25000000000', -90000]], quality: 0.97, note: 'Matched on accel_x: −3.33 s, r = 0.97', channel: 'accel_x' }));
  await p.click('#explorer .row.slot[data-slot="2"]');
  await p.selectOption('[data-falign="2"]', 'xcorr');
  await p.waitForTimeout(900);
  await p.mouse.move(5, 740);
  await shot(p, 'app-slots.png');
  await b.close();
  server.close();
}
