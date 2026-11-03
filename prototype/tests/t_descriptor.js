// Format descriptors: a binary file no descriptor claims opens the form on its own bytes; Find frames, the preview
// (hex, decoded rows, checksums), problems worded as the decoder words them, the TOML, Save, Share and Open; the
// descriptor then claims the file (and its siblings) after a reload.
const T = require('./lib');
T.run('descriptor', async ({ p, check, text, tabs, wait, until, menu, pick, settingsLS }) => {
  const dlg = '#liveDlg';
  check('a binary file without a descriptor says so in Explorer', /no descriptor/.test(await text('.fitem[data-file="escbin"]')), await text('.fitem[data-file="escbin"]'));
  await p.dblclick('.fitem[data-file="escbin"] .fname'); await wait(300);
  check('opening it opens the form on the file', /Describe esc_bench_0930\.bin/.test(await text(`${dlg} header`)), await text(`${dlg} header`));
  check('a new descriptor names the files like this one', await p.evaluate(() => document.querySelector('#liveDlg [data-ds="files"]').value) === 'esc_bench_*.bin');
  const bytes0 = await p.evaluate(() => [...document.querySelectorAll('#liveDlg .dschex > div:first-child span:not(.at)')].slice(0, 4).map(e => e.textContent));
  check('the preview shows the file\'s first bytes', bytes0.join(' ') === 'A5 5A 00 00', bytes0);

  // Find frames: the sync word repeats every 24 bytes
  await p.click(`${dlg} [data-dfind]`); await wait(200);
  check('Find frames finds the sync word and the frame length', /Sync A55A every 24 bytes/.test(await text(`${dlg} [data-dfound]`)), await text(`${dlg} [data-dfound]`));
  check('and switches to packets', await p.evaluate(() => document.querySelector('#liveDlg [data-ds="framing"]').value) === 'packets');

  // a descriptor you have: the ESC logger's, from its live connection
  await p.selectOption(`${dlg} [data-dpick]`, 'esc_telemetry'); await wait(200);
  const rows = () => p.evaluate(() => [...document.querySelectorAll('#liveDlg .dsctab tbody tr')].map(r => [...r.cells].map(c => c.textContent)));
  const r1 = await rows();
  const head = await p.evaluate(() => [...document.querySelectorAll('#liveDlg .dsctab th')].map(c => c.textContent));
  check('picking a descriptor decodes the preview', r1.length === 8 && head.includes('motor_rpm (rpm)') && head.includes('battery_v (V)'), [head, r1[0]]);
  const rpm = head.indexOf('motor_rpm (rpm)'), bat = head.indexOf('battery_v (V)');
  check('values come out scaled and plausible', +r1[0][rpm] >= 1200 && +r1[0][rpm] < 4000 && +r1[0][bat] > 20 && +r1[0][bat] < 26, r1[0]);
  check('time from the µs field, in seconds', r1[0][0] === '120 s' && r1[1][0] === '120.01 s', [r1[0][0], r1[1][0]]);
  check('every checksum checks', r1.every(r => r[r.length - 1] === 'ok') && /checksums ok/.test(await text(`${dlg} h4:nth-of-type(4)`)), await text(`${dlg} h4:nth-of-type(4)`));
  check('a picked descriptor that claims no files is offered for this name', await p.evaluate(() => document.querySelector('#liveDlg [data-ds="files"]').value) === 'esc_bench_*.bin');

  // a wrong byte order garbles the values and fails the checksums
  await p.selectOption(`${dlg} [data-ds="endian"]`, 'big'); await wait(200);
  const r2 = await rows();
  check('the wrong byte order shows in the values and the checksums', r2.every(r => r[r.length - 1] === 'bad') && r2[0][rpm] !== r1[0][rpm], r2[0]);
  await p.selectOption(`${dlg} [data-ds="endian"]`, 'little'); await wait(200);

  // a field past the record is a problem, worded as the decoder words it, and Open waits for a fix
  const bv = await p.evaluate(() => [...document.querySelectorAll('#liveDlg [data-fi]')].findIndex(r => r.querySelector('[data-ff="name"]').value === 'battery_v'));
  await p.fill(`${dlg} [data-fi="${bv}"] [data-ff="offset"]`, '21'); await p.press(`${dlg} [data-fi="${bv}"] [data-ff="offset"]`, 'Tab'); await wait(200);
  check('a field past the record is named', /battery_v \(u16 at offset 21\) runs past the 22-byte packet header and record/.test(await text(`${dlg} .dscerr`)), await text(`${dlg} .dscerr`));
  check('Open waits for a fix', await p.evaluate(() => document.querySelector('#liveDlg [data-dopen]').disabled));
  await p.fill(`${dlg} [data-fi="${bv}"] [data-ff="offset"]`, '14'); await p.press(`${dlg} [data-fi="${bv}"] [data-ff="offset"]`, 'Tab'); await wait(200);

  // + Add field goes after the last one
  await p.click(`${dlg} [data-fadd]`); await wait(200);
  const added = await p.evaluate(() => { const r = [...document.querySelectorAll('#liveDlg [data-fi]')].pop(); return [r.querySelector('[data-ff="name"]').value, r.querySelector('[data-ff="offset"]').value]; });
  check('+ Add field starts after the last field', added[0] === 'field_10' && added[1] === '18', added);
  await p.evaluate(() => [...document.querySelectorAll('#liveDlg [data-fi]')].pop().querySelector('[data-fdel]').click()); await wait(200);

  // the TOML the decoders read
  await p.evaluate(() => { const d = document.querySelector('#liveDlg details.dscview'); d.open = true; }); await wait(100);
  const toml = await p.evaluate(() => document.querySelector('#liveDlg [data-dtoml]').textContent);
  check('the TOML has the framing, the time and the fields', /files = \["esc_bench_\*\.bin"\]/.test(toml) && /\[packets\]\nsync = "A55A"\nheader_size = 2\nchecksum = "crc16-ccitt"/.test(toml)
    && /\[time\]\nsource = "field"\nfield = "timestamp_us"\nunit = "us"/.test(toml) && /name = "battery_v"\ntype = "u16"\noffset = 14\nunit = "V"\nscale = 0\.001/.test(toml) && !/length =/.test(toml), toml);

  // Share… writes it to .tlmbench/decoders/
  await p.click(`${dlg} [data-dshare]`); await wait(200);
  check('Share… previews .tlmbench/decoders/esc-telemetry.toml', /\.tlmbench\/decoders\/esc-telemetry\.toml/.test(await text('#recDlg')), await text('#recDlg'));
  await p.click('#recDlg [data-shgo]'); await wait(200);
  check('Share to folder records it (simulated)', Object.keys((await settingsLS()).sharedFiles || {}).includes('.tlmbench/decoders/esc-telemetry.toml'));
  await p.click('#recDlg [data-x]'); await wait(100);

  // Open: the file opens through it, and the descriptor is saved with the files it claims
  await p.click(`${dlg} [data-dopen]`); await wait(2500);
  check('Open opens the file in its tab', (await tabs()).some(t => /esc_bench_0930\.bin\*/.test(t)), await tabs());
  const saved = ((await settingsLS()).decoders || []).find(d => d.id === 'esc_telemetry');
  check('the descriptor is saved with the files it claims', saved && saved.files.includes('esc_bench_*.bin') && saved.fields.length === 9, saved);
  check('its fields are the file\'s channels (the time field aside)', /motor_rpm/.test(await text('#fileTree')) && /battery_v/.test(await text('#fileTree')) && !/timestamp_us/.test(await text('#fileTree')), await text('#fileTree'));
  check('Explorer no longer asks for one', !/no descriptor/.test(await text('.fitem[data-file="escbin"]')));

  // after a reload the descriptor still claims the file: it opens straight away
  await p.reload(); await wait(1500);
  await p.dblclick('.fitem[data-file="escbin"] .fname'); await wait(1500);
  const after = await p.evaluate(() => ({ hidden: document.getElementById('liveDlg').hidden, head: (document.querySelector('#liveDlg header') || {}).textContent, rows: [...document.querySelectorAll('.fitem[data-file="escbin"]')].map(e => e.className) }));
  check('after a reload the file opens without the form', after.hidden && (await tabs()).some(t => /esc_bench_0930\.bin/.test(t)), [after, await tabs()]);
  // right-click → Format descriptor… edits it
  await p.evaluate(() => { const r = document.querySelector('.fitem[data-file="escbin"]').getBoundingClientRect(); document.querySelector('.fitem[data-file="escbin"]').dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left + 20, clientY: r.top + 5 })); }); await wait(200);
  check('the file menu edits its descriptor', (await menu()).includes('Format descriptor…'), await menu());
  await pick(/^Format descriptor…$/); await wait(300);
  check('editing opens the form with its descriptor', await p.evaluate(() => document.querySelector('#liveDlg [data-dpick]').value) === 'esc_telemetry');
});
