// Export to HTML…: from the ⋯ menu's Export group; what the dialog offers (Full, Light, Automatic with their sizes against the
// budget, the time view or the whole recording, the layout's channels or every channel, the checks summary), the command line it
// says is the same, the choices remembered, and the request the core gets (the folder is chosen when Export is pressed).
import { run } from './lib.js';

run(
  'exporthtml',
  async ({ p, check, text, until, fake, settingsLS }) => {
    const dlg = '#htmlDlg';
    await p.click('#b-export');
    check('the top bar\'s Export menu offers Export to HTML…', /Export to HTML…/.test(await text('.exportmenu')), await text('.exportmenu'));
    await p.click('.exportmenu [data-xm="html"]');
    check('it opens the dialog', await until(() => !!document.querySelector('#htmlDlg')));
    check('the summary counts the tab\'s channels and the mode Automatic picks', await until(() => /6 channels · Full: about/.test(document.querySelector('#htmlSummary')?.textContent ?? '')), await text('#htmlSummary'));
    check('the mode list shows both sizes', /Full \(/.test(await text('#htmlMode')) && /Light \(/.test(await text('#htmlMode')), await text('#htmlMode'));
    check('the command line is the same export', /tlmbench export html flight --out …/.test(await text(`${dlg} .hint`)), await text(`${dlg} .hint`));

    check('Light\'s raw windows are not offered in Full', !(await p.$('#htmlDetail')));

    // a file that doesn't fit the budget: Automatic says Light, and why
    await fake(() => (window.__tlmFake.htmlSizeScale = 1e6));
    await p.selectOption('#htmlChannels', 'all');
    check('Automatic picks Light when Full is over the budget', await until(() => /Light: about/.test(document.querySelector('#htmlSummary')?.textContent ?? '')), await text('#htmlSummary'));
    check('and says what Light keeps', /doesn't fit the budget/.test(await text(dlg)), await text(dlg));
    await fake(() => (window.__tlmFake.htmlSizeScale = 1));
    await p.selectOption('#htmlMode', 'light');
    await p.selectOption('#htmlRange', 'view');
    check('the choices show in the command line', /--all-channels/.test(await text(`${dlg} .hint`)) && /--mode light/.test(await text(`${dlg} .hint`)), await text(`${dlg} .hint`));
    check('and are remembered in settings', (await settingsLS()).exportHtml?.mode === 'light' && (await settingsLS()).exportHtml?.range === 'view' && (await settingsLS()).exportHtml?.channels === 'all');

    check('Light offers the raw windows around events: 3 times the duration, at least 30 s, at most 600 s', await p.$eval('#htmlDetFactor', (i) => i.value) === '3' && await p.$eval('#htmlDetMin', (i) => i.value) === '30' && await p.$eval('#htmlDetCap', (i) => i.value) === '600');
    await p.fill('#htmlDetMin', '60');
    await p.dispatchEvent('#htmlDetMin', 'change');
    check('changing one is in the command line (--detail) and remembered', await until(() => /--detail 3x,60s,600s/.test(document.querySelector('#htmlDlg .hint')?.textContent ?? '')) && (await settingsLS()).exportHtml?.detail?.min === 60, await text(`${dlg} .hint`));
    await fake(() => Object.assign(window.__tlmFake, { pickFolder: '/out/dir', htmlSteps: 4, htmlStepMs: 200 }));
    await p.click('#htmlGo');
    // the dialog closes at once; the export runs on with a progress bar under the top bar (sheet F1)
    check('the dialog closes as soon as the folder is chosen', await until(() => !document.querySelector('#htmlDlg')));
    check('the progress shows under the top bar, with a bar', await until(() => /Exporting flight to HTML/.test(document.querySelector('#wsTask')?.textContent ?? '') && !!document.querySelector('#wsTask [role=progressbar]')), await text('#wsTask'));
    const seen = new Set();
    for (let k = 0; k < 12; k++) {
      const v = await p.getAttribute('#wsTask [role=progressbar]', 'aria-valuenow', { timeout: 200 }).catch(() => null);
      if (v != null) seen.add(v);
      await p.waitForTimeout(80);
    }
    check('and it moves as the core reports', seen.size >= 2, [...seen]);
    check('Export asks the core to write', await until(() => window.__tlmFake.htmlExports.length === 1));
    const r = (await fake(() => window.__tlmFake.htmlExports))[0];
    check('into a folder named after the file, in the folder chosen', r.out === '/out/dir/flight-export', r.out);
    check('with the mode, every channel and the time view', r.mode === 'light' && r.channels.join() === '0,1,2,3,4,5' && r.from != null && r.until != null, r);
    check('the windows go with it', r.detail === '3x,60s,600s', r.detail);
    check('the layout in front goes with it', r.layout?.schema === 1 && Array.isArray(r.layout?.panes) && Array.isArray(r.layout?.view), r.layout);
    check('when done the progress goes and the note says where it went', await until(() => !document.querySelector('#wsTask') && /Exported to flight-export in \/out\/dir: 4\.0 MB, Light\./.test(document.querySelector('#wsNote .txt')?.textContent ?? ''), null, 8000), await text('#wsNote'));
    check('a finished export\'s dot is the accent colour, not a warning (UI audit, R2.1a)', await p.$eval('#wsNote .dot', (e) => e.classList.contains('ok')));
    const reveal = p.locator('#wsNote button', { hasText: /Show in (Finder|Explorer|folder)/ });
    check('with Show in Finder', (await reveal.count()) === 1, await text('#wsNote'));
    await reveal.click();
    check('which shows the export folder', await until(() => (window.__tlmFake.revealedPaths ?? []).join() === '/out/dir/flight-export'), await fake(() => window.__tlmFake.revealedPaths ?? null));

    // One file (owner decision 7): opens from anywhere, with its own size limit (default 25 MB)
    await p.click('#b-export');
    await p.click('.exportmenu [data-xm="html"]');
    await until(() => !!document.querySelector('#htmlDlg'));
    check('Save as offers Folder and One file, each with its size', await until(() => /Folder \(/.test(document.querySelector('#htmlFormat')?.textContent ?? '') && /One file \(/.test(document.querySelector('#htmlFormat')?.textContent ?? '')), await text('#htmlFormat'));
    check('a folder has no size limit field', !(await p.$('#htmlLimit')));
    await p.selectOption('#htmlFormat', 'file');
    check('One file shows the limit field, 25 MB to start', (await p.$eval('#htmlLimit', (i) => i.value)) === '25');
    check('the summary counts against the limit, not the budget', await until(() => /of the 25 MB limit/.test(document.querySelector('#htmlSummary')?.textContent ?? '')), await text('#htmlSummary'));
    check('the command line says --single-file', /--single-file/.test(await text(`${dlg} .hint`)) && !/--single-limit/.test(await text(`${dlg} .hint`)), await text(`${dlg} .hint`));
    await p.selectOption('#htmlMode', 'auto');
    check('every raw sample when they fit the limit: Automatic says Full', await until(() => /Full: about/.test(document.querySelector('#htmlSummary')?.textContent ?? '')), await text('#htmlSummary'));
    await fake(() => (window.__tlmFake.htmlSizeScale = 1e6));
    await p.selectOption('#htmlChannels', 'layout');
    await p.selectOption('#htmlChannels', 'all');
    check('Light when they don\'t, and says a folder has no such limit', await until(() => /A folder has no such limit/.test(document.querySelector('#htmlDlg')?.textContent ?? '')), await text(dlg));
    await fake(() => (window.__tlmFake.htmlSizeScale = 1));
    await p.selectOption('#htmlChannels', 'layout');
    await p.fill('#htmlLimit', '100');
    await p.dispatchEvent('#htmlLimit', 'change');
    check('a new limit is in the command line and remembered', await until(() => /--single-limit 100MB/.test(document.querySelector('#htmlDlg .hint')?.textContent ?? '')) && (await settingsLS()).exportHtml?.limit === 100 && (await settingsLS()).exportHtml?.format === 'file', await text(`${dlg} .hint`));
    await fake(() => Object.assign(window.__tlmFake, { savePick: '/out/mail/flight.html', htmlSteps: 0 }));
    await p.click('#htmlGo');
    check('Export asks the core for one file at the path chosen, with the limit', await until(() => window.__tlmFake.htmlExports.length === 2));
    const one = (await fake(() => window.__tlmFake.htmlExports))[1];
    check('single, out is the file, the limit in bytes', one.single === true && one.out === '/out/mail/flight.html' && one.singleLimit === 100 * 1048576, one);
    check('the note gives the file\'s size', await until(() => /Exported to flight\.html in mail: 5\.3 MB/.test(document.querySelector('#wsNote .txt')?.textContent ?? ''), null, 8000), await text('#wsNote'));
    await fake(() => (window.__tlmFake.savePick = null));
    await p.click('#b-export');
    await p.click('.exportmenu [data-xm="html"]');
    await until(() => !!document.querySelector('#htmlDlg'));
    await p.click('#htmlGo');
    await p.waitForTimeout(200);
    check('cancelling the save dialog writes nothing', (await fake(() => window.__tlmFake.htmlExports.length)) === 2);
    await p.selectOption('#htmlFormat', 'folder');
    await p.keyboard.press('Escape');
    await until(() => !document.querySelector('#htmlDlg'));

    // a failure is shown, with Try again reopening the dialog
    await fake(() => Object.assign(window.__tlmFake, { exportFails: 'disk full', htmlSteps: 0 }));
    await p.click('#b-export');
    await p.click('.exportmenu [data-xm="html"]');
    await until(() => !!document.querySelector('#htmlDlg'));
    await p.click('#htmlGo');
    check('a failure is shown', await until(() => /Couldn't export: disk full/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')), await text('#wsNote'));
    check('with the usual warning dot', !(await p.$eval('#wsNote .dot', (e) => e.classList.contains('ok'))));
    await p.locator('#wsNote button', { hasText: 'Try again' }).click();
    check('Try again opens the dialog again', await until(() => !!document.querySelector('#htmlDlg')));
    await p.keyboard.press('Escape');
    check('Escape closes it', await until(() => !document.querySelector('#htmlDlg')));
  },
);
