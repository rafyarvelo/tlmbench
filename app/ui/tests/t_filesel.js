// Explorer picks several files (owner, Nov 7): Cmd/Ctrl-click adds or takes one out, Shift-click takes a range; with
// two or more a bar says how many and offers Open (each in a tab of its own; Enter too) and Close (the open ones, with
// Undo); Esc or × clears it.
import { run } from './lib.js';

const DIR = '/data/flights';
const files = {
  [`${DIR}/a/flight.tlm`]: {},
  [`${DIR}/a/hover.csv`]: { channels: ['volts', 'amps'], units: ['V', 'A'] },
  [`${DIR}/b/esc_bench.csv`]: { channels: ['rpm', 'temp'], units: ['rpm', '°C'] },
};

run(
  'filesel',
  async ({ p, check, text, visible, fake, until }) => {
    const row = (n) => `#explorer .row.file[data-path$="${n}"]`;
    const tabs = () => p.$$eval('#wsTabs .wstab .nm', (t) => t.map((x) => x.textContent));
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length === 3);

    await p.click(row('flight.tlm'));
    check('one file picked: no bar', !(await visible('#fileSelBar')));
    await p.click(row('esc_bench.csv'), { modifiers: ['ControlOrMeta'] });
    check('Cmd/Ctrl-click adds a second: the bar says how many', /2 files/i.test((await text('#fileSelBar')) ?? ''), await text('#fileSelBar'));
    check('both rows show as selected', (await p.$$('#explorer .row.file[aria-selected="true"]')).length === 2);
    check('no Close while none is open', !(await visible('#fileSelBar [data-fsel="close"]')));
    await p.click('#fileSelBar [data-fsel="open"]');
    check('Open opens each in a tab of its own', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2), await tabs());
    check('...the last in front', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'esc_bench.csv'));
    check('...and the bar goes', !(await visible('#fileSelBar')));

    // Shift-click: a range, in the order the rows show
    await p.click(row('flight.tlm'));
    await p.click(row('esc_bench.csv'), { modifiers: ['Shift'] });
    check('Shift-click picks the range', /3 files/i.test((await text('#fileSelBar')) ?? ''), await text('#fileSelBar'));
    await p.click(row('hover.csv'), { modifiers: ['ControlOrMeta'] });
    check('Cmd/Ctrl-click takes one out', /2 files/i.test((await text('#fileSelBar')) ?? ''));
    await p.click('#fileSelBar [data-fsel="close"]');
    check('Close closes the open ones', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 0), await tabs());
    check('...with Undo', /Closed 2 files/.test((await text('#wsNote')) ?? ''), await text('#wsNote'));
    await p.click('#wsNote button:has-text("Undo")');
    check('Undo opens them again', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2), await tabs());

    // Enter opens, Esc clears
    await p.click(row('flight.tlm'));
    await p.click(row('hover.csv'), { modifiers: ['ControlOrMeta'] });
    await p.keyboard.press('Escape');
    check('Esc clears the selection', !(await visible('#fileSelBar')));
    await p.click(row('flight.tlm'), { modifiers: ['ControlOrMeta'] });
    check('picked again', await visible('#fileSelBar'));
    await p.keyboard.press('Enter');
    check('Enter opens them', await until(() => (document.querySelectorAll('#wsTabs .wstab').length === 3)), await tabs());
  },
  { files, launch: null },
);
