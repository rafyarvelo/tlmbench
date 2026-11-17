// Packet types in the app (S4.3-08): a packet file whose descriptor has [[packet_types]] opens one recording per type,
// as the design says, in the same tab on the same clock: the first type is slot 1, the others slots 2, 3, … named
// `flight.bin · gps`. Opening the file again fronts the tab; closing the file closes it.
import { run } from './lib.js';

const DIR = '/data/logs';
const BIN = `${DIR}/flight.bin`;

run(
  'packets',
  async ({ p, check, fake, until, text }) => {
    const tabs = () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent));
    const front = () => p.evaluate(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent);
    const chans = () => p.$$eval('#explorer .row.chan .nm', (r) => r.map((x) => x.textContent));
    await fake(([bin]) => {
      const f = window.__tlmFake;
      f.bin(bin, {
        spec: { channels: ['acc'], samples: 1000, rate: 100, parts: { gps: { channels: ['lat', 'lon'], samples: 500, rate: 10 }, imu: { channels: ['gx', 'gy', 'gz'], samples: 2000, rate: 200 } } },
      });
      f.decoders.flight = { schema: 1, name: 'Flight', files: ['flight*.bin'], endian: 'little', header_size: 0, record_size: 0, time: { source: 'index' }, fields: [], packet_types: [{ name: 'gps' }, { name: 'imu' }], packets: { header_size: 4, checksum: 'none', compression: 'none' } };
      f.pickFolder = '/data/logs';
    }, [BIN]);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length === 1);
    check('a file whose descriptor names it is listed, with no descriptor badge', !/no descriptor/.test(await text('#explorer .row.file')), await text('#explorer .row.file'));

    await p.dblclick('#explorer .row.file[data-path$="flight.bin"]');
    check('opening it opens one tab for the file', await until(() => document.querySelector('#wsTabs .wstab .nm')?.textContent === 'flight.bin' && document.querySelectorAll('#explorer .row.slot').length === 2), await tabs());
    check('only that tab', (await tabs()).join() === 'flight.bin', await tabs());
    const tip = await p.$eval('#wsTabs .wstab', (t) => t.getAttribute('data-tip') || t.getAttribute('title') || '');
    check("its tooltip lists the types, file · type", tip.includes('flight.bin · gps') && tip.includes('flight.bin · imu'), tip);
    const slots = await p.$$eval('#explorer .row.slot .nm', (r) => r.map((x) => x.textContent));
    check("a slot per type, in the descriptor's order", slots.join() === 'flight.bin · gps,flight.bin · imu', slots);
    check('the file row says how many types', /2 packet types/.test(await text('#explorer .row.file [data-fslotmeta]')), await text('#explorer .row.file'));
    check('the second type is on the same clock', /same clock|clock/i.test(await text('#explorer .row.slot[data-slot="2"] [data-asum]')), await text('#explorer .row.slot[data-slot="2"]'));
    check('each type keeps its channels, the second numbered as slot 2', (await chans()).join() === 'lat,lon,gx,gy,gz', await chans());
    const opened = await fake(() => window.__tlmFake.opened);
    check('each opened by its type, after the file alone was refused', opened.map((o) => o.part).join() === ',gps,imu', opened);
    const sets = await fake(() => window.__tlmFake.groupSets);
    check('the two recordings are grouped', sets.at(-1)?.length === 2, sets);
    const aligns = await fake(() => window.__tlmFake.aligns.length);
    check('nothing is estimated: one file, one clock', aligns === 0, aligns);
    const panes = await p.evaluate(() => document.querySelectorAll('#stack .pane').length);
    check('a first layout draws both types', panes >= 2, panes);

    // opening the file again only brings the tab forward
    await p.dblclick('#explorer .row.file[data-path$="flight.bin"]');
    await p.waitForTimeout(300);
    check('opening it again opens nothing new', (await tabs()).length === 1 && (await fake(() => window.__tlmFake.opened.length)) === 3, await tabs());

    // the session brings the types back in their slots
    await until(() => window.__tlmSettings.get('sessions')?.['/data/logs']?.groups);
    const session = await p.evaluate(() => window.__tlmSettings.get('sessions')?.['/data/logs']);
    check('the session keeps the second type with its part', session.groups?.[BIN]?.[0]?.part === 'imu', session.groups);

    // closing the file closes the tab
    await p.click('#explorer .row.file[data-path$="flight.bin"]', { button: 'right' });
    await p.click('.filemenu [data-fm="close"]');
    check('Close this file closes it', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 0), await tabs());
  },
  { files: {}, launch: null },
);
