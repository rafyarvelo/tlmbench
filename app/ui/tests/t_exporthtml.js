// Export to HTML…: from the ⋯ menu's Export group; what the dialog offers (Full, Light, Automatic with their sizes against the
// budget, the time view or the whole recording, the layout's channels or every channel, the checks summary), the command line it
// says is the same, the choices remembered, and the request the core gets (the folder is chosen when Export is pressed).
import { run } from './lib.js';

run(
  'exporthtml',
  async ({ p, check, text, until, fake, settingsLS }) => {
    const dlg = '#htmlDlg';
    await p.click('#b-more');
    check('the ⋯ menu offers Export to HTML…', /Export to HTML…/.test(await text('.moremenu')), await text('.moremenu'));
    await p.click('.moremenu [data-more="exporthtml"]');
    check('it opens the dialog', await until(() => !!document.querySelector('#htmlDlg')));
    check('the summary counts the tab\'s channels and the mode Automatic picks', await until(() => /6 channels · Full: about/.test(document.querySelector('#htmlSummary')?.textContent ?? '')), await text('#htmlSummary'));
    check('the mode list shows both sizes', /Full \(/.test(await text('#htmlMode')) && /Light \(/.test(await text('#htmlMode')), await text('#htmlMode'));
    check('the command line is the same export', /tlmbench export html flight --out …/.test(await text(`${dlg} .hint`)), await text(`${dlg} .hint`));

    // a file that doesn't fit the budget: Automatic says Light, and why
    await fake(() => (window.__tlmFake.htmlSizeScale = 1e6));
    await p.selectOption('#htmlChannels', 'all');
    check('Automatic picks Light when Full is over the budget', await until(() => /Light: about/.test(document.querySelector('#htmlSummary')?.textContent ?? '')), await text('#htmlSummary'));
    check('and says what Light keeps', /Full doesn't fit the budget/.test(await text(dlg)), await text(dlg));
    await fake(() => (window.__tlmFake.htmlSizeScale = 1));
    await p.selectOption('#htmlMode', 'light');
    await p.selectOption('#htmlRange', 'view');
    check('the choices show in the command line', /--all-channels/.test(await text(`${dlg} .hint`)) && /--mode light/.test(await text(`${dlg} .hint`)), await text(`${dlg} .hint`));
    check('and are remembered in settings', (await settingsLS()).exportHtml?.mode === 'light' && (await settingsLS()).exportHtml?.range === 'view' && (await settingsLS()).exportHtml?.channels === 'all');

    await fake(() => (window.__tlmFake.pickFolder = '/out/dir'));
    await p.click('#htmlGo');
    check('Export asks the core to write', await until(() => window.__tlmFake.htmlExports.length === 1));
    const r = (await fake(() => window.__tlmFake.htmlExports))[0];
    check('into a folder named after the file, in the folder chosen', r.out === '/out/dir/flight-export', r.out);
    check('with the mode, every channel and the time view', r.mode === 'light' && r.channels.join() === '0,1,2,3,4,5' && r.from != null && r.until != null, r);
    check('the layout in front goes with it', r.layout?.schema === 1 && Array.isArray(r.layout?.panes) && Array.isArray(r.layout?.view), r.layout);
    check('it says what it wrote', await until(() => /Wrote 4\.0 MB \(Light\) to flight-export in \/out\/dir\./.test(document.querySelector('#htmlDone')?.textContent ?? '')), await text('#htmlDone'));

    // a failure is shown
    await fake(() => (window.__tlmFake.exportFails = 'disk full'));
    await p.click('#htmlGo');
    check('a failure is shown', await until(() => /Couldn't export: disk full/.test(document.querySelector('#htmlErr')?.textContent ?? '')), await text(dlg));
    await p.keyboard.press('Escape');
    check('Escape closes it', await until(() => !document.querySelector('#htmlDlg')));
  },
);
