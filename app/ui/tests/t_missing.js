// Scale, offset and missing values: the channel editor's Display tab sets them; the file opens again with them (the
// core gets them by channel name); Explorer's row gets a ⌀ badge with the count; Measure's stats say how many samples
// are missing and for how long; a condition the core can't read is named in the editor; Reset display undoes it all.
import { run } from './lib.js';

run(
  'missing',
  async ({ p, check, text, until, fake, settingsLS }) => {
    const last = () => fake(() => window.__tlmFake.adjusts.at(-1));
    const edit = async (sel, v) => {
      await p.fill(`#chEditor ${sel}`, v);
      await p.dispatchEvent(`#chEditor ${sel}`, 'change');
    };
    check('a file opens with no adjustments', (await last()).length === 0, await last());
    await p.click('#stack .chip[data-ch="0"]');

    await edit('[data-d="scale"]', '2');
    await edit('[data-d="offset"]', '0.5');
    check('a scale and offset reopen the file with them, by channel name', await until(() => window.__tlmFake.adjusts.at(-1).some((a) => a.channel === 'accel_x' && a.scale === 2 && a.offset === 0.5)), JSON.stringify(await last()));
    check('and are kept in settings', (await settingsLS()).chdisp?.accel_x?.scale === 2);
    check('nothing is missing yet', !(await p.$('#explorer [data-miss]')));

    await edit('[data-m="vals"]', '-9999, 65535');
    await edit('[data-m="hi"]', '100');
    check('missing values and a valid range reach the core', await until(() => window.__tlmFake.adjusts.at(-1).some((a) => a.channel === 'accel_x' && a.values?.join() === '-9999,65535' && a.hi === 100)), JSON.stringify(await last()));
    check('Explorer\'s row shows a ⌀ count', await until(() => /⌀ 7/.test(document.querySelector('#explorer .row.chan[data-ch="0"]')?.textContent ?? '')), await text('#explorer .row.chan[data-ch="0"]'));
    check('the editor says how many are missing', await until(() => /7 are missing/.test(document.querySelector('#chEditor')?.textContent ?? '')));
    check('other channels have no badge', !(await p.$('#explorer .row.chan[data-ch="1"] [data-miss]')));

    // Measure: the stats count them
    await p.click('#chEditor [data-close]');
    await p.click('[data-rail="measure"]');
    if ((await p.getAttribute('[data-rail="measure"]', 'aria-pressed')) !== 'true') await p.click('[data-rail="measure"]');
    check('Measure\'s stats count the missing samples and say for how long', await until(() => /Missing\s*3\s*Missing for/.test(document.querySelector('#mpStats')?.textContent ?? '')), await text('#mpStats'));
    await p.click('[data-rail="explorer"]');

    // a condition that can't be read
    await p.click('#stack .chip[data-ch="0"]');
    await edit('[data-m="when"]', 'bad > 1');
    check('a condition the core refuses is named in the editor', await until(() => /There's no channel called bad/.test(document.querySelector('#chErr')?.textContent ?? '')), await text('#chEditor'));
    await edit('[data-m="when"]', 'accel_y > 1');
    check('fixing it clears the message', await until(() => !document.querySelector('#chErr')));
    check('the condition reaches the core', (await last()).some((a) => a.when === 'accel_y > 1'), JSON.stringify(await last()));

    // undone
    await p.click('#chEditor [data-dreset]');
    check('Reset display opens the file without them', await until(() => window.__tlmFake.adjusts.at(-1).length === 0), JSON.stringify(await last()));
    check('and the badge goes', await until(() => !document.querySelector('#explorer [data-miss]')));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
