// Reconnect and Save recordings to (D-live-11, the prototype's Connect form): TCP and serial offer "Reconnect
// automatically if the link drops", on by default, and UDP, which has no link, doesn't; the choice reaches the core and
// is kept with the form; "Save recordings to" starts from Settings' folder, can be changed for this connection, and
// reaches the core; a source whose link dropped says Reconnecting on its badge and card and can still be stopped.
import { run } from './lib.js';

const TCP = 'tcp://127.0.0.1:9000';
run(
  'reconnect',
  async ({ p, check, text, fake, until, settingsLS }) => {
    await fake((e) => (window.__tlmFake.sources[e] = { channels: ['rpm', 'temp'], rate: 1000, already: 5 }), TCP);
    await p.evaluate(() => window.__tlmSettings.set('recordTo', '/Users/me/Recordings'));
    await p.click('[data-rail="live"]');
    await p.click('#liveDlg [data-kind="udp"]');
    check('UDP has no link to drop: no Reconnect', !(await p.$('#lvReconnect')));
    await p.click('#liveDlg [data-kind="serial"]');
    check('a serial port offers Reconnect, on', await p.isChecked('#lvReconnect'));
    await p.click('#liveDlg [data-kind="tcp"]');
    check('TCP offers Reconnect, on by default', await p.isChecked('#lvReconnect'));
    check("Save recordings to starts from Settings' folder", (await p.inputValue('#lvRecordTo')) === '/Users/me/Recordings', await p.inputValue('#lvRecordTo'));
    await p.fill('#lvHost', '127.0.0.1');
  await p.fill('#lvPortNo', '9000');
    await p.fill('#lvDesc', '/data/frames.toml');
    await p.fill('#lvRecordTo', '/Volumes/Bench/rec');
    await p.click('#lvConnect');
    await until(() => !document.querySelector('#liveDlg'));
    const args = await fake(() => window.__tlmFake.liveConnects.at(-1));
    check('Reconnect and the folder reach the core', args?.reconnect === true && args?.dir === '/Volumes/Bench/rec', args);
    const form = (await settingsLS()).liveForm;
    check('the form keeps both for next time', form?.reconnect === true && form?.recordTo === '/Volumes/Bench/rec', form);
    check("Settings' folder is unchanged", (await settingsLS()).recordTo === '/Users/me/Recordings');
    check('streaming', await until(() => document.getElementById('lvBadge')?.textContent === 'LIVE'), await text('#lvBadge'));

    // the link drops: the core says it is connecting again
    await fake((e) => (window.__tlmFake.reconnecting = { [e]: 2 }), TCP);
    check('the badge says RECONNECTING', await until(() => document.getElementById('lvBadge')?.textContent === 'RECONNECTING'), await text('#lvBadge'));
    const tip = await p.getAttribute('#lvBadge', 'data-tip');
    check('its tip says what happens and how many times', /connecting again/.test(tip ?? '') && /2 times/.test(tip ?? ''), tip);
    await p.click('#lvSrcBtn');
    const card = await until(() => document.querySelector('.lvcard .lvcs')?.textContent);
    check('the card says Reconnecting…', card === 'Reconnecting…', card);
    check('and it can still be stopped', !!(await p.$('.lvcard [data-sstop]')));
    await fake(() => (window.__tlmFake.reconnecting = {}));
    check('back to LIVE once it answers', await until(() => document.getElementById('lvBadge')?.textContent === 'LIVE'), await text('#lvBadge'));

    // a source added with Reconnect off sends that
    await p.click('#lvAddSrc');
    await p.click('#liveDlg [data-kind="tcp"]');
    check('the dialog remembers Reconnect', await p.isChecked('#lvReconnect'));
    await p.uncheck('#lvReconnect');
    await p.click('#lvConnect');
    await until(() => !document.querySelector('#liveDlg'));
    const off = await fake(() => window.__tlmFake.liveConnects.at(-1));
    check('Reconnect off reaches the core', off?.reconnect === false, off);
  },
  { files: {}, launch: null },
);
