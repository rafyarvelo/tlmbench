// Plugins, step 5.9: Settings lists them with tier and version (a native one stays off until you turn it on, a
// folder's native one is refused, a broken one says why), Open folder and Rescan, and an alignment plugin in the
// Alignment section (chosen, its parameters, remembered with the pair). The fake core stands in for `tlmbench-plugin`;
// the Rust side has its own tests (crates/plugin/tests).
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = {
  [A]: {},
  [B]: { channels: ['lat', 'lon', 'accel_x'], units: ['deg', 'deg', 'g'], samples: 20000, rate: 100 },
};

run(
  'plugins',
  async ({ p, check, text, visible, fake, until, settingsLS }) => {
    const calls = (k) => fake((k) => window.__tlmFake[k], k);
    await p.click('[data-rail="settings"]');
    check('Settings lists every plugin file', await until(() => document.querySelectorAll('[data-plugin]').length === 6), await p.$$eval('[data-plugin]', (r) => r.map((x) => x.dataset.plugin)));
    const line = (f) => p.$eval(`[data-plugin="${f}"]`, (e) => e.textContent.replace(/\s+/g, ' ').trim());
    check('a WASM decoder shows its name, version, kind and tier', /demo 0\.1\.0 · Decoder · WASM, sandboxed/.test(await line('my_decoder.wasm')), await line('my_decoder.wasm'));
    check('an alignment plugin says so, and its description', /peak.*Alignment · WASM, sandboxed.*Lines files up/.test(await line('my_estimator.wasm')), await line('my_estimator.wasm'));
    check('a native plugin is off and says it is not loaded', /Native, runs with your rights/.test(await line('libfast.dylib')) && /Off/.test(await line('libfast.dylib')) && !(await p.isChecked('[data-pluginon="libfast.dylib"]')), await line('libfast.dylib'));
    check('a plugin that will not load says why', /not a plugin component/.test(await text('[data-plugin="bad.wasm"] [data-plugwhy]')));
    check('the folder\'s native plugin is refused, with no switch', /only from your own/.test(await text('[data-plugin="shared.so"] [data-plugwhy]')) && !(await p.$('[data-pluginon="shared.so"]')));
    check('a folder\'s plugin says where it came from', /from this folder/.test(await line('shared.so')));

    // a folder's plugins wait for your OK
    check("a folder's plugin is listed, not loaded, and says why", /Not loaded until you trust/.test(await text('[data-plugin="team.wasm"] [data-plugwhy]')), await line('team.wasm'));
    check('the folder asks once for all of them', /waiting for your OK/.test(await text('[data-plugfolder]')) && (await p.$$('[data-plugtrust]')).length === 1, await text('[data-plugfolder]'));
    await p.click('[data-plugtrust]');
    check('Trust rescans with the folder trusted', await until(() => window.__tlmFake.trustedFolders?.join() === '/data/.tlmbench/plugins'), await calls('trustedFolders'));
    check('it is remembered in settings', (await settingsLS()).trustedPluginFolders?.join() === '/data/.tlmbench/plugins', (await settingsLS()).trustedPluginFolders);
    check('and the plugin loads, as a decoder', (await until(() => /Decoder · WASM/.test(document.querySelector('[data-plugin="team.wasm"]')?.textContent ?? ''))) && /trusted/.test(await text('[data-plugfolder]')), await line('team.wasm'));
    await p.click('[data-plugdistrust]');
    check('Stop trusting puts it back to waiting', (await until(() => /Not loaded until you trust/.test(document.querySelector('[data-plugin="team.wasm"] [data-plugwhy]')?.textContent ?? ''))) && (await settingsLS()).trustedPluginFolders?.length === 0);

    // turning a native plugin on
    await p.check('[data-pluginon="libfast.dylib"]');
    check('turning it on rescans with it on', await until(() => window.__tlmFake.nativeOn?.join() === 'libfast.dylib'), await calls('nativeOn'));
    check('it is remembered in settings', (await settingsLS()).nativePlugins?.join() === 'libfast.dylib', (await settingsLS()).nativePlugins);
    check('and it no longer says off', await until(() => !/Off/.test(document.querySelector('[data-plugin="libfast.dylib"]')?.textContent ?? '')));
    await p.uncheck('[data-pluginon="libfast.dylib"]');
    check('turning it off again forgets it', await until(() => window.__tlmFake.nativeOn?.length === 0) && (await settingsLS()).nativePlugins?.length === 0);

    // Open folder, Rescan
    await p.click('#pluginsOpen');
    check('Open folder asks the core to reveal it', await until(() => window.__tlmFake.pluginsRevealed === 1), await calls('pluginsRevealed'));
    const n = await calls('scans');
    await p.click('#pluginsRescan');
    check('Rescan reads the folders again', await until((n) => window.__tlmFake.scans > n, n));

    // an alignment plugin in the Alignment section
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('[data-rail="explorer"]');
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    await p.click('#explorer .row.slot[data-slot="2"]');
    const options = await p.$$eval('[data-falign="2"] option', (o) => o.map((x) => x.value));
    check('the Alignment list offers the loaded estimator, not the decoder or the broken one', options.includes('plugin:peak') && options.length === 8 && !options.some((o) => /demo|bad/.test(o)), options);
    await fake(() => (window.__tlmFake.alignResults.plugin = { knots: [['0', '2500000000']], residuals: [], quality: 1, note: 'Peaks of accel_x matched' }));
    await p.selectOption('[data-falign="2"]', 'plugin:peak');
    check('choosing it asks the core for that plugin', await until(() => window.__tlmFake.aligns.at(-1)?.method.method === 'plugin'), await calls('aligns'));
    const last = async () => (await calls('aligns')).at(-1).method;
    check('with its name and no parameters yet', (await last()).name === 'peak' && (await last()).params === '', await last());
    check('its note shows, and the result can be kept or undone', await until(() => /Peaks of accel_x matched/.test(document.querySelector('.align .anote')?.textContent ?? '')) && (await visible('[data-preview]')));
    await p.fill('[data-fparams="2"]', '{"channel": "accel_x"}');
    await p.press('[data-fparams="2"]', 'Tab');
    check('its parameters go with the next run', await until(() => window.__tlmFake.aligns.at(-1)?.method.params === '{"channel": "accel_x"}'), await last());
    check('the file\'s summary names the plugin', /peak/.test(await text('#explorer .row.slot[data-slot="2"] [data-asum]')), await text('#explorer .row.slot[data-slot="2"] [data-asum]'));
    check('the pair is saved with the plugin and its parameters', await until(() => window.__tlmFake.timemaps.at(-1)?.plugin === 'peak' && window.__tlmFake.timemaps.at(-1)?.params === '{"channel": "accel_x"}'), await calls('timemaps'));
  },
  {
    files,
    launch: null,
    setup: (fake) => {
      fake.pluginFiles = [
        { file: 'my_decoder.wasm', path: '/plugins/my_decoder.wasm', tier: 'wasm', kind: 'decoder', name: 'demo', version: '0.1.0', description: 'The template\'s made-up DEMO format', shared: false, status: { state: 'loaded' } },
        { file: 'my_estimator.wasm', path: '/plugins/my_estimator.wasm', tier: 'wasm', kind: 'estimator', name: 'peak', version: '0.1.0', description: 'Lines files up on the moment a channel peaks', shared: false, status: { state: 'loaded' } },
        { file: 'libfast.dylib', path: '/plugins/libfast.dylib', tier: 'native', kind: 'decoder', name: 'libfast', version: null, description: null, shared: false, status: { state: 'off' } },
        { file: 'bad.wasm', path: '/plugins/bad.wasm', tier: 'wasm', kind: null, name: 'bad', version: null, description: null, shared: false, status: { state: 'failed', why: '/plugins/bad.wasm: not a plugin component (unknown binary)' } },
        { file: 'shared.so', path: '/data/.tlmbench/plugins/shared.so', tier: 'native', kind: 'decoder', name: 'shared', version: null, description: null, shared: true, folder: '/data/.tlmbench/plugins', status: { state: 'failed', why: 'Native plugins load only from your own plugins folder' } },
        { file: 'team.wasm', path: '/data/.tlmbench/plugins/team.wasm', tier: 'wasm', kind: null, kindWhenTrusted: 'decoder', name: 'team', version: null, description: null, shared: true, folder: '/data/.tlmbench/plugins', waiting: true, status: { state: 'untrusted' } },
      ];
      fake.plugins = fake.pluginFiles;
    },
  },
);
