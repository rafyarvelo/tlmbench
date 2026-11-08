<!-- Settings in the left sidebar: the everyday ones (Basic) or all of them, a search over every setting by name,
     explanation and choices, and each explanation behind an ⓘ. Changes apply and save at once. -->
<script lang="ts">
  import { TIME_FORMATS } from '../axes';
  import { resetTips } from '../tips';
  import { movePanel, resetSettings, resetUiLayout, type Settings, setPanelOpen, setSetting, settings, sideOf } from '../settings.svelte';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';

  interface Row {
    key: keyof Settings;
    label: string;
    hint?: string;
    section: string;
    basic?: boolean;
    /** A select's choices; none: a switch. */
    options?: [string, string][];
  }
  const STARTERS: [string, string][] = [['starter', 'Starter charts'], ['starterboth', 'Starter charts + tables'], ['table', 'One table, every channel'], ['values', 'One values pane'], ['none', 'Nothing plotted']];
  const ROWS: Row[] = [
    { key: 'mode', section: 'Mouse', basic: true, label: 'Mouse mode', hint: 'What dragging across a chart does. Shift-drag or middle-drag always pans.', options: [['box', 'Box zoom'], ['pan', 'Pan'], ['cursor', 'Cursors']] },
    { key: 'zoomSpeed', section: 'Mouse', label: 'Zoom speed', hint: 'How far one wheel notch or trackpad swipe zooms.', options: [['slow', 'Slow'], ['normal', 'Normal'], ['fast', 'Fast']] },
    { key: 'locked', section: 'Mouse', label: 'Lock the time view', hint: "Zoom, pan and fit gestures can't change the view. Also on the top bar (L)." },
    { key: 'lockLayout', section: 'Mouse', label: 'Also lock the layout', hint: 'A locked view also refuses closing, moving and adding panes.' },
    { key: 'wheelZoom', section: 'Mouse', label: 'Scroll wheel zooms the charts', hint: 'Off: the wheel pans, and Ctrl + wheel zooms.' },
    { key: 'lockCursors', section: 'Mouse', label: 'Cursors still work while locked' },
    { key: 'dblFit', section: 'Mouse', label: 'Double-click fits the whole recording' },
    { key: 'theme', section: 'Display', basic: true, label: 'Theme', hint: 'Match system follows your light or dark setting.', options: [['system', 'Match system'], ['light', 'Light'], ['dark', 'Dark']] },
    { key: 'exCategories', section: 'Display', basic: true, label: 'Group channels into categories', hint: "Explorer groups a recording's channels by the start of their names (before the first _ or .)." },
    { key: 'tooltips', section: 'Display', basic: true, label: 'Tooltips', hint: 'Short shows the first sentence and the shortcut, and each tip retires after three showings. Full always shows everything.', options: [['short', 'Short, then learned'], ['full', 'Full'], ['off', 'Off']] },
    { key: 'fileOpen', section: 'Files', basic: true, label: 'When a file opens', hint: 'Remember: the panes you had for that file last time (a file opened for the first time uses the choice below). The others always start from that choice.', options: [['remember', 'Remember its last layout'], ...STARTERS] },
    { key: 'fileOpenNew', section: 'Files', basic: true, label: 'Files opened for the first time', hint: 'Starter charts: one chart per group of channels, up to four. Starter charts + tables: the same, each with its table below. One table: every channel in a single table. One values pane: the value of every channel at the pointer.', options: STARTERS },
    { key: 'clickMode', section: 'Files', basic: true, label: 'Clicking a row', hint: 'Select first: one click selects a file or channel, double-click (or Enter) opens a file, the checkbox plots a channel. Act at once: one click opens or plots.', options: [['select', 'Select first'], ['act', 'Act at once']] },
    { key: 'dropNewView', section: 'Files', label: 'New pane from a dragged channel', hint: "Dropping a channel where a new pane goes makes a chart. Same view keeps the view of the pane it came from, so a channel dragged out of a table makes another table.", options: [['chart', 'Chart'], ['same', 'Same view as where it came from']] },
    { key: 'leftPanel', section: 'Display', basic: true, label: 'Left sidebar', hint: 'What it shows, picked from the rail. Ctrl+B.', options: [['explorer', 'Explorer'], ['measure', 'Measure'], ['settings', 'Settings'], ['', 'Hidden']] },
    { key: 'measureOpen', section: 'Display', basic: true, label: 'Right sidebar', hint: 'Cursor values and stats, unless you moved other panels there. Ctrl+Alt+B. With Measure on it, it also opens by itself when both cursors are placed (see below).' },
    { key: 'showHints', section: 'Display', label: 'Show tips and extra controls', hint: 'How-to notes in the panels.' },
    { key: 'quietUI', section: 'Display', basic: true, label: 'Quiet interface', hint: 'Hides badges and counts until they matter: the panes a channel is in, file sizes and category counts.' },
    { key: 'showStart', section: 'Display', label: 'Show the start page at launch', hint: 'Recent recordings and ways to open things, over your tabs when the app starts.' },
    { key: 'timeFormat', section: 'Display', basic: true, label: 'Time format', hint: 'Used for axes, readouts, tables and cursors. A file with no time reads in sample numbers. Click the time range in the top bar to switch quickly.', options: TIME_FORMATS.map(([k, n, ex]) => [k, k === 'wall' ? 'Wall clock or dates (files with absolute time)' : `${n} · ${ex}`] as [string, string]) },
    { key: 'timeDecimals', section: 'Display', label: 'Time decimals', options: [['auto', 'Automatic'], ...[0, 1, 2, 3, 4, 5, 6].map((n) => [String(n), String(n)] as [string, string])] },
    { key: 'timeAxis', section: 'Display', label: 'Time labels', hint: 'Which charts show the time axis labels. Charts share one time axis.', options: [['auto', 'Automatic (every chart on a narrow window)'], ['bottom', 'Bottom chart only'], ['every', 'Every chart']] },
    { key: 'drawRaw', section: 'Display', label: 'Draw every sample', hint: 'Automatic draws each sample only when there are at most 4 per pixel column, and reduces the rest to min/max per column (M4), which looks the same. Raise it to see every point drawn; slower with millions.', options: [['0', 'Automatic'], ['100000', 'Up to 100k per channel'], ['1000000', 'Up to 1M per channel'], ['10000000', 'Always (slow)']] },
    { key: 'overviewTop', section: 'Display', label: 'Overview above the panes' },
    { key: 'measureAuto', section: 'Display', label: 'Open Measure when both cursors are placed', hint: 'It closes again when you clear the cursors, unless you opened it yourself.' },
    { key: 'showPerf', section: 'Display', label: 'Show performance details', hint: 'A status bar with the last view: points drawn, how they were reduced, and the round trip.' },
  ];

  let q = $state('');
  let open = $state<Record<string, boolean>>({});
  const text = (r: Row) => [r.label, r.hint ?? '', ...(r.options ?? []).map((o) => o[1])].join(' ').toLowerCase();
  const rows = $derived(q.trim() ? ROWS.filter((r) => text(r).includes(q.trim().toLowerCase())) : ROWS.filter((r) => settings.settingsAll || r.basic));
  const sections = $derived([...new Set(rows.map((r) => r.section))]);
  const closed = $derived(settings.panelsClosed.includes('settings'));
  const other = $derived(sideOf('settings') === 'left' ? 'right' : 'left');
