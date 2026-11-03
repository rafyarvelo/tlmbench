// Export data and Save the view as an image (⋯ menu → Export): format, time (view, cursors, whole recording), channels
// (this tab's or all), wide or long, resampling, time format and stored values, with the size, the files and the
// matching `tlmbench convert` command; the image's format, panes and size; the choices saved in settings.
const T = require('./lib');
T.run('export', async ({ p, check, wait, text, settingsLS }) => {
  const more = async (label) => {
    await p.click('#b-more'); await wait(200);
    const items = await p.evaluate(() => [...document.querySelectorAll('.moremenu .mgrp, .moremenu .mi')].map(x => x.textContent.trim()));
    if (label) { await p.evaluate(l => [...document.querySelectorAll('.moremenu .mi')].find(x => x.textContent.includes(l)).click(), label); await wait(300); }
    return items;
  };
  const items = await more();
  const at = items.indexOf('Export');
  check('the ⋯ menu has an Export group', at >= 0 && items.slice(at + 1, at + 4).join('|') === 'Export data…|Save the view as an image…|Export to HTML…', items);
  await p.keyboard.press('Escape'); await wait(100);

  // Export data
  await more('Export data…');
  const sum = () => text('#recDlg .exsum');
  const files = () => p.$$eval('#recDlg .exfiles code', c => c.map(x => x.textContent));
  const cli = () => text('#recDlg .hint');
  check('it says the channels, rows, columns and size', /^\d+ channels? · [\d,]+ rows × \d+ columns · about [\d.]+ (KB|MB|GB)/.test(await sum()), await sum());
  check('a data file and the channel list', /\.csv$/.test((await files())[0]) && /\.channels\.csv$/.test((await files())[1]), await files());
  check('the matching command, for the time view', /^Same as tlmbench convert .+ --to csv --from [\d.]+ --until [\d.]+ -c \S+/.test(await cli()), await cli());
  check('between the cursors needs them placed', await p.$eval('#recDlg [data-xo="range"] option[value="cursors"]', o => o.disabled && /place them first/.test(o.textContent)));
  const wideCols = +(/× (\d+) columns/.exec(await sum())[1]);

  await p.selectOption('#recDlg [data-xo="format"]', 'parquet'); await wait(150);
  check('Parquet', /\.parquet$/.test((await files())[0]) && /--to parquet/.test(await cli()), await files());
  await p.selectOption('#recDlg [data-xo="layout"]', 'long'); await wait(150);
  check('long: time, channel, value', /× 3 columns/.test(await sum()) && /--layout long/.test(await cli()), await sum());
  await p.selectOption('#recDlg [data-xo="format"]', 'hdf5'); await wait(150);
  check('HDF5 is wide only', await p.$eval('#recDlg [data-xo="layout"] option[value="long"]', o => o.disabled) && new RegExp(`× ${wideCols} columns`).test(await sum()) && /\.h5$/.test((await files())[0]), [await sum(), await files()]);

  // resampling, all channels, the whole recording, times and stored values
  check('the rate waits for resampling', !(await p.$('#recDlg [data-xo="hz"]')));
  await p.selectOption('#recDlg [data-xo="resample"]', '1'); await wait(150);
  await p.fill('#recDlg [data-xo="hz"]', '10'); await p.press('#recDlg [data-xo="hz"]', 'Tab'); await wait(150);
  check('resampled to 10 Hz', /--resample 10Hz/.test(await cli()), await cli());
  await p.selectOption('#recDlg [data-xo="range"]', 'all'); await wait(150);
  const rows = +(/· ([\d,]+) rows/.exec(await sum())[1].replace(/,/g, ''));
  const dur = await p.evaluate(() => window.__tlmExport.dur());
  check('the whole recording at 10 Hz', rows === Math.round(10 * dur) && !/--from/.test(await cli()), [rows, dur]);
  await p.selectOption('#recDlg [data-xo="channels"]', 'all'); await wait(150);
  check('every channel: more columns, no -c', +(/× (\d+) columns/.exec(await sum())[1]) > wideCols && !/ -c /.test(await cli()), await sum());
  await p.selectOption('#recDlg [data-xo="time"]', 'iso'); await wait(100);
  await p.check('#recDlg [data-xo="raw"]'); await wait(100);
  check('times and stored values', /--time iso --raw/.test(await cli()), await cli());
  const saved = (await settingsLS()).exportDataOpts;
  check('the choices are saved', saved && saved.format === 'hdf5' && saved.range === 'all' && saved.channels === 'all' && saved.resample === true && saved.hz === '10' && saved.raw === true, saved);
  await p.click('#recDlg [data-xgo]'); await wait(150);
  check('Export (simulated)', /^Exported [\d,]+ rows of \d+ channels to .+\.h5 and .+\.channels\.csv/.test(await text('#recDlg [data-xmsg]')), await text('#recDlg [data-xmsg]'));
  await p.click('#recDlg [data-x]'); await wait(150);

  // reopened, it starts from the saved choices; with cursors, between them
  await p.evaluate(() => { const [t0] = window.__tlmExport.view(); window.__tlmPlay.cursors(t0 + 10, t0 + 40); });
  await more('Export data…');
  check('reopened with the saved choices', (await p.$eval('#recDlg [data-xo="format"]', s => s.value)) === 'hdf5');
  await p.selectOption('#recDlg [data-xo="range"]', 'cursors'); await wait(150);
  check('between the cursors', /--from [\d.]+ --until [\d.]+/.test(await cli()) && /30\.000 s|30 s|30\.0/.test(await p.$eval('#recDlg [data-xo="range"] option[value="cursors"]', o => o.textContent)), await cli());
  await p.click('#recDlg [data-x]'); await wait(150);

  // Save the view as an image
  await more('Save the view as an image…');
  const img = () => text('#recDlg .exfiles code');
  check('PNG of every pane', /\.png$/.test(await img()) && !/-pane-/.test(await img()) && !!(await p.$('#recDlg [data-io="scale"]')), await img());
  await p.selectOption('#recDlg [data-io="what"]', 'sel'); await wait(100);
  check('the selected pane', /-pane-\d+\.png$/.test(await img()), await img());
  await p.selectOption('#recDlg [data-io="format"]', 'svg'); await wait(100);
  check('SVG: no size to choose', /\.svg$/.test(await img()) && !(await p.$('#recDlg [data-io="scale"]')), await img());
  check('saved', JSON.stringify((await settingsLS()).exportImgOpts) === JSON.stringify({ format: 'svg', what: 'sel', scale: 2 }), (await settingsLS()).exportImgOpts);
  await p.click('#recDlg [data-igo]'); await wait(100);
  check('Save (simulated)', /^Saved .+\.svg/.test(await text('#recDlg [data-imsg]')));
});
