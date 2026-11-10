// A tab with several files comes back with the session: the others in slot order, each with the alignment it had
// (its knots reproduce it exactly), a locked file still locked.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { channels: ['lat', 'lon'], units: ['deg', 'deg'], samples: 20000, rate: 100 } };

run(
  'slotsession',
  async ({ p, check, fake, until, settingsLS }) => {
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    await p.click('#explorer .row.slot[data-slot="2"]');
    await p.selectOption('[data-falign="2"]', 'manual');
    await p.fill('[data-foff="2"]', '1.5');
    await p.press('[data-foff="2"]', 'Tab');
    await until(() => window.__tlmFake.aligns.at(-1)?.method.seconds === 1.5);
    await p.check('[data-lock="2"]');
    await p.waitForTimeout(600);
    const g = (await settingsLS()).sessions?.[DIR]?.groups?.[A];
    check('the session lists the second file with its alignment', g?.length === 1 && g[0].path === B && g[0].method === 'manual' && g[0].locked === true && g[0].knots.length === 1, g);

    await p.reload();
    check('the tab comes back with two slots', await until(() => document.querySelectorAll('#explorer .row.slot').length === 2, null, 8000));
    const back = (await fake(() => window.__tlmFake.groupMaps.at(-1) ?? null)) ?? null;
    check('the saved map is applied, nothing re-measured', (await fake(() => window.__tlmFake.aligns.length)) === 0);
    await p.click('#explorer .row.slot[data-slot="2"]');
    check('its alignment is as it was, locked', (await p.isChecked('[data-lock="2"]')) && (await p.inputValue('[data-falign="2"]')) === 'manual' && Math.abs(+(await p.inputValue('[data-foff="2"]')) - 1.5) < 0.01, [await p.inputValue('[data-foff="2"]'), back]);
  },
  { files, launch: null },
);
