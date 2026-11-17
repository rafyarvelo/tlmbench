// The parity harness's scenes. Each puts the prototype and the app in the same state from their first screen (one
// recording, three chart panes) and names the elements to compare.
//   name, about: the scene
//   proto(p), app(p): the steps in each page (Playwright)
//   pairs: [label, prototype selector, app selector, props?]: the last visible match on each side is compared
//   focus: [prototype selector, app selector]: the screenshot is also cropped to this element
//   protoSettings, appSettings: first-launch settings on top of the defaults

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const PROTO_CSV = fileURLToPath(new URL('../../../../prototype/tests/csv/', import.meta.url));

const click = (sel) => async (p) => p.click(sel);
const hoverPane = (p, i) => p.locator('#stack section.pane .phead').nth(i).hover();

/** The pairs every menu scene compares: the menu, an item, a disabled item, a separator, a group heading. */
const menu = (pm, am) => [
  ['menu', pm, am],
  ['item', `${pm} .mi:not(:disabled)`, `${am} .mi:not(:disabled)`],
  ['disabled item', `${pm} .mi:disabled`, `${am} .mi:disabled`],
  ['separator', `${pm} .msep`, `${am} .msep`],
  ['group heading', `${pm} .mgrp`, `${am} .mgrp`],
  ['item icon', `${pm} .mi svg`, `${am} .mi svg`],
];
const PM = '[role=menu]';
const AM = '[role=menu]';

/** Pane 1 as a table, as the pane's view menu does it. */
const tableOpen = (side) => async (p) => {
  await hoverPane(p, 0);
  await p.click(side === 'proto' ? '#stack [data-viewmenu]' : '#stack [data-vmenu]');
  await p.locator('[role=menu] .mi').filter({ hasText: /^Table/ }).first().click();
  await p.waitForSelector('.trw');
};

/** Holds a drag in progress (D-ui-18): a synthetic dragstart on `src`, then dragenter and dragover at a point of `target` given
 *  as fractions of its box (the element under that point gets them, as a real drag). Nothing is dropped, so the outlines
 *  stay up for the screenshot and the measures. */
const holdDrag = (src, target, fx, fy) => async (p) => {
  await p.evaluate(([src, n]) => {
    const s = document.querySelectorAll(src)[n];
    const r = s.getBoundingClientRect();
    window.__parityDT = new DataTransfer();
    // a press first, as a real drag has (the prototype's pane header drags only from a press on its number)
    s.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, clientX: r.x + 4, clientY: r.y + 4 }));
    s.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: window.__parityDT, clientX: r.x + 4, clientY: r.y + 4 }));
  }, Array.isArray(src) ? src : [src, 0]);
  await p.waitForTimeout(120);
  await p.evaluate(([target, fx, fy]) => {
    const t = [...document.querySelectorAll(target)].find((e) => e.getBoundingClientRect().height > 0);
    const r = t.getBoundingClientRect();
    const [x, y] = [r.x + r.width * fx, r.y + r.height * fy];
    const at = document.elementFromPoint(x, y) ?? t;
    for (const type of ['dragenter', 'dragover']) at.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: window.__parityDT, clientX: x, clientY: y }));
  }, [target, fx, fy]);
};
/** The outline of a pane a drag is over: the pane's box, and the label box drawn in its ::after. */
const outline = (pp, ap) => [
  ['pane under the drag', pp, ap, ['border-color', 'box-shadow', 'background-color', 'outline-style', 'outline-color']],
  ['its label box', `${pp}::after`, `${ap}::after`, ['content', 'width', 'height', 'border-width', 'border-style', 'border-color', 'border-radius', 'background-color', 'color', 'font-size', 'font-weight']],
];
/** The first chip of pane 2 (the source of a channel drag), and pane 2's drag handle. */
const PROTO_CHIP = ['#stack section.pane .phead .chip', 2];
const APP_CHIP = ['#stack section.pane .phead .chip', 2];
const PROTO_PANE2 = ['#stack section.pane .phead .pnum', 1];
const APP_PANE2 = ['#stack section.pane .pnum', 1];
const PANE1 = '#stack section.pane';

