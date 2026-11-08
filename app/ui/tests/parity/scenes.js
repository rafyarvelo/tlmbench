// The parity harness's scenes. Each puts the prototype and the app in the same state from their first screen (one
// recording, three chart panes) and names the elements to compare.
//   name, about: the scene
//   proto(p), app(p): the steps in each page (Playwright)
//   pairs: [label, prototype selector, app selector, props?]: the last visible match on each side is compared
//   focus: [prototype selector, app selector]: the screenshot is also cropped to this element
//   protoSettings, appSettings: first-launch settings on top of the defaults

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
    app: click('#exFolderMenu'),
    pairs: menu(PM, AM),
    focus: [PM, AM],
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
];
