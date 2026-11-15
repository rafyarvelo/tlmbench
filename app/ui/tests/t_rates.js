// Rates read as people say them (D-ui-38): the import dialog's "As recorded" line, the table's status line and the
// table's filter placeholders.
import { run } from './lib.js';

run(
  'rates',
  async ({ p, check, fake, until, text }) => {
    await fake(() => {
      const f = window.__tlmFake;
      f.csv('/data/daily.csv', ['t', 'x'], { unit: 's', rate: 1 / 86400 });
      f.csv('/data/fast.csv', ['t', 'x'], { unit: 's', rate: 2500 });
      f.csv('/data/slow.csv', ['t', 'x'], { unit: 's', rate: 0.4 });
    });
    const asRecorded = async (path) => {
      await fake((a) => window.__tlmFake.drop([a]), path);
      await until(() => !!document.querySelector('#importDlg'));
      const t = await text('.imprate');
      await p.keyboard.press('Escape');
      await until(() => !document.querySelector('#importDlg'));
      return t;
    };
    const daily = await asRecorded('/data/daily.csv');
    check('a daily file reads "daily · median interval 1 d"', /As recorded daily · median interval 1 d/.test(daily), daily);
    const fast = await asRecorded('/data/fast.csv');
    check('a fast one reads in kHz', /As recorded 2\.5 kHz/.test(fast), fast);
    const slow = await asRecorded('/data/slow.csv');
    check('a slow one gives its interval', /As recorded median interval 2\.5 s/.test(slow), slow);
  },
  { files: {}, launch: null },
);
