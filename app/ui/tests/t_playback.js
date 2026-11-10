// Live, step 5.5: the Connect dialog's file playback, name, record switch, preview and saved connections; the strip's
// session name, PAUSED (a replay stops in time), the "dropping samples" warning, the scrubber's faint trace; the
// Sources panel's cards and "+ Add source"; Settings' "Record live sessions to"; the start page's connections and
// recording banner.
import { run } from './lib.js';

const REC = '/data/flight.tlm';
const CONNECT = '#liveDlg [data-kind="playback"]';

run(
  'playback',
  async ({ p, check, text, visible, fake, until, settingsLS, wait }) => {
    const connects = () => fake(() => window.__tlmFake.liveConnects);
    const dlg = async () => {
      await p.click('[data-rail="live"]');
      await until(() => !!document.getElementById('liveDlg'));
    };

    // the dialog: playback is the first kind and the default
    await dlg();
    check('file playback is the default, and asks for a recording', (await p.getAttribute(CONNECT, 'aria-pressed')) === 'true' && (await p.isDisabled('#lvConnect')));
    check('the name starts as the kind\'s', (await p.inputValue('#lvName')) === 'File playback');
    await p.fill('#lvRecording', REC);
    check('the name follows the recording', (await p.inputValue('#lvName')) === 'flight.tlm playback', await p.inputValue('#lvName'));
    check('the preview names the channels it decodes', await until(() => /accel_x/.test(document.getElementById('lvPreview')?.textContent ?? '') && /Decodes as/.test(document.getElementById('lvPreview').textContent)), await text('#lvPreview'));
    check('speed, start and loop are the playback\'s own fields', (await p.inputValue('#lvSpeed')) === '2' && (await p.inputValue('#lvStart')) === '0' && (await p.isChecked('#lvLoop')));
    check('recording is on and says where', (await p.isChecked('#lvRecord')) && /Recordings folder/.test(await text('.lvwhere')), await text('.lvwhere'));
    await p.fill('#lvName', 'Bench demo');
    await p.selectOption('#lvSpeed', '5');
    await p.fill('#lvStart', '12.5');
    await p.uncheck('#lvLoop');
    await p.check('#lvSave');
    check('a name you typed stays when the recording changes', (await p.inputValue('#lvName')) === 'Bench demo');
    await p.click('#lvConnect');
    check('connecting opens a live tab named for it', await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.textContent.includes('Bench demo')) && !(await visible('#liveDlg')));
    const c0 = (await connects())[0];
    check('the core is asked for a replay at that speed, start and no loop', c0.endpoint === `replay://${REC}?speed=5&start=12.5&loop=0` && c0.name === 'Bench demo' && c0.record === true && c0.dir === null && c0.addTo === null, c0);
    check('the strip names the session', (await text('#lvSession')) === 'Bench demo' && (await text('#lvBadge')) === 'LIVE');

    // the faint trace behind the scrubber, once there is a session to draw
    check('the scrubber has the session drawn faintly behind it', await until(() => /^M/.test(document.querySelector('#lvTrace path')?.getAttribute('d') ?? ''), null, 6000));

    // pausing a replay stops time: the badge says PAUSED and why, the recorded time holds, resuming goes on
    await p.click('#lvSrcBtn');
    check('the Sources panel has the source\'s card', (await p.$$('#lvSources [data-card]')).length === 1 && /Streaming/.test(await text('#lvSources [data-card]')) && (await text('#lvSrcBtn')) === '1 source');
    await p.click('#lvSources [data-spause]');
    check('pause shows PAUSED', await until(() => document.getElementById('lvBadge')?.textContent === 'PAUSED'));
    check('...and the badge says a replay stops in time', /replay stops in time/.test(await p.getAttribute('#lvBadge', 'data-tip')));
    const rec = await text('#lvRec');
    await wait(1200);
    check('the recorded time holds still while paused', (await text('#lvRec')) === rec, [rec, await text('#lvRec')]);
    check('the card says Paused and offers Resume', /Paused/.test(await text('#lvSources [data-card]')) && (await text('#lvSources [data-spause]')) === 'Resume');
    await p.click('#lvSources [data-spause]');
    check('resuming goes on from where it paused', await until(() => document.getElementById('lvBadge')?.textContent === 'LIVE') && (await until((r) => document.getElementById('lvRec').textContent !== r, rec)));

    // dropping samples: a warning with the counts
    check('no warning while nothing is lost', !(await p.$('#lvWarn')));
    await fake(() => (window.__tlmFake.liveDropped = 1234));
    check('records dropped show a warning with the count', await until(() => /Dropping samples · 1,234 lost/.test(document.getElementById('lvWarn')?.textContent ?? '')), await text('#liveStrip'));
    check('...and its tooltip says why', /1,234 records dropped \(the buffer was full\)/.test(await p.getAttribute('#lvWarn', 'data-tip')), await p.getAttribute('#lvWarn', 'data-tip'));
    await fake(() => (window.__tlmFake.liveDropped = 0));

    // + Add source: the dialog says it joins the session, has no record switch, and the source joins it
    await p.click('#lvAddSrc');
    check('Add source opens the dialog for joining the session', /Add a source/.test(await text('#liveDlg header')) && !(await p.$('#lvRecord')) && /Joins this session/.test(await text('#liveDlg footer')));
    await p.fill('#lvRecording', REC);
    await p.click('#lvConnect');
    check('the second source joins the tab as a slot, not a tab of its own', (await until(() => window.__tlmFake.groupSets.length === 1)) && (await p.$$eval('#wsTabs .wstab[data-tip^="replay://"]', (t) => t.length)) === 1, await fake(() => window.__tlmFake.groupSets));
    const c1 = (await connects())[1];
    check('it joins the first one\'s session', typeof c1.addTo === 'number' && c1.endpoint.startsWith('replay://'), c1);
    if (!(await p.$('#lvSources'))) await p.click('#lvSrcBtn');
    check('the strip counts two sources, with a card each', (await until(() => /2 sources/.test(document.getElementById('lvSrcBtn')?.textContent ?? ''))) && (await p.$$('#lvSources [data-card]')).length === 2);
    check('Remove closes the source, not the tab', await (async () => {
      await p.click('#lvSources [data-card]:last-child [data-sremove]');
      return (await until(() => document.querySelectorAll('#lvSources [data-card]').length === 1)) && (await p.$$eval('#wsTabs .wstab[data-tip^="replay://"]', (t) => t.length)) === 1;
    })());

    // the start page: the saved connection, and the banner that a session is being recorded
    await p.click('#b-start');
    check('the start page lists the saved connection', await until(() => /Bench demo/.test(document.querySelector('#startPage [data-conn]')?.textContent ?? '')));
    check('...and says it is recording', /Recording to \/rec\/session-1/.test(await text('#startRec')), await text('#startRec'));
    await p.click('#startPage.overlay [data-conn] .connopen');
    check('opening it fills the dialog as it was saved', (await p.inputValue('#lvName')) === 'Bench demo' && (await p.inputValue('#lvRecording')) === REC && (await p.inputValue('#lvSpeed')) === '5' && (await p.inputValue('#lvStart')) === '12.5' && !(await p.isChecked('#lvLoop')) && !(await p.$('#lvSave')));
    check('the last form is kept for next time', (await settingsLS()).liveForm?.recording === REC);
    await p.keyboard.press('Escape');

    // record off: the session isn't kept, and the dialog says so. From a live tab, "Open in" picks a new tab.
    await p.click('#b-start');
    await p.click('#startPage.overlay [data-start="live"]');
    await p.click(CONNECT);
    check('with live tabs open, the dialog offers to add a source to one', (await p.$$eval('#lvTarget option', (o) => o.length)) >= 2 && (await p.inputValue('#lvTarget')) === '');
    await p.fill('#lvRecording', REC);
    await p.uncheck('#lvRecord');
    check('without recording, it says the recording goes with the source', /Not kept/.test(await text('.lvwhere')));
    await p.click('#lvConnect');
    await until(() => document.querySelectorAll('#wsTabs .wstab[data-tip^="replay://"]').length >= 2);
    const c2 = (await connects()).at(-1);
    check('the core is told not to keep it', c2.record === false, c2);
    await p.click('#lvSrcBtn');
    check('its card says its recording is not kept', await until(() => /not kept/.test(document.querySelector('#lvSources [data-card].front')?.textContent ?? '')), await text('#lvSources'));

    // Settings: where live sessions are recorded
    await fake(() => (window.__tlmFake.pickFolder = '/Volumes/bench/sessions'));
    await p.click('[data-rail="settings"]');
    check('Settings says where live sessions go, by default the Recordings folder', await until(() => /Recordings folder/.test(document.getElementById('recDir')?.textContent ?? '')), await text('#recDir'));
    await p.click('#recChoose');
    check('Record live sessions to… takes a folder', await until(() => document.getElementById('recDir')?.textContent === '/Volumes/bench/sessions') && (await settingsLS()).recordTo === '/Volumes/bench/sessions', await text('#recDir'));
    await p.click('#b-start');
    await p.click('#startPage.overlay [data-start="live"]');
    await p.click(CONNECT);
    await p.fill('#lvRecording', REC);
    await p.check('#lvRecord');
    check('the dialog names that folder', /\/Volumes\/bench\/sessions/.test(await text('.lvwhere')), await text('.lvwhere'));
    await p.click('#lvConnect');
    await until(async () => true);
    await wait(500);
    check('the core is told where to record', (await connects()).at(-1).dir === '/Volumes/bench/sessions', (await connects()).at(-1));
    await p.click('#recDefault');
    check('Default puts it back', (await settingsLS()).recordTo === '');
  },
  { files: { [REC]: {} }, launch: null },
);
