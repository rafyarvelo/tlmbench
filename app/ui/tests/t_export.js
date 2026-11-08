// Export data…: from the ⋯ menu's Export group; what the dialog offers (format, time view / cursors / whole recording,
// this tab's channels or every channel, wide or long, resampled, times, stored values), what it tells about size and
// files, the choices remembered, and the request the core gets (the folder is chosen when Export is pressed).
import { run } from './lib.js';

run(
  'export',
  async ({ p, check, text, until, fake, visible, settingsLS }) => {
    const dlg = '#exportDlg';
    const asked = () => fake(() => window.__tlmFake.dataExports);
    await p.click('#b-more');
    check('the ⋯ menu has an Export group', /Export data…/.test(await text('.moremenu')) && /Save the view as an image…/.test(await text('.moremenu')), await text('.moremenu'));
    await p.click('.moremenu [data-more="exportdata"]');
    check('Export data… opens the dialog', await until(() => !!document.querySelector('#exportDlg')));
    check('the summary counts this tab\'s channels', /6 channels · .* rows × 7 columns/.test(await text('#exSummary')), await text('#exSummary'));
    check('the files it writes are named after the recording', /flight\.csv/.test(await text('.exfiles')) && /flight\.channels\.csv/.test(await text('.exfiles')), await text('.exfiles'));
    check('cursors are offered only once placed', await p.$eval('#exRange option[value=cursors]', (o) => o.disabled));
    await p.selectOption('#exFormat', 'hdf5');
    check('HDF5 has no long shape', await p.$eval('#exLayout option[value=long]', (o) => o.disabled));
    check('and names its file .h5', /flight\.h5/.test(await text('.exfiles')), await text('.exfiles'));
    await p.selectOption('#exFormat', 'parquet');
    await p.selectOption('#exLayout', 'long');
    check('long: three columns', /× 3 columns/.test(await text('#exSummary')), await text('#exSummary'));
    await p.selectOption('#exChannels', 'all');
    check('every channel counts all six', /6 channels/.test(await text('#exSummary')), await text('#exSummary'));
    await p.selectOption('#exRange', 'all');
    await p.selectOption('#exResample', '1');
    await p.fill('#exHz', '50');
    await p.dispatchEvent('#exHz', 'change');
    check('the CLI equivalent follows the choices', /--to parquet/.test(await text('.hint')) && /--layout long/.test(await text('.hint')) && /--resample 50Hz/.test(await text('.hint')), await text('.hint'));
    await p.selectOption('#exTime', 'ns');
    await p.check('#exRaw');

    check('the choices are remembered in settings', (await settingsLS()).exportData?.format === 'parquet' && (await settingsLS()).exportData?.hz === 50);

    await fake(() => (window.__tlmFake.pickFolder = '/out/dir'));
    await p.click('#exGo');
    check('Export asks the core to write', await until(() => window.__tlmFake.dataExports.length === 1));
    const r = (await asked())[0];
    check('into the folder chosen, named after the file', r.out === '/out/dir/flight.parquet', r.out);
    check('with the format, shape, rate, time and stored values', r.format === 'parquet' && r.layout === 'long' && r.resampleHz === 50 && r.time === 'ns' && r.raw === true, r);
    check('every channel of the file', r.channels.join() === '0,1,2,3,4,5', r.channels);
    check('the whole recording: no span', r.from === null && r.until === null, [r.from, r.until]);
    check('it says what it wrote', await until(() => /Wrote 1,234 rows of 6 channels to flight\.parquet and flight\.channels\.csv/.test(document.querySelector('#exDone')?.textContent ?? '')), await text('#exDone'));

    // the time view: the span goes with the request; a failure is shown
    await p.selectOption('#exRange', 'view');
    await p.selectOption('#exChannels', 'tab');
    await p.selectOption('#exResample', '');
    await fake(() => (window.__tlmFake.exportFails = 'disk full'));
    await p.click('#exGo');
    check('a failure is shown in the dialog', await until(() => /Couldn't export: disk full/.test(document.querySelector('#exErr')?.textContent ?? '')), await text('#exErr'));
    const v = (await asked())[1];
    check('the time view is the span', typeof v.from === 'string' && typeof v.until === 'string' && BigInt(v.until) > BigInt(v.from), [v.from, v.until]);
    check('this tab\'s channels, not resampled', v.channels.join() === '0,1,2,3,4,5' && v.resampleHz === null, v);

    // no channels in the panes: nothing to export until some are added or every channel is chosen
    await p.keyboard.press('Escape');
    await p.click('#b-more');
    await p.click('.moremenu [data-more="nochan"]');
    await p.click('#b-more');
    await p.click('.moremenu [data-more="exportdata"]');
    await until(() => !!document.querySelector('#exportDlg'));
    check('with empty panes Export waits and says what to do', (await p.isDisabled('#exGo')) && /add channels to the panes/.test(await text('#exSummary')), await text('#exSummary'));
    await p.selectOption('#exChannels', 'all');
    check('every channel brings it back', !(await p.isDisabled('#exGo')));

    // cancelling the folder dialog writes nothing
    await fake(() => { window.__tlmFake.exportFails = null; window.__tlmFake.pickFolder = null; });
    await p.click('#exGo');
    await p.waitForTimeout(300);
    check('cancelling the folder writes nothing', (await asked()).length === 2);
    await p.keyboard.press('Escape');
    check('Escape closes the dialog', !(await visible(dlg)));
  },
);
