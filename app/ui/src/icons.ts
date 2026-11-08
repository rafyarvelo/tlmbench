// Icons from the prototype (prototype/plotter.html), as SVG markup for {@html}: the same paths and stroke widths, so
// `tests/parity/icons.mjs` finds them equal. Sizes are set per use, as the prototype's CSS sets them.

const s = (body: string, extra = 'fill="none" stroke="currentColor" stroke-width="2"') => `<svg viewBox="0 0 24 24" ${extra} aria-hidden="true">${body}</svg>`;

export const ICON = {
  logo: s(
    '<path d="M4 3v17h17" stroke="var(--fg-muted)" stroke-width="1.75"/><path d="M7 14.5c1.1-4 2.2-6 3.3-6s2 6 3.3 6 2.1-4.5 3.3-4.5 1.6 1.4 2.4 2.4" stroke="var(--accent)" stroke-width="2"/>',
    'fill="none" stroke-linecap="round" stroke-linejoin="round"',
  ),
  explorer: s('<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4M9 12h6M9 16h6"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"'),
  live: s('<circle cx="12" cy="12" r="2.5"/><path d="M7.5 7.5a6.4 6.4 0 000 9M16.5 7.5a6.4 6.4 0 010 9M4.6 4.6a10.5 10.5 0 000 14.8M19.4 4.6a10.5 10.5 0 010 14.8"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"'),
  measure: s('<path d="M8 3v18M16 3v18M8 12h8"/><path d="M10.5 9.5L8 12l2.5 2.5M13.5 9.5L16 12l-2.5 2.5"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"'),
  settings: s('<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"'),
  box: s('<rect x="4" y="4" width="16" height="16" rx="1" stroke-dasharray="3 3"/>'),
  pan: s('<path d="M12 3v18M3 12h18M8 7l4-4 4 4M8 17l4 4 4-4"/>'),
  cursor: s('<path d="M8 3v18M16 3v18"/>'),
  clear: s('<path d="M8 3v8M16 3v8M8 21v-2M16 21v-2"/><path d="M9 12l6 6M15 12l-6 6"/>'),
  back: s('<path d="M9 14L4 9l5-5M4 9h11a5 5 0 010 10h-3"/>'),
  fit: s('<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>'),
  check: s('<path d="M5 12.5l4.5 4.5L19 7.5"/>', 'fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"'),
  lock: s('<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 018 0v3"/>'),
  vchart: s('<path d="M4 4v16h16"/><path d="M7 15l3.5-5 3 3 4.5-6"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  vtable: s('<rect x="3.5" y="4.5" width="17" height="15" rx="1.5"/><path d="M3.5 9.5h17M3.5 14.5h17M9.5 4.5v15"/>'),
  vboth: s('<path d="M4 3.5v8h16"/><path d="M7 9l3-3 3 2 4-4"/><rect x="3.5" y="14" width="17" height="6.5" rx="1"/><path d="M3.5 17.25h17"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  vvalues: s('<rect x="3" y="5" width="8" height="6" rx="1.2"/><rect x="13" y="5" width="8" height="6" rx="1.2"/><rect x="3" y="13" width="8" height="6" rx="1.2"/><rect x="13" y="13" width="8" height="6" rx="1.2"/>'),
  more: s('<circle cx="5" cy="12" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="19" cy="12" r="1.7"/>', 'fill="currentColor"'),
  /** the top bar's ⋯ */
  moreBar: s('<circle cx="5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="19" cy="12" r="1.6"/>', 'fill="currentColor"'),
  file: s('<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>'),
  /** a tab's menu */
  chevron: s('<path d="M6 9l6 6 6-6"/>', 'fill="none" stroke="currentColor" stroke-width="2.4"'),
  /** everywhere else: folds (turned -90° when shut), the view button, Add pane's kinds, the folder button */
  chev: s('<path d="M6 9l6 6 6-6"/>', 'fill="none" stroke="currentColor" stroke-width="2.5"'),
  play: s('<path d="M8 5v14l11-7z"/>', 'fill="currentColor"'),
  pause: s('<path d="M7 5h4v14H7zM13 5h4v14h-4z"/>', 'fill="currentColor"'),
  close: s('<path d="M6 6l12 12M18 6L6 18"/>', 'fill="none" stroke="currentColor" stroke-width="2.4"'),
  folder: s('<path d="M3 7h6l2 2h10v10H3z"/>'),
  expand: s('<path d="M7 15l5 5 5-5M7 9l5-5 5 5"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  collapse: s('<path d="M7 20l5-5 5 5M7 4l5 5 5-5"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  /** move a sidebar panel to the other side */
  swap: s('<path d="M4 8h14l-3-3M20 16H6l3 3"/>'),
  layout: s('<rect x="3.5" y="4.5" width="17" height="15" rx="1.5"/><path d="M3.5 11h17M12 11v8.5"/>', 'fill="none" stroke="currentColor" stroke-width="2"'),
  plus: s('<path d="M12 5v14M5 12h14"/>', 'fill="none" stroke="currentColor" stroke-width="2.2"'),
  /** Explorer's add-the-shown-channels */
  add: s('<path d="M12 5v14M5 12h14"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"'),
  /** Explorer's categories on/off */
  cats: s('<path d="M4 5h6M4 12h0M8 12h12M8 19h12M7 5v14"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  /** the top bar's Remove every channel and Remove every pane */
  clearChannels: s('<path d="M3 17l5-6 4 4 3-4"/><path d="M15 15l6 6M21 15l-6 6"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  removePanes: s('<rect x="3" y="4" width="18" height="6" rx="1"/><path d="M3 15h7M3 19h7"/><path d="M15 14l6 6M21 14l-6 6"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  /** a folder's session on the start page */
  session: s('<path d="M3.5 12a8.5 8.5 0 102.5-6"/><path d="M3 3.5V8h4.5"/><path d="M12 7.5V12l3 2"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'),
  trash: s('<path d="M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"'),
  open: s('<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4M12 11v6M9 14l3 3 3-3"/>', 'fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"'),
};