</script>

<div class="setin" id="setPanel">
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, 'settings')} ondragend={endPanelDrag}>
    <button class="phtog" aria-expanded={!closed} data-tip="Collapse or expand. Drag the title to move this panel." onclick={() => setPanelOpen('settings', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Settings</button><span class="spacer"></span>
    <button class="ibtn pmove" data-pmove aria-label="Move Settings to the {other} sidebar" data-tip="Move to the {other} sidebar" onclick={() => movePanel('settings', other)}>{@html ICON.swap}</button>
  </div>
  {#if !closed}
  <div class="setview" role="group" aria-label="Which settings">
    <button type="button" data-sv="basic" aria-pressed={!settings.settingsAll} onclick={() => setSetting('settingsAll', false)}>Basic</button>
    <button type="button" data-sv="all" aria-pressed={settings.settingsAll} onclick={() => setSetting('settingsAll', true)}>All settings</button>
  </div>
  <label class="setsearch"><input type="search" placeholder="Search settings" aria-label="Search settings" bind:value={q} /></label>
  {#each sections as s (s)}
    <section>
      <h3>{s}</h3>
      {#each rows.filter((r) => r.section === s) as r (r.key)}
        {#if r.options}
          <div class="row">
            <span>{r.label}{#if r.hint}<button class="info" aria-label="About {r.label}" aria-expanded={!!open[r.key]} onclick={() => (open[r.key] = !open[r.key])}>ⓘ</button>{/if}
              {#if r.hint && open[r.key]}<span class="hint">{r.hint}</span>{/if}</span>
            <select class="sel" data-set={r.key} aria-label={r.label} value={settings[r.key]} onchange={(e) => setSetting(r.key, e.currentTarget.value as never)}>
              {#each r.options as [v, l] (v)}<option value={v}>{l}</option>{/each}
            </select>
          </div>
        {:else}
          <label class="switch">
            <input type="checkbox" role="switch" data-set={r.key} checked={!!settings[r.key]} onchange={(e) => setSetting(r.key, e.currentTarget.checked as never)} />
            <span>{r.label}{#if r.hint}<button class="info" aria-label="About {r.label}" aria-expanded={!!open[r.key]} onclick={(e) => { e.preventDefault(); open[r.key] = !open[r.key]; }}>ⓘ</button>{/if}
              {#if r.hint && open[r.key]}<span class="hint">{r.hint}</span>{/if}</span>
          </label>
        {/if}
      {/each}
    </section>
  {:else}
    <p class="setnone">No setting matches “{q.trim()}”.</p>
  {/each}
  <section><h3>Layout</h3><div class="urow"><span>Sidebar sizes, sides and panels</span><button class="tbtn" id="setResetUi" onclick={resetUiLayout}>Reset UI layout</button></div><div class="urow"><span id="sessTxt">{settings.sessions[settings.folder]?.files.length ? 'Session saved · only you (app cache)' : 'Session saves automatically'}</span><button class="tbtn" id="setResetSession" data-tip="Close this folder's tabs and forget its saved session" onclick={actions.resetSession}>Reset session</button></div><div class="urow"><span>Panes remembered for each file</span><button class="tbtn" id="setForget" onclick={() => setSetting('layouts', {})}>Forget layouts</button></div><div class="urow"><span>Tips you have learned</span><button class="tbtn" id="setResetTips" onclick={resetTips}>Reset tooltips</button></div></section>
  <footer><span>Saved as you change them</span><button class="tbtn" id="setReset" onclick={resetSettings}>Reset to defaults</button></footer>
  {/if}
</div>

<style>
  .setin {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 12px 16px;
    font-size: 13px;
  }
  .setview {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
  }
  .setview button {
    min-height: 26px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .setview button[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--fg);
  }
  .setsearch input {
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 0 8px;
    border-bottom: 1px solid var(--grid);
  }
  h3 {
    margin: 2px 0;
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .row,
  .urow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 30px;
  }
  .row > span:first-child {
    flex: 1 1 40%;
    min-width: 0;
  }
  .row > select {
    max-width: 58%;
    min-height: 30px;
  }
  .switch {
    display: flex;
    flex-direction: row-reverse;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 3px 0;
  }
  .switch input {
    margin-top: 2px;
    accent-color: var(--accent);
  }
  .info {
    margin-left: 4px;
    padding: 0 2px;
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    font-size: 11px;
  }
  .info:hover,
  .info[aria-expanded='true'] {
    color: var(--accent);
  }
  .hint {
    display: block;
    margin-top: 2px;
    color: var(--fg-muted);
    font-size: 11.5px;
    line-height: 1.35;
  }
  .setnone {
    margin: 8px 0;
    color: var(--fg-muted);
    font-size: 12px;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
</style>
