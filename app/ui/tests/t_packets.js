// Packet types in the app: a packet file whose descriptor has [[packet_types]] opens one recording per type, each in
// a tab of its own (`flight.bin · gps`) until step 5.6 puts several recordings in one tab. The tab's identity is the
// file plus the type: opening the file again fronts them, closing the file closes them all.
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
    check('opening it opens a tab per packet type', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].length === 2), await tabs());
    check('named file · type, in the descriptor\'s order', (await tabs()).join() === 'flight.bin · gps,flight.bin · imu', await tabs());
    check('the first type is in front', (await front()) === 'flight.bin · gps', await front());
    check('each recording has its own channels', (await chans()).join() === 'lat,lon', await chans());
    const opened = await fake(() => window.__tlmFake.opened);
    check('each opened by its type, after the file alone was refused', opened.map((o) => o.part).join() === ',gps,imu', opened);

    await p.click('#wsTabs .wstab:nth-child(2)');
    check('the other tab shows the other type', (await until(() => document.querySelector('#explorer .row.chan .nm')?.textContent === 'gx')) && (await chans()).join() === 'gx,gy,gz', await chans());

    // opening the file again only brings a tab forward
    await p.dblclick('#explorer .row.file[data-path$="flight.bin"]');
    await p.waitForTimeout(300);
    check('opening it again opens nothing new', (await tabs()).length === 2 && (await fake(() => window.__tlmFake.opened.length)) === 3, await tabs());

    // closing the file closes every type
    await p.click('#explorer .row.file[data-path$="flight.bin"]', { button: 'right' });
    await p.click('.filemenu [data-fm="close"]');
    check('Close this file closes them all', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 0), await tabs());
  },
  { files: {}, launch: null },
);
