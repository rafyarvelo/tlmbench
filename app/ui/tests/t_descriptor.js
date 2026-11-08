// The descriptor form (ported from the prototype's t_descriptor): a binary file no descriptor claims says so in
// Explorer and opens the form on its own bytes; Find frames, the core's preview (hex, decoded rows, checksums,
// problems worded as the decoder words them), the TOML, bitfields and a counter time base in the descriptor the form
// sends, Save, Share… (asks first) and Open; the descriptor then claims the file, and the file menu edits it.
import { run } from './lib.js';

const DIR = '/data/logs';
const BIN = `${DIR}/esc_bench_0930.bin`;
const ESC = {
  schema: 1, name: 'ESC telemetry', files: [], endian: 'little', header_size: 0, record_size: 20,
  time: { source: 'field', field: 'timestamp_us', unit: 'us' },
  fields: [
    { name: 'seq', type: 'u16', offset: 2 }, { name: 'timestamp_us', type: 'u32', offset: 4 },
    { name: 'motor_rpm', type: 'u16', offset: 8, unit: 'rpm' }, { name: 'throttle_pct', type: 'u8', offset: 10, unit: '%', scale: 0.5 },
    { name: 'status', type: 'u8', offset: 11 }, { name: 'current_a', type: 'i16', offset: 12, unit: 'A', scale: 0.01 },
    { name: 'battery_v', type: 'u16', offset: 14, unit: 'V', scale: 0.001 }, { name: 'esc_temp_1', type: 'i8', offset: 16, unit: '°C' },
    { name: 'esc_temp_2', type: 'i8', offset: 17, unit: '°C' },
  ],
  packets: { sync: 'A55A', header_size: 2, checksum: 'crc16-ccitt', compression: 'none' },
};

