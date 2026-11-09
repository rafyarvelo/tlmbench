// Channel display: a chip opens the channel editor; alias, colour, decimals and unit label apply to the chip and are
// kept in settings by channel name; Display… is in the channel's right-click menu; Reset display clears them.
import { run } from './lib.js';

run(
  'display',
  async ({ p, check, text, until, settingsLS }) => {
    const chip = '#stack section.pane .chip[data-ch="0"]';
    const first = (await text(chip)).trim();
    check('a chip shows the channel\'s own name', /accel_x/.test(first), first);
    await p.click(chip);
    check('a click on a chip opens the channel editor', await until(() => !!document.querySelector('#chEditor')));

    await p.fill('#chEditor [data-d="alias"]', 'Lateral g');
    await p.dispatchEvent('#chEditor [data-d="alias"]', 'change');
    check('the alias shows on the chip', await until(() => /Lateral g/.test(document.querySelector('#stack .chip[data-ch="0"]')?.textContent ?? '')), await text(chip));
    check('the alias is kept by channel name', (await settingsLS()).chdisp?.accel_x?.alias === 'Lateral g', JSON.stringify((await settingsLS()).chdisp));

    await p.fill('#chEditor [data-d="unit"]', 'm/s²');
    await p.dispatchEvent('#chEditor [data-d="unit"]', 'change');
    check('the unit label shows on the chip', await until(() => /m\/s²/.test(document.querySelector('#stack .chip[data-ch="0"] .unit')?.textContent ?? '')));

    await p.click('#chEditor [data-sw="5"]');
    check('a palette colour is used by the chip', await until(() => document.querySelector('#stack .chip[data-ch="0"]').getAttribute('style').includes('--trace-5')));
    check('the colour is kept', (await settingsLS()).chdisp?.accel_x?.color === 5);
    await p.click('#chEditor [data-autocolor]');
    check('Auto brings the palette slot back', await until(() => !document.querySelector('#stack .chip[data-ch="0"]').getAttribute('style').includes('--trace-5')));

    await p.selectOption('#chEditor [data-d="dec"]', '1');
    check('decimals are kept', await until(() => true) && (await settingsLS()).chdisp?.accel_x?.dec === 1);
    await p.check('#chEditor [data-d="dash"]');
    check('line style is kept', (await settingsLS()).chdisp?.accel_x?.dash === true);

    await p.click('#chEditor [data-dreset]');
    check('Reset display clears the entry', await until(() => true) && !(await settingsLS()).chdisp?.accel_x, JSON.stringify((await settingsLS()).chdisp));
    check('the chip is back to the recording\'s name', await until(() => /accel_x/.test(document.querySelector('#stack .chip[data-ch="0"]')?.textContent ?? '')));
    await p.click('#chEditor [data-close]');
    check('Done closes the editor', await until(() => !document.querySelector('#chEditor')));

    await p.click(chip, { button: 'right' });
    check('the right-click menu has Display…', !!(await p.$('.menu [data-cm="display"]')));
    await p.click('.menu [data-cm="display"]');
    check('Display… opens the editor', await until(() => !!document.querySelector('#chEditor')));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