export const SCENES = [
  {
    name: 'main',
    about: 'The first screen: top bar, Explorer, three chart panes, Add pane and the overview.',
    pairs: [
      ['rail button', '[data-rail=explorer]', '[data-rail=explorer]'],
      ['tab', '#wsTabs .wstab', '#wsTabs .wstab'],
      ['new tab', '#wsTabNew', '#wsTabNew'],
      ['range', '#rangeTxt', '#rangeTxt'],
      ['top bar button', '#b-fit', '#b-fit'],
      ['top bar button (on)', '#m-box', '#m-box'],
      ['top bar icon', '#b-fit svg', '#b-fit svg'],
      ['Explorer heading', '.pcol', '#exToggle'],
      ['filter', '#fileFilter', 'input[aria-label="Filter files and channels"]'],
      ['open file chip', '.exochip', '.ofile'],
      ['channel row', '.chan.on', '#chans .chan.on'],
      ['channel tick', '.chan.on .tog', '#chans .chan.on .box'],
      ['pane', '#stack section.pane:not(.selected)', '#stack section.pane:not(.sel)'],
      ['pane (selected)', '#stack section.pane.selected', '#stack section.pane.sel'],
      ['pane header', '#stack .phead', '#stack .phead'],
      ['channel chip', '#stack .phead .chip', '#stack .phead .chip'],
      ['Add pane', '[data-addp]', '#addPane'],
      ['overview select', '#ovCh', '[data-ovch]'],
    ],
  },
  {
    name: 'more-menu',
    about: 'The top bar\'s ⋯ menu.',
    proto: click('#b-more'),
    app: click('#b-more'),
    pairs: menu(PM, AM),
    focus: [PM, AM],
  },
  {
    name: 'tab-menu',
    about: 'A tab\'s ▾ menu.',
    proto: click('[data-tabm="0"]'),
    app: click('#wsTabs .wstab .tm'),
    pairs: menu('#tabMenu', AM),
    focus: ['#tabMenu', AM],
  },
  {
    name: 'alignment',
    about: 'Two files in one tab: file 2 opened beside the primary, with its alignment control (the prototype lists it in the Files panel; the app in Explorer under slot 2).',
    proto: async (p) => { await p.dblclick('.fitem[data-file="gpslog"] .fname'); await p.waitForSelector('#openFiles [data-falign]'); },
    app: async (p) => {
      if (!(await p.$('#explorer .row.file[data-path$="esc_bench_1412.csv"]'))) await p.click('#explorer .row.dir[data-path$="bench"]');
      await p.click('#explorer .row.file[data-path$="esc_bench_1412.csv"]');
      await p.click('#explorer .row.file.sel', { button: 'right' });
      await p.click('.filemenu [data-fm="add"]');
      await p.click('#explorer .row.slot[data-slot="2"]');
      await p.waitForSelector('.align select');
    },
    pairs: [
      ['alignment select', '#openFiles [data-falign]', '.align select'],
      ['file label', '#openFiles .fopen .fname', '#explorer .row.slot[data-slot="2"] .nm'],
      ['slot number', '.slotn', '.slotn'],
    ],
  },
  {
    name: 'new-tab-menu',
    about: 'The + after the tabs.',
    proto: click('#wsTabNew'),
    app: click('#wsTabNew'),
    pairs: menu('#addMenu', AM),
    focus: ['#addMenu', AM],
  },
  {
    name: 'pane-menu',
    about: 'A pane\'s ⋯ menu.',
    proto: async (p) => { await hoverPane(p, 0); await p.click('#stack [data-pmenu]'); },
    app: async (p) => { await hoverPane(p, 0); await p.click('#stack [data-pmenu]'); },
    pairs: menu(PM, AM),
    focus: [PM, AM],
  },
  {
    name: 'view-menu',
    about: 'A pane\'s view menu (Chart, Table, …).',
    proto: async (p) => { await hoverPane(p, 0); await p.click('#stack [data-viewmenu]'); },
    app: async (p) => { await hoverPane(p, 0); await p.click('#stack [data-vmenu]'); },
    pairs: [...menu(PM, AM), ['checked item', `${PM} .mi[aria-checked=true]`, `${AM} .mi[aria-checked=true]`], ['item note', `${PM} .mi small`, `${AM} .mi small`]],
    focus: [PM, AM],
  },
  {
    name: 'add-pane-menu',
    about: 'The ▾ beside Add pane.',
    proto: click('[data-addpm]'),
    app: click('#addPaneKinds'),
    pairs: [['button', '[data-addpm]', '#addPaneKinds'], ...menu(PM, AM)],
    focus: [PM, AM],
  },
  {
    name: 'explorer-menu',
    about: 'Explorer\'s ⋯ menu.',
    proto: click('#exMore'),
    app: click('#exMore'),
    pairs: menu(PM, AM),
    focus: [PM, AM],
  },
  {
    name: 'folder-menu',
    about: 'Explorer\'s folder menu.',
    proto: click('#exFolder'),
    app: click('#exFolder'),
    pairs: menu(PM, AM),
    focus: [PM, AM],
  },
  {
    name: 'connect',
    about: 'The Connect dialog (Live in the rail): kinds on the left, the form on the right, over a scrim.',
    proto: async (p) => { await p.click('#railLive'); await p.click('#liveDlg [data-kind=tcp]'); },
    app: async (p) => { await p.click('[data-rail=live]'); await p.click('#liveDlg [data-kind=tcp]'); },
    pairs: [
      ['dialog', '#liveDlg', '#liveDlg'],
      ['heading', '#liveDlg > header', '#liveDlg > header'],
      ['kinds', '#liveDlg .lvkinds', '#liveDlg .lvkinds'],
      ['kind', '#liveDlg .lvkind[aria-pressed=false]', '#liveDlg .lvkind[aria-pressed=false]'],
      ['kind (on)', '#liveDlg .lvkind[aria-pressed=true]', '#liveDlg .lvkind[aria-pressed=true]'],
      ['form', '#liveDlg .lvform', '#liveDlg .lvform'],
      ['field label', '#liveDlg .lvf > span', '#liveDlg .lvf > span'],
      ['field', '#liveDlg .lvf .sel', '#liveDlg .lvf .sel'],
      ['footer', '#liveDlg > footer', '#liveDlg > footer'],
      ['button', '#liveDlg > footer .tbtn:not(.primary)', '#liveDlg > footer .tbtn:not(.primary)'],
    ],
    focus: ['#liveDlg', '#liveDlg'],
  },
  {
    name: 'import',
    about: 'The CSV import dialog on a first open (a dropped file): time and rate, the columns list, where the file goes, over a scrim.',
    proto: async (p) => {
      const text = readFileSync(PROTO_CSV + 'AAPL.csv', 'utf8');
      await p.evaluate((t) => {
        const dt = new DataTransfer();
        dt.items.add(new File([t], 'AAPL.csv', { type: 'text/csv' }));
        window.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }));
      }, text);
      await p.waitForSelector('#liveDlg .impmain');
    },
    app: async (p) => {
      await p.evaluate(() => {
        const f = window.__tlmFake;
        f.csv('/data/AAPL.csv', ['Date', 'Open', 'High', 'Low', 'Close', 'Volume'], { unit: 'iso', rate: 1 / 86400, rows: 521 });
        f.files['/data/AAPL.csv'] = {};
        f.drop(['/data/AAPL.csv']);
      });
      await p.waitForSelector('#importDlg .impmain');
    },
    pairs: [
      ['dialog', '#liveDlg', '#importDlg'],
      ['heading', '#liveDlg > header', '#importDlg > header'],
      ['main', '.impmain', '.impmain'],
      ['section heading', '.impmain h4', '.impmain h4'],
      ['note', '.impmain .lvnote', '.impmain .lvnote'],
      ['field label', '.impmain .lvf > span', '.impmain .lvf > span'],
      ['select', '.impmain .lvf .sel', '.impmain .lvf .sel'],
      ['rate options', '.imprate', '.imprate'],
      ['rate option', '.imprate .radio', '.imprate .radio'],
      ['column list', '.implist', '.implist'],
      ['column row', '.impcol', '.impcol:not(.head)'],
      ['column name', '.impcol .imph', '.impcol .imph'],
      ['name input', '.impnm', '.impnm'],
      ['unit input', '.impun', '.impun'],
      ['type select', '.imptype', '.imptype'],
      ['footer', '#liveDlg > footer', '#importDlg > footer'],
      ['button', '#liveDlg > footer .tbtn:not(.primary)', '#importDlg > footer .tbtn:not(.primary)'],
      ['primary button', '#liveDlg > footer .tbtn.primary, #liveDlg [data-go]', '#importDlg > footer .tbtn.primary'],
    ],
    focus: ['#liveDlg', '#importDlg'],
  },
  {
    name: 'descriptor',
    about: 'The descriptor form on a binary file nothing claims, with the ESC logger\'s descriptor picked: framing, time, fields, the hex of the first bytes, the decoded rows and the TOML, over a scrim.',
    proto: async (p) => {
      await p.dblclick('.fitem[data-file="escbin"] .fname');
      await p.waitForSelector('#liveDlg .dscfields');
      await p.selectOption('#liveDlg [data-dpick]', 'esc_telemetry');
      await p.evaluate(() => { document.querySelector('#liveDlg details.dscview').open = true; });
    },
    app: async (p) => {
      await p.evaluate(() => {
        const f = window.__tlmFake;
        f.bin('/data/recordings/bench/esc_bench_0930.bin');
        f.decoders.esc = { schema: 1, name: 'ESC telemetry', files: [], endian: 'little', header_size: 0, record_size: 20, time: { source: 'field', field: 'timestamp_us', unit: 'us' },
          fields: [{ name: 'seq', type: 'u16', offset: 2 }, { name: 'timestamp_us', type: 'u32', offset: 4 }, { name: 'motor_rpm', type: 'u16', offset: 8, unit: 'rpm' }, { name: 'throttle_pct', type: 'u8', offset: 10, unit: '%', scale: 0.5 }, { name: 'status', type: 'u8', offset: 11 }, { name: 'current_a', type: 'i16', offset: 12, unit: 'A', scale: 0.01 }, { name: 'battery_v', type: 'u16', offset: 14, unit: 'V', scale: 0.001 }, { name: 'esc_temp_1', type: 'i8', offset: 16, unit: '°C' }, { name: 'esc_temp_2', type: 'i8', offset: 17, unit: '°C' }],
          packets: { sync: 'A55A', header_size: 2, checksum: 'crc16-ccitt', compression: 'none' } };
        f.drop(['/data/recordings/bench/esc_bench_0930.bin']);
      });
      await p.waitForSelector('#dscDlg .dscfields');
      await p.selectOption('#dscPick', '/cache/decoders/esc.toml');
      await p.evaluate(() => { document.querySelector('#dscDlg details.dscview').open = true; });
      await p.waitForSelector('#dscTable tbody tr');
    },
    pairs: [
      ['dialog', '#liveDlg', '#dscDlg'],
      ['heading', '#liveDlg > header', '#dscDlg > header'],
      ['main', '.impmain', '.impmain'],
      ['section heading', '.impmain h4', '.impmain h4'],
      ['note', '.impmain .lvnote', '.impmain .lvnote'],
      ['field label', '.impmain .lvf > span', '.impmain .lvf > span'],
      ['select', '.impmain .lvf .sel', '.impmain .lvf .sel'],
      ['link button', '.impbar .linkbtn', '.impbar .linkbtn'],
      ['field list', '.dscfields', '.dscfields'],
      ['field list heading', '.dscf.head', '.dscf.head'],
      ['field row', '.dscf:not(.head)', '.dscf:not(.head)'],
      ['field name', '.dscf [data-ff=name]', '.dscf [data-ff=name]'],
      ['field type', '.dscf select', '.dscf select'],
      ['field offset', '.dscf [data-ff=offset]', '.dscf [data-ff=offset]'],
      ['hex', '.dschex', '.dschex'],
      ['hex byte', '.dschex span:not(.at)', '.dschex span:not(.at)'],
      ['rows table', '.dsctab', '.dsctab'],
      ['column heading', '.dsctab th', '.dsctab th'],
      ['cell', '.dsctab td', '.dsctab td'],
      ['TOML', '.dsctoml', '.dsctoml'],
      ['footer', '#liveDlg > footer', '#dscDlg > footer'],
      ['button', '#liveDlg > footer .tbtn:not(.primary)', '#dscDlg > footer .tbtn:not(.primary)'],
      ['primary button', '#liveDlg > footer .tbtn.primary, #liveDlg [data-dopen]', '#dscDlg > footer .tbtn.primary'],
    ],
    focus: ['#liveDlg', '#dscDlg'],
  },
  {
    name: 'axis-dialog',
    about: 'Axis and scale… from a pane\'s ⋯ menu.',
    proto: async (p) => { await hoverPane(p, 0); await p.click('#stack [data-pmenu]'); await p.click('[role=menu] .mi:has-text("Axis and scale")'); },
    app: async (p) => { await hoverPane(p, 0); await p.click('#stack [data-pmenu]'); await p.click('[role=menu] .mi:has-text("Axis and scale")'); },
    pairs: [
      ['dialog', '[role=dialog]', '[role=dialog]'],
      ['heading', '[role=dialog] header', '[role=dialog] header'],
      ['input', '[role=dialog] input[type=number], [role=dialog] input[type=text]', '[role=dialog] input[type=number], [role=dialog] input[type=text]'],
      ['button', '[role=dialog] button', '[role=dialog] button'],
    ],
    focus: ['[role=dialog]', '[role=dialog]'],
  },
  {
    name: 'tooltip',
    about: 'Hovering Fit: the button\'s hover state and its tooltip.',
    proto: async (p) => { await p.hover('#b-fit'); await p.waitForTimeout(900); },
    app: async (p) => { await p.hover('#b-fit'); await p.waitForTimeout(900); },
    pairs: [
      ['button (hover)', '#b-fit', '#b-fit'],
      ['tooltip', '#tip', '.tip[role=tooltip]'],
    ],
    focus: ['#b-fit', '#b-fit'],
  },
  {
    name: 'channel-hover',
    about: 'Hovering a channel in Explorer.',
    proto: (p) => p.hover('.chan.on'),
    app: (p) => p.hover('#chans .chan.on'),
    pairs: [
      ['row (hover)', '.chan.on:hover', '#chans .chan.on:hover'],
      ['tick', '.chan.on:hover .tog', '#chans .chan.on:hover .box'],
      ['name', '.chan.on:hover .nm', '#chans .chan.on:hover .nm'],
    ],
    focus: ['.chan.on:hover', '#chans .chan.on:hover'],
  },
  {
    name: 'pane-hover',
    about: 'Hovering a pane\'s header shows its controls.',
    proto: (p) => hoverPane(p, 1),
    app: (p) => hoverPane(p, 1),
    pairs: [
      ['view button', '#stack [data-viewmenu]', '#stack [data-vmenu]'],
      ['⋯ button', '#stack [data-pmenu]', '#stack [data-pmenu]'],
      ['chip remove', '#stack .phead .chip .x, #stack .phead [data-rm]', '#stack .phead .chip .x, #stack .phead [data-rm]'],
    ],
    focus: ['#stack .phead:hover', '#stack .phead:hover'],
  },
  {
    name: 'table',
    about: 'A pane as a table: header, filter row, rows, the bar above.',
    proto: tableOpen('proto'),
    app: tableOpen('app'),
    pairs: [
      ['table bar', '.tbar', '.tbar'],
      ['status', '.tbar .tstat', '.tbar .tstat'],
      ['bar button', '.tbar .tbtn', '.tbar .tbtn'],
      ['header cell', '.thead .tth, .tth', '.thead .tth'],
      ['header name', '.tth .tlab', '.tth .tlab'],
      ['filter box', '.tfl input', '.tfin'],
      ['row', '.trw', '.trw'],
      ['cell', '.trw > span', '.trw > span'],
    ],
    focus: ['.twrap', '.ptable'],
  },
  {
    name: 'table-columns',
    about: 'The Columns popover of a table.',
    proto: async (p) => { await tableOpen('proto')(p); await p.click('[data-cols]'); },
    app: async (p) => { await tableOpen('app')(p); await p.click('[data-cols]'); },
    pairs: [
      ['popover', '.colpop', '.colpop'],
      ['heading', '.colpop header', '.colpop .cphead'],
      ['row', '.colpop .colrow', '.colpop .colrow'],
      ['footer button', '.colpop footer button', '.colpop .cpfoot button'],
    ],
    focus: ['.colpop', '.colpop'],
  },
  {
    name: 'table-autofit',
    about: 'The Autofit menu of a table.',
    proto: async (p) => { await tableOpen('proto')(p); await p.click('[data-autofit]'); },
    app: async (p) => { await tableOpen('app')(p); await p.click('[data-autofit]'); },
    pairs: menu('.colmenu', '.colmenu'),
    focus: ['.colmenu', '.colmenu'],
  },
  {
    name: 'table-filter',
    about: 'A table with a filter typed: the strip under the bar.',
    proto: async (p) => { await tableOpen('proto')(p); await p.locator('.tfl input').nth(2).fill('> 0.5'); },
    app: async (p) => { await tableOpen('app')(p); await p.locator('[data-filter]').nth(1).fill('> 0.5'); },
    pairs: [
      ['strip', '.tfilt', '.tfilt'],
      ['scope button', '.tfilt .fb, .tfilt button', '.tfilt .fb'],
      ['match count', '.tfilt .fpos, .tfilt span', '.tfilt .fpos'],
      ['filter box', '.tfl input', '.tfin'],
    ],
    focus: ['.ptable, .tbar', '.ptable'],
  },
  {
    name: 'settings',
    about: 'Settings from the rail.',
    proto: click('#b-settings'),
    app: click('#b-settings'),
    pairs: [
      ['panel', '[role=dialog][aria-label*=ettings], .setpop, #setPanel', '[role=dialog][aria-label*=ettings], .setpop, #setPanel'],
      ['select', '.setpop select, #setPanel select', '.setpop select, #setPanel select'],
    ],
  },
  {
    name: 'start',
    about: 'The start page from the rail\'s logo.',
    proto: click('#railHome'),
    app: click('#b-start'),
    pairs: [
      ['title', '.sthead h1, .sthero h1', '.start.overlay h1'],
      ['card', '.stcard', '.start.overlay .stcard'],
      ['row', '.strow', '.start.overlay .strow'],
    ],
  },
  {
    name: 'drag-edge',
    about: 'A channel chip of pane 2 held over the left edge of pane 1: "Place on the left" (D-ui-18).',
    proto: holdDrag(PROTO_CHIP, PANE1, 0.08, 0.6),
    app: holdDrag(APP_CHIP, PANE1, 0.08, 0.6),
    pairs: outline('#stack section.pane.over-left', '#stack section.pane[data-side="left"]'),
    focus: ['#stack section.pane.over-left', '#stack section.pane[data-side="left"]'],
  },
  {
    name: 'drag-edge-right',
    about: 'The same chip over the right edge: "Place on the right".',
    proto: holdDrag(PROTO_CHIP, PANE1, 0.92, 0.6),
    app: holdDrag(APP_CHIP, PANE1, 0.92, 0.6),
    pairs: outline('#stack section.pane.over-right', '#stack section.pane[data-side="right"]'),
    focus: ['#stack section.pane.over-right', '#stack section.pane[data-side="right"]'],
  },
  {
    name: 'drag-into',
    about: 'The chip over the middle of pane 1: it joins that pane.',
    proto: holdDrag(PROTO_CHIP, PANE1, 0.5, 0.6),
    app: holdDrag(APP_CHIP, PANE1, 0.5, 0.6),
    pairs: [['pane under the drag', '#stack section.pane.over', '#stack section.pane.chanover', ['border-color', 'box-shadow', 'background-color', 'outline-style', 'outline-color']]],
    focus: ['#stack section.pane.over', '#stack section.pane.chanover'],
  },
  {
    name: 'drag-gap',
    about: 'The chip over the gap before pane 1: "Drop here for a new pane", with the other gaps shown.',
    proto: holdDrag(PROTO_CHIP, '.dropgap', 0.5, 0.5),
    app: holdDrag(APP_CHIP, '.dropgap', 0.5, 0.5),
    pairs: [
      ['gap under the drag', '.dropgap.over', '.dropgap.over'],
      ['another gap', '.dropgap:not(.over)', '.dropgap:not(.over)'],
    ],
    focus: ['#stack', '#stack'],
  },
  {
    name: 'drag-pane-top',
    about: 'Pane 2 dragged by its header (the app: its number) over the top of pane 1: "Move to a new row above".',
    proto: holdDrag(PROTO_PANE2, PANE1, 0.5, 0.12),
    app: holdDrag(APP_PANE2, PANE1, 0.5, 0.12),
    pairs: outline('#stack section.pane.over-top', '#stack section.pane[data-zone="top"]'),
    focus: ['#stack', '#stack'],
  },
  {
    name: 'drag-pane-merge',
    about: 'Pane 2 dragged over the middle of pane 1: "Merge into this pane".',
    proto: holdDrag(PROTO_PANE2, PANE1, 0.5, 0.5),
    app: holdDrag(APP_PANE2, PANE1, 0.5, 0.5),
    pairs: [...outline('#stack section.pane.over-merge', '#stack section.pane[data-zone="middle"]'), ['gap during a pane drag', '.dropgap', '.dropgap', ['content', 'font-size', 'color', 'border-color']]],
    focus: ['#stack', '#stack'],
  },
];