run(
  'descriptor',
  async ({ p, check, text, until, wait, fake, visible }) => {
    const dlg = '#dscDlg';
    const tabs = () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent));
    const rows = () => p.evaluate(() => [...document.querySelectorAll('#dscTable tbody tr')].map((r) => [...r.cells].map((c) => c.textContent)));
    const last = () => fake(() => window.__tlmFake.previewed.at(-1));
    await fake(([bin, esc]) => {
      const f = window.__tlmFake;
      f.bin(bin);
      f.decoders['esc-telemetry'] = esc;
    }, [BIN, ESC]);

    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    const row = '#explorer .row.file[data-path$="esc_bench_0930.bin"]';
    check('a binary file without a descriptor says so in Explorer', await until(() => /no descriptor/.test(document.querySelector('#explorer .row.file')?.textContent ?? '')), await text('#explorer .row.file'));
    await p.dblclick(row);
    check('opening it opens the form on the file', await until(() => /Describe esc_bench_0930\.bin/.test(document.querySelector('#dscDlg header')?.textContent ?? '')), await text(`${dlg} header`));
    check('no tab opened', !(await tabs()).length);
    check('a new descriptor names the files like this one', (await p.inputValue('#dscFiles')) === 'esc_bench_*.bin');
    const first = () => p.evaluate(() => [...document.querySelectorAll('#dscHex > div:first-child span:not(.at)')].slice(0, 4).map((e) => e.textContent).join(' '));
    check("the preview shows the file's first bytes", await until(() => document.querySelector('#dscHex > div span:not(.at)')?.textContent === 'A5'), await first());
    check('A55A 0000', (await first()) === 'A5 5A 00 00', await first());

    // Find frames: the sync word repeats every 24 bytes
    await p.click('#dscFind');
    check('Find frames finds the sync word and the frame length', await until(() => /Sync A55A every 24 bytes/.test(document.querySelector('#dscFound')?.textContent ?? '')), await text('#dscFound'));
    check('and switches to packets', (await p.inputValue('#dscFraming')) === 'packets');
    check('the record size follows: the frame less its header', (await p.inputValue('#dscSize')) === '22', await p.inputValue('#dscSize'));

    // a descriptor you have: the ESC logger's
    await p.selectOption('#dscPick', '/cache/decoders/esc-telemetry.toml');
    check('picking a descriptor decodes the preview', await until(() => document.querySelectorAll('#dscTable tbody tr').length === 8 && /motor_rpm/.test(document.querySelector('#dscTable thead')?.textContent ?? '')));
    const head = await p.evaluate(() => [...document.querySelectorAll('#dscTable th')].map((c) => c.textContent));
    const r1 = await rows();
    check('its fields are the columns, the time field aside', head.includes('motor_rpm (rpm)') && head.includes('battery_v (V)') && !head.includes('timestamp_us'), head);
    const rpm = head.indexOf('motor_rpm (rpm)');
    const bat = head.indexOf('battery_v (V)');
    check('values come out scaled and plausible', +r1[0][rpm] >= 1200 && +r1[0][rpm] < 4000 && +r1[0][bat] > 20 && +r1[0][bat] < 26, r1[0]);
    check('time from the µs field, in seconds', r1[0][0] === '120 s' && r1[1][0] === '120.01 s', [r1[0][0], r1[1][0]]);
    check('every checksum checks', r1.every((r) => r.at(-1) === 'ok') && /checksums ok/.test(await text('.dscdlg h4:nth-of-type(4)')), await text('.dscdlg h4:nth-of-type(4)'));
    check('a picked descriptor that claims no files is offered for this name', (await p.inputValue('#dscFiles')) === 'esc_bench_*.bin');

    // a wrong byte order garbles the values and fails the checksums
    await p.selectOption('#dscEndian', 'big');
    check('the wrong byte order shows in the values and the checksums', await until(() => [...document.querySelectorAll('#dscTable tbody tr')].every((r) => r.cells[r.cells.length - 1].textContent === 'bad')));
    check('values differ', (await rows())[0][rpm] !== r1[0][rpm]);
    await p.selectOption('#dscEndian', 'little');
    await until(() => [...document.querySelectorAll('#dscTable tbody tr')].every((r) => r.cells[r.cells.length - 1].textContent === 'ok'));

    // a field past the record is a problem, worded as the core words it, and Open waits for a fix
    const bv = await p.evaluate(() => [...document.querySelectorAll('#dscDlg [data-fi]')].findIndex((r) => r.querySelector('[data-ff="name"]').value === 'battery_v'));
    await p.fill(`[data-fi="${bv}"] [data-ff="offset"]`, '21');
    await p.press(`[data-fi="${bv}"] [data-ff="offset"]`, 'Tab');
    check('a field past the record is named', await until(() => /field "battery_v" \(u16 at offset 21\) runs past the 22-byte packet header and record/.test(document.querySelector('#dscErr')?.textContent ?? '')), await text('#dscErr'));
    check('that field is marked', await p.$eval(`[data-fi="${bv}"]`, (e) => e.classList.contains('bad')));
    check('Open, Save and Share wait for a fix', (await p.isDisabled('#dscOpen')) && (await p.isDisabled('#dscSave')) && (await p.isDisabled('#dscShare')));
    await p.fill(`[data-fi="${bv}"] [data-ff="offset"]`, '14');
    await p.press(`[data-fi="${bv}"] [data-ff="offset"]`, 'Tab');
    check('a fix brings the rows back', await until(() => document.querySelectorAll('#dscTable tbody tr').length === 8 && !document.querySelector('#dscErr')));

    // + Add field goes after the last one
    await p.click('#dscAdd');
    const added = await p.evaluate(() => { const r = [...document.querySelectorAll('#dscDlg [data-fi]')].pop(); return [r.querySelector('[data-ff="name"]').value, r.querySelector('[data-ff="offset"]').value]; });
    check('+ Add field starts after the last field', added.join() === 'field_10,18', added);
    await p.evaluate(() => [...document.querySelectorAll('#dscDlg [data-fi]')].pop().querySelector('[data-fdel]').click());
    check('and × removes it', (await p.$$('#dscDlg [data-fi]')).length === 9);

    // a bitfield: the descriptor the form sends has it, and text that isn't a bit range is left alone
    const st = await p.evaluate(() => [...document.querySelectorAll('#dscDlg [data-fi]')].findIndex((r) => r.querySelector('[data-ff="name"]').value === 'status'));
    await p.fill(`[data-fi="${st}"] [data-ff="bits"]`, '2-4');
    await p.press(`[data-fi="${st}"] [data-ff="bits"]`, 'Tab');
    check('bits 2-4 are sent as a shift and a width', await until(() => window.__tlmFake.previewed.at(-1)?.fields?.[4]?.bits?.width === 3), await last());
    check('as shift 2', (await last()).fields[4].bits.shift === 2);
    await p.fill(`[data-fi="${st}"] [data-ff="bits"]`, 'lots');
    await p.press(`[data-fi="${st}"] [data-ff="bits"]`, 'Tab');
    check('text that is no bit range goes back to what it was', (await p.inputValue(`[data-fi="${st}"] [data-ff="bits"]`)) === '2-4');
    await p.fill(`[data-fi="${st}"] [data-ff="bits"]`, '');
    await p.press(`[data-fi="${st}"] [data-ff="bits"]`, 'Tab');
    check('a blank takes all the bits', await until(() => window.__tlmFake.previewed.at(-1)?.fields?.[4]?.bits === undefined));
    check('a float has no bits', await p.evaluate(() => { const r = document.querySelector('#dscDlg [data-fi]'); return !r.querySelector('[data-ff="bits"]').disabled; }));

    // a counter that wraps as the time base
    await p.selectOption('#dscTime', 'counter');
    check('Counter asks for its clock and range', await until(() => !!document.querySelector('#dscClock') && !!document.querySelector('#dscWrap')));
    await p.selectOption('#dscTimeField', 'seq');
    await p.fill('#dscClock', '1000');
    await p.press('#dscClock', 'Tab');
    await p.fill('#dscWrap', '65536');
    await p.press('#dscWrap', 'Tab');
    check('the descriptor sent has the counter', await until(() => { const t = window.__tlmFake.previewed.at(-1)?.time; return t?.source === 'counter' && t.field === 'seq' && t.clock_hz === 1000 && t.wraps_at === 65536; }), (await last()).time);
    await p.selectOption('#dscTime', 'field');
    await p.selectOption('#dscTimeField', 'timestamp_us');
    check('back to the field', await until(() => { const t = window.__tlmFake.previewed.at(-1)?.time; return t?.source === 'field' && t.field === 'timestamp_us'; }));

    // the TOML the core writes
    await p.evaluate(() => { document.querySelector('#dscDlg details.dscview').open = true; });
    check('the TOML is the core\'s', await until(() => /name = "ESC telemetry"/.test(document.querySelector('#dscToml')?.textContent ?? '') && /sync = "A55A"/.test(document.querySelector('#dscToml')?.textContent ?? '')), await text('#dscToml'));

    // Share… asks, then writes to .tlmbench/decoders/
    await p.click('#dscShare');
    check('Share… asks before it writes beside the data', await visible('#dscShareAsk') && /\.tlmbench\/decoders\/esc-telemetry\.toml/.test(await text('#dscShareAsk')), await text('#dscShareAsk'));
    check('nothing is written yet', (await fake((d) => Object.keys(window.__tlmFake.sharedDecoders[d] ?? {}).length, DIR)) === 0);
    await p.click('#dscShareGo');
    check('Write it shares it with the folder', await until((d) => !!window.__tlmFake.sharedDecoders[d]?.['esc-telemetry'], DIR));
    check('and says so', await until(() => /Shared as \.tlmbench\/decoders\/esc-telemetry\.toml/.test(document.body.innerText)));

    // a read-only folder: the reason is shown in the form
    await fake((d) => window.__tlmFake.readOnly.push(d), DIR);
    await p.click('#dscShare');
    await p.click('#dscShareGo');
    check('a folder that can\'t be written says why', await until(() => /Read-only file system/.test(document.querySelector('#dscFail')?.textContent ?? '')), await text('#dscFail'));
    await fake(() => window.__tlmFake.readOnly.pop());

    // Open: saved to the cache with the files it claims, and the file opens through it
    await p.click('#dscOpen');
    check('Open opens the file in its tab', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => /esc_bench_0930\.bin/.test(n.textContent))), await tabs());
    check('the form is gone', !(await visible('#dscDlg')));
    const saved = await fake(() => window.__tlmFake.decoders['esc-telemetry']);
    check('the descriptor is saved with the files it claims', saved?.files?.includes('esc_bench_*.bin') && saved.fields.length === 9, saved);
    check("its fields are the file's channels", await until(() => /motor_rpm/.test(document.querySelector('#explorer')?.innerText ?? '')));

    // the descriptor claims the file: it opens straight away, and the file menu edits the descriptor
    await p.click(row, { button: 'right' });
    await p.click('.filemenu [data-fm="close"]');
    check('closed', await until(() => ![...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => /esc_bench/.test(n.textContent))));
    check('Explorer no longer asks for a descriptor', !/no descriptor/.test(await text(row)), await text(row));
    await p.dblclick(row);
    await wait(400);
    check('it opens without the form', !(await visible('#dscDlg')));
    await p.click(row, { button: 'right' });
    check('the file menu offers Format descriptor…', !(await p.isDisabled('.filemenu [data-fm="descriptor"]')));
    await p.click('.filemenu [data-fm="descriptor"]');
    check('editing opens the form with its descriptor', await until(() => /esc-telemetry\.toml$/.test(document.querySelector('#dscPick')?.value ?? '')), await p.inputValue('#dscPick'));
    check('the folder\'s shared copy is the one that names the file', /\.tlmbench\/decoders\//.test(await p.inputValue('#dscPick')), await p.inputValue('#dscPick'));
    check('and its fields', (await p.$$('#dscDlg [data-fi]')).length === 9);
    await p.keyboard.press('Escape');
    check('Escape closes it', !(await visible('#dscDlg')));
  },
  { files: { '/data/logs/other.csv': { channels: ['a', 'b'] } }, launch: null },
);
