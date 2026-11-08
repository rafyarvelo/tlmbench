// Save the view as an image…: from the ⋯ menu's Export group; PNG (as on screen or twice as sharp) or SVG, every pane or
// the selected one, written where the person chooses. The SVG holds the axes the screen draws, each trace as a line, the
// cursors and a legend; the PNG is a real PNG of the panes. Choices are remembered.
import { run } from './lib.js';

run(
  'image',
  async ({ p, check, text, until, fake, visible, settingsLS }) => {
    const saved = () => fake(() => window.__tlmFake.saved_files.map((f) => ({ path: f.path, n: f.bytes.length, head: f.bytes.slice(0, 8), text: f.bytes[0] === 60 ? new TextDecoder().decode(new Uint8Array(f.bytes)) : '' })));
    await p.click('#b-more');
    await p.click('.moremenu [data-more="exportimage"]');
    check('the dialog opens', await until(() => !!document.querySelector('#imageDlg')));
    check('every pane by default, as a PNG', (await p.inputValue('#imWhat')) === 'all' && (await p.inputValue('#imFormat')) === 'png' && /3 panes/.test(await text('.exfiles')), await text('.exfiles'));
    check('named after the recording', /flight\.png/.test(await text('.exfiles')), await text('.exfiles'));

    await fake(() => (window.__tlmFake.savePick = '/out/flight.png'));
    await p.click('#imGo');
    check('Save writes a PNG where chosen', await until(() => window.__tlmFake.saved_files.length === 1));
    let f = (await saved())[0];
    check('at the path chosen', f.path === '/out/flight.png', f.path);
    check('with the PNG signature', f.head.join() === '137,80,78,71,13,10,26,10', f.head);
    check('and says so', await until(() => /Saved flight\.png \(3 panes\)/.test(document.querySelector('#imDone')?.textContent ?? '')), await text('#imDone'));

    // the selected pane as SVG
    await p.selectOption('#imFormat', 'svg');
    await p.selectOption('#imWhat', 'sel');
    check('the name follows the choices', /flight-pane-1\.svg/.test(await text('.exfiles')), await text('.exfiles'));
    check('the size only matters for PNG', !(await visible('#imScale')));
    await fake(() => (window.__tlmFake.savePick = '/out/flight-pane-1.svg'));
    await p.click('#imGo');
    check('Save writes the SVG', await until(() => window.__tlmFake.saved_files.length === 2));
    f = (await saved())[1];
    check('an SVG document', /^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg"/.test(f.text), f.text.slice(0, 80));
    check('with the axes (grid lines and value labels)', /<path d="M[\d.]+ [\d.]+L[\d.]+ [\d.]+"/.test(f.text) && /<text [^>]*text-anchor="end"/.test(f.text));
    check('a line for each trace of the pane', (f.text.match(/<polyline/g) ?? []).length === 2, (f.text.match(/<polyline/g) ?? []).length);
    check('the legend names the channels', /accel_x/.test(f.text) && /accel_y/.test(f.text));
    check('one pane only', !/roll/.test(f.text));

    // every pane as SVG; cursors appear
    await p.selectOption('#imWhat', 'all');
    await p.click('#imGo');
    await until(() => window.__tlmFake.saved_files.length === 3);
    f = (await saved())[2];
    check('every pane: six traces', (f.text.match(/<polyline/g) ?? []).length === 6, (f.text.match(/<polyline/g) ?? []).length);

    check('the choices are remembered', (await settingsLS()).exportImage?.format === 'svg' && (await settingsLS()).exportImage?.what === 'all');
    // cancelling the file dialog writes nothing
    await fake(() => (window.__tlmFake.savePick = null));
    await p.click('#imGo');
    await p.waitForTimeout(300);
    check('cancelling writes nothing', (await saved()).length === 3);
    await p.keyboard.press('Escape');
    check('Escape closes it', !(await visible('#imageDlg')));
  },
);
