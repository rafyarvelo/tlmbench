<!-- The top bar: tabs and + (open a saved layout) · time range · mouse mode, Clear, Back, Fit · ⋯ with the rest. A
     tab's ⌄ menu saves its layout (Save as… turns a file tab into a workspace tab), reverts or deletes it, and closes
     tabs. -->
<script lang="ts">
  import { keyName } from '../commands';
  import { ICON, userIcon } from '../icons';
  import { type Mode, PANELS, type Pin, setSetting, settings, toggleAllPanels } from '../settings.svelte';
  import { play, playActions } from '../play.svelte';
  import { moveTab, tabs } from '../tabs.svelte';
  import { TIME_FORMATS } from '../axes';
  import { addPaneBelow, allMode, clearChannels, clearPanes, setAllModes, stack } from '../stack.svelte';
  import { actions, isLocked, setLocked, ui, viewLocked } from '../ui.svelte';
  import { cfg } from '../share.svelte';
  import CheckBadge from './CheckBadge.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import { slotsUsed, type Workspace } from '../workspaces';
  /** A pin's name as people read it: a shared layout by its layout's name. */
  const pinName = (pn: Pin) => (pn.kind === 'shared' ? (ui.shared.find((v) => v.file === pn.name)?.ws?.name ?? pn.name.replace(/\.tlmview\.json$/, '')) : pn.name);
  let pinMenu = $state<null | { at: { x: number; y: number }; pin: Pin }>(null);
  /** The tab strip's arrows glide, or jump under reduced motion (the CSS rule doesn't reach a script's smooth scroll). */
  const glide = (): ScrollBehavior => (matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth');
  const pinItems = ({ kind, name }: Pin): MenuItem[] => [
    { label: kind === 'recipe' ? 'Add to this file' : 'Open', data: { pm: 'run' }, run: () => actions.runPin(kind, name) },
    { label: 'Unpin from the top bar', data: { pm: 'unpin' }, run: () => actions.togglePin(kind, name) },
  ];

  /** Files a layout names that the tab in front doesn't have yet (its `accel_x·2` needs file 2). */
  const have = $derived(Math.max(1, ui.slots.length));
  const needs = (w: Workspace) => Math.max(0, slotsUsed(w) - have);

  const ALL: ['chart' | 'table' | 'both', string, string, string][] = [
    ['chart', 'Charts', ICON.vchart, 'Show every pane as a chart'],
    ['table', 'Tables', ICON.vtable, 'Show every pane as a table of the samples in view'],
    ['both', 'Both', ICON.vboth, 'Show every pane as a chart with its table below'],
  ];
  const MODES: [Mode, string, string][] = [
    ['box', 'Box zoom', 'Drag across a plot to zoom into that range (B)'],
    ['pan', 'Pan', 'Drag to pan (P). In other modes, Shift-drag or middle-drag pans.'],
    ['cursor', 'Cursors', 'Click to place cursors A and B (C)'],
  ];
  let more = $state(false);
  /** The tab strip scrolls when its tabs don't fit: ‹ › appear beside it. */
  let tabsEl = $state<HTMLElement>();
  let tabsOver = $state(false);
  let tabsAt = $state(0);
  let tabsMax = $state(0);
  function measureTabs() {
    if (!tabsEl) return;
    tabsOver = tabsEl.scrollWidth > tabsEl.clientWidth + 1;
    tabsAt = tabsEl.scrollLeft;
    tabsMax = tabsEl.scrollWidth - tabsEl.clientWidth;
  }
  $effect(() => {
    if (!tabsEl) return;
    void [tabs.list.length, tabs.active, barW];
    const ro = new ResizeObserver(measureTabs);
    ro.observe(tabsEl);
    measureTabs();
    // the tab in front stays in sight
    // a tab wider than the strip shows its start (the name is cut at the end, never at the left)
    const front = tabsEl.querySelector<HTMLElement>('[aria-selected="true"]');
    if (front && front.offsetWidth > tabsEl.clientWidth) tabsEl.scrollLeft = front.offsetLeft - tabsEl.offsetLeft;
    else front?.scrollIntoView({ inline: 'nearest', block: 'nearest' });
    return () => ro.disconnect();
  });
  /** The bar's width: under 900 px Clear, Back, Fit, Play and Lock move into ⋯. */
  let barW = $state(1200);
  const narrow = $derived(barW < 900);
  /** a wide bar shows Remove every channel and Remove every pane as icons (the ⋯ menu keeps them otherwise) */
  const wide = $derived(barW >= 1300);
  /** The menu on the time range: how times read. */
  let timeMenu = $state<HTMLElement | null>(null);
  /** The tab whose menu is open, and where; `naming` while Save as… asks for a name. */
  /** The top bar's Export menu (owner decision 6: one click away, not only under ⋯). */
  let exporting = $state(false);
  let exportBtn = $state<HTMLElement>();
  let tabMenu = $state<null | { key: number; anchor?: HTMLElement; at?: { x: number; y: number }; naming?: 'saveas' | 'share' | 'rename' }>(null);
  /** The + menu: your layouts, and a new empty one (`naming`). */
  let addMenu = $state<null | { anchor: HTMLElement; naming?: 'new' }>(null);
  let wsName = $state('');
  let wsErr = $state('');
  const menuTab = $derived(tabMenu ? tabs.list.find((t) => t.key === tabMenu!.key) : undefined);
  function nameSubmit(e: SubmitEvent, f: (name: string) => string | null) {
    e.preventDefault();
    const err = f(wsName);
    if (err) wsErr = err;
    else tabMenu = addMenu = null;
  }
  function startNaming(name: string) {
    wsName = name;
    wsErr = '';
  }
  let dragTab = $state<number | null>(null);
  /** Runs a tab menu item on the menu's tab, closing the menu first. */
  function tabAct(f: (key: number) => void) {
    const key = tabMenu?.key;
    tabMenu = null;
    if (key != null) f(key);
  }
  /** Opens a tab's menu under its ▾ (`anchor`) or tab, or at the pointer (`at`). */
  function openTabMenu(key: number, how: { anchor?: HTMLElement; at?: { x: number; y: number } }) {
    addMenu = null;
    tabMenu = { key, ...how };
  }
  let moreBtn: HTMLButtonElement | undefined = $state();
  /** The desktop app has a menu bar; a browser page and an export's viewer have not. */
  const desktop = !ui.browserOnly && !__VIEWER__;
  const lk = $derived(viewLocked());
  const LOCKED = 'Unlock to change panes';
  const provenance = () => (ui.viewerInfo ? [ui.viewerInfo.files.join(', '), ui.viewerInfo.created ? new Date(ui.viewerInfo.created).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }) : '', ui.viewerInfo.version ? `TelemetryBench ${ui.viewerInfo.version}` : ''].filter(Boolean).join(' · ') : '');
  const lockTip = () =>
    (isLocked() ? 'Locked. Click to unlock (L)' : `Lock the view so zoom, pan and fit can't change it${settings.lockLayout || ui.viewer ? ', and panes stay put' : ''} (L)`) +
    (ui.viewer && provenance() ? `. ${provenance()}` : '');
  const moreItems = (): MenuItem[] => [
    ...(ui.viewer ? [] : [{ label: 'Open recording…', kbd: '⌘O', run: actions.open }]),
    ...(narrow
      ? [
          { heading: 'Bar' },
          { label: 'Back', kbd: '⌥←', data: { more: 'back' }, disabled: !ui.canBack, run: actions.back },
          { label: 'Fit', kbd: 'F', data: { more: 'fit' }, disabled: !ui.file, run: actions.fit },
          { label: 'Play back', kbd: 'Space', checked: play.open, data: { more: 'play' }, disabled: !ui.file, run: () => (play.open ? playActions.close() : playActions.open()) },
        ]
      : []),
    { heading: 'View' },
    ...(narrow
      ? [
          ...(ui.browserOnly ? [] : [{ label: 'Event markers', kbd: 'E', checked: settings.eventsOn, run: () => setSetting('eventsOn', !settings.eventsOn) } as MenuItem]),
          { label: 'Lock the view', kbd: 'L', checked: isLocked(), run: () => setLocked(!isLocked()) },
          { label: 'Clear cursors', kbd: 'Esc', disabled: !ui.anyCursor, run: actions.clearCursors },
        ]
      : []),
    ...(ui.browserOnly ? [] : ([
      { label: 'Link time view with other windows', checked: settings.linkView, data: { more: 'linkview' }, tip: 'This window follows the other windows that link too, and they follow it', run: () => setSetting('linkView', !settings.linkView) },
      { label: 'Link cursors too', checked: settings.linkCursors, data: { more: 'linkcursors' }, disabled: !settings.linkView, run: () => setSetting('linkCursors', !settings.linkCursors) },
    ] as MenuItem[])),
    { label: 'Overview above the panes', checked: settings.overviewTop, run: () => setSetting('overviewTop', !settings.overviewTop) },
    { heading: 'All panes' },
    ...ALL.map(([m, label, , tip]): MenuItem => ({ label: `Every pane as ${m === 'both' ? 'chart and table' : label.toLowerCase().replace(/s$/, '')}`, checked: allMode() === m, radio: true, data: { allmode: m }, disabled: lk, tip: lk ? LOCKED : tip, run: () => setAllModes(m) })),
    // the desktop app's menu bar (Pane, File) and the top bar's Export say the rest; a page without them keeps these here
    ...(desktop
      ? []
      : ([
          { label: 'New pane', kbd: '⌘⇧N', data: { more: 'addpane' }, disabled: !ui.file || lk, tip: lk ? LOCKED : undefined, run: () => addPaneBelow(Math.max(0, stack.panes.length - 1)) },
          { label: 'Remove every channel', data: { more: 'nochan' }, disabled: lk || !stack.panes.some((p) => p.channels.length), tip: lk ? LOCKED : undefined, run: clearChannels },
          { label: 'Remove every pane', data: { more: 'nopanes' }, disabled: lk || !stack.panes.length, tip: lk ? LOCKED : undefined, run: clearPanes },
          ...(__VIEWER__ ? [{ label: 'Save the view as an image…', data: { more: 'exportimage' }, disabled: !ui.file || !stack.panes.length, run: actions.imageOpen }] : []),
        ] as MenuItem[])),
    { heading: 'Window' },
    { label: settings.panelsClosed.length >= PANELS.length ? 'Expand all sidebar panels' : 'Collapse all sidebar panels', data: { more: 'panels' }, run: toggleAllPanels },
    { label: 'Switch theme', kbd: 'T', data: { more: 'theme' }, run: actions.toggleTheme },
    { label: 'Focus mode', kbd: '⌘.', data: { more: 'focus' }, run: () => actions.toggleFocus(true) },
    { label: 'Show performance details', checked: settings.showPerf, run: () => setSetting('showPerf', !settings.showPerf) },
    { label: 'Settings…', run: () => setSetting('leftPanel', 'settings') },
  ];
  /** ⌘S (the command table's saveLayout) saves the layout in front; a file tab asks for a name. */
  function saveFront() {
    const t = tabs.list.find((t) => t.key === tabs.active);
    if (t?.ws) actions.saveWs(t.key);
    else if (t && !t.live) {
      const el = document.querySelector<HTMLElement>(`[data-tab="${t.key}"]`);
      openTabMenu(t.key, el ? { anchor: el } : { at: { x: 8, y: 44 } });
      tabMenu!.naming = 'saveas';
      startNaming('');
    }
  }
  actions.saveFront = saveFront;
  function key(e: KeyboardEvent) {
    // Ctrl+PageUp / PageDown switch tabs; with Shift they move the tab
    if (e.ctrlKey && (e.key === 'PageUp' || e.key === 'PageDown') && tabs.list.length > 1) {
      e.preventDefault();
      const i = tabs.list.findIndex((t) => t.key === tabs.active);
      const d = e.key === 'PageUp' ? -1 : 1;
      const j = (i + d + tabs.list.length) % tabs.list.length;
      if (e.shiftKey) {
        if (j !== i + d) return;
        moveTab(i, d > 0 ? j + 1 : j);
        actions.saveSession();
      } else actions.showTab(tabs.list[j].key);
      return;
    }
  }
</script>

<svelte:window onkeydowncapture={key} />

<header class="toolbar" bind:clientWidth={barW}>
  {#if tabsOver}<button class="tb tscroll" id="tabsLeft" aria-label="Scroll tabs left" disabled={tabsAt <= 0} onclick={() => tabsEl?.scrollBy({ left: -200, behavior: glide() })}>‹</button>{/if}
  <div class="wstabs" role="tablist" aria-label="Open tabs" id="wsTabs" bind:this={tabsEl} onscroll={measureTabs}>
    {#each tabs.list as t, i (t.key)}
      <div
        class="wstab"
        role="tab"
        tabindex="0"
        aria-selected={t.key === tabs.active}
        data-tab={t.key}
        data-tip={t.tip ?? t.path}
        data-path={t.path}
        draggable="true"
        onclick={() => actions.showTab(t.key)}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') (e.preventDefault(), actions.showTab(t.key));
          else if (e.key === 'Delete') (e.preventDefault(), actions.closeTab(t.key));
        }}
        onauxclick={(e) => e.button === 1 && actions.closeTab(t.key)}
        oncontextmenu={(e) => (e.preventDefault(), openTabMenu(t.key, { at: { x: e.clientX, y: e.clientY } }))}
        ondragstart={() => (dragTab = i)}
        ondragover={(e) => dragTab != null && e.preventDefault()}
        ondrop={(e) => {
          e.preventDefault();
          // the half of the tab under the pointer says which side it lands on (a drag onto the right-hand neighbour moves it)
          const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
          if (dragTab != null) moveTab(dragTab, e.clientX > r.left + r.width / 2 ? i + 1 : i);
          dragTab = null;
          actions.saveSession();
        }}
        ondragend={(e) => {
          // let go away from the strip (outside the window, or below the bar): the tab leaves for another window
          const away = e.clientY < 0 || e.clientY > 56 || e.clientX < 0 || e.clientX > innerWidth;
          const cancelled = e.screenX === 0 && e.screenY === 0;
          if (dragTab != null && e.dataTransfer?.dropEffect === 'none' && away && !cancelled) actions.dropTab(t.key, e.screenX, e.screenY);
          dragTab = null;
        }}
      >
        {@html t.report ? ICON.checks : t.ws ? ICON.layout : ICON.file}<span class="nm">{t.name}</span><CheckBadge path={t.path} />{#if t.live}<span class="livedot" data-live-tab aria-label="live"></span>{/if}{#if t.dirty}<span class="dirty" data-dirty aria-label="unsaved changes" data-tip="Unsaved changes: Save layout (⌘S) or Revert changes in the tab menu"></span>{/if}
        <span
          class="tm"
          role="button"
          tabindex="-1"
          aria-label="Menu for {t.name}"
          aria-haspopup="menu"
          onclick={(e) => {
            e.stopPropagation();
            if (tabMenu?.key === t.key) tabMenu = null;
            else openTabMenu(t.key, { anchor: e.currentTarget as HTMLElement });
          }}
          onkeydown={() => {}}>{@html ICON.chevron}</span
        >
        <span
          class="tx"
          role="button"
          tabindex="-1"
          aria-label="Close {t.name}"
          onclick={(e) => (e.stopPropagation(), actions.closeTab(t.key))}
          onkeydown={() => {}}>{@html ICON.close}</span
        >
      </div>
    {/each}
    {#if !__VIEWER__ && !ui.browserOnly}<button
      class="tnew"
      id="wsTabNew"
      aria-label="New tab"
      aria-haspopup="menu"
      aria-expanded={!!addMenu}
      data-tip="Open one of your layouts in a new tab, or start an empty one"
      onclick={(e) => {
        tabMenu = null;
        addMenu = addMenu ? null : { anchor: e.currentTarget as HTMLElement };
        if (addMenu) actions.loadShared();
      }}>+</button
    >{/if}
  </div>
  {#if tabsOver}<button class="tb tscroll" id="tabsRight" aria-label="Scroll tabs right" disabled={tabsAt >= tabsMax - 1} onclick={() => tabsEl?.scrollBy({ left: 200, behavior: glide() })}>›</button>{/if}
  {#if settings.pins.length && !ui.viewer}
    <div class="pins" id="pinStrip" role="group" aria-label="Pinned">
      {#each settings.pins as pn (pn.kind + pn.name)}
        <button class="tb" data-pin={pn.name} aria-label={pinName(pn)} data-tip="{pn.kind === 'recipe' ? 'Add the recipe to this file' : pn.kind === 'shared' ? 'Open the shared layout' : 'Open the layout'} {pinName(pn)}. Right-click to unpin." onclick={() => actions.runPin(pn.kind, pn.name)} oncontextmenu={(e) => (e.preventDefault(), (pinMenu = { at: { x: e.clientX, y: e.clientY }, pin: pn }))}>{#if pn.icon && userIcon(pn.icon)}{@html userIcon(pn.icon, 16)}{:else}{@html pn.kind === 'recipe' ? ICON.file : ICON.layout}{/if}</button>
      {/each}
    </div>
    {#if pinMenu}<Menu class="pinmenu" label="Pinned {pinName(pinMenu.pin)}" at={pinMenu.at} items={pinItems(pinMenu.pin)} onclose={() => (pinMenu = null)} />{/if}
  {/if}
  <div class="rangebar">
    <button class="num rangebtn" id="rangeTxt" aria-haspopup="menu" aria-expanded={!!timeMenu} data-tip="Time format: click to change how times read" onclick={(e) => (timeMenu = timeMenu ? null : (e.currentTarget as HTMLElement))}>{ui.range}</button>
    <span class="muted" id="spanTxt">{ui.span}</span>
    <span class="num muted" id="hoverTxt">{ui.pointer}</span>
  </div>
  <div class="seg" role="group" aria-label="Mouse mode">
    {#each MODES as [m, label, tip] (m)}
      <button class="tb" id="m-{m}" disabled={!ui.file} aria-pressed={settings.mode === m} aria-label={label} data-tip={tip} onclick={() => setSetting('mode', m)}>{@html ICON[m]}</button>
    {/each}
  </div>
  {#if !narrow}
    <button class="tb" id="b-clear" aria-label="Clear cursors" disabled={!ui.anyCursor} data-tip="Remove cursors A and B (Esc)" onclick={actions.clearCursors}>{@html ICON.clear}</button>
  {/if}
  {#if !narrow}
    <button class="tb" id="b-back" aria-label="Back" disabled={!ui.canBack} data-tip="Previous view (⌥←)" onclick={actions.back}>{@html ICON.back}</button>
  {/if}
  {#if !narrow}
    <button class="tb" id="b-fit" aria-label="Fit" disabled={!ui.file} data-tip="Show the whole recording (F)" onclick={actions.fit}>{@html ICON.fit}</button>
  {/if}
  {#if !narrow}
    <button class="tb" id="b-play" aria-label="Play back" aria-pressed={play.open} disabled={!ui.file} data-tip="Play back: a playhead moves through the file and the view follows it (Space plays and pauses)" onclick={() => (play.open ? playActions.close() : playActions.open())}>{@html ICON.play}</button>
  {/if}
  {#if !narrow}<span class="vsep" aria-hidden="true"></span>{/if}
  {#if !narrow}
    {#if !ui.browserOnly}<button class="tb" id="b-events" aria-pressed={settings.eventsOn} aria-label="Event markers" data-tip="Show or hide event markers on the charts and the overview (E)" onclick={() => setSetting('eventsOn', !settings.eventsOn)}>{@html ICON.events}</button>{/if}
  {/if}
  {#if !narrow || isLocked()}
    <!-- one lock, in the app and in exports (owner, Nov 16, D-ui-62); a narrow bar keeps it while it's on, so the way out shows -->
    <button class="tb" id="b-lock" aria-pressed={isLocked()} aria-label={isLocked() ? 'Unlock' : 'Lock'} data-tip={lockTip()} onclick={() => setLocked(!isLocked())}>{@html isLocked() ? ICON.lock : ICON.unlock}</button>
  {/if}
  {#if barW >= 1200}
    <div class="seg allmodes" role="group" aria-label="All panes view">
      {#each ALL as [m, label, icon, tip] (m)}
        <button class="tb" data-allmode={m} aria-pressed={allMode() === m} disabled={lk || !stack.panes.length} aria-label={label} data-tip={lk ? LOCKED : tip} onclick={() => setAllModes(m)}>{@html icon}</button>
      {/each}
    </div>
  {/if}
  {#if wide}
    <div class="seg pact" role="group" aria-label="Pane actions">
      <button class="tb" id="tb-clearch" aria-label="Remove every channel" disabled={lk || !stack.panes.some((p) => p.channels.length)} data-tip={lk ? LOCKED : "Remove every channel from every pane (panes stay)"} onclick={clearChannels}>{@html ICON.clearChannels}</button>
      <button class="tb" id="tb-rmpanes" aria-label="Remove every pane" disabled={lk || !stack.panes.length} data-tip={lk ? LOCKED : "Remove every pane"} onclick={clearPanes}>{@html ICON.removePanes}</button>
    </div>
  {/if}
  {#if ui.viewer}
    <!-- where an export came from (owner decision 6, R12) -->
    <span class="viewinfo" id="viewerInfo">
      {#if ui.viewerInfo}<span class="vprov">{provenance()}</span>{/if}
    </span>
  {/if}
  {#if !__VIEWER__}
    <button class="tb" id="b-export" aria-label="Export" aria-haspopup="menu" aria-expanded={exporting} disabled={!ui.file} data-tip="Export: to HTML, data files or an image" bind:this={exportBtn} onclick={() => (exporting = !exporting)}>{@html ICON.export}</button>
  {/if}
  <button class="tb" id="b-more" aria-label="More" aria-haspopup="menu" aria-expanded={more} bind:this={moreBtn} onclick={() => (more = !more)}>{@html ICON.moreBar}</button>
  {#if tabMenu}
    <Menu class="tabmenu" label="Tab" anchor={tabMenu?.anchor} at={tabMenu?.at} onclose={() => (tabMenu = null)}>
      {#if tabMenu.naming}
        <form class="wsname" onsubmit={(e) => nameSubmit(e, (n) => tabMenu!.naming === 'rename' ? actions.renameWs(menuTab!.ws!, n) : (tabMenu!.naming === 'share' ? actions.share : actions.saveAs)(tabMenu!.key, n))}>
          <label for="wsName">{tabMenu.naming === 'share' ? 'Share with this folder as' : tabMenu.naming === 'rename' ? 'Rename the layout to' : 'Save as a layout named'}</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="wsName" class="sel" maxlength="60" autocomplete="off" spellcheck="false" autofocus bind:value={wsName} oninput={() => (wsErr = '')} />
          {#if wsErr}<span class="wserr" role="alert">{wsErr}</span>{/if}
          {#if tabMenu.naming === 'share'}<span class="wshint">Writes .tlmbench/views/ in {settings.folder || 'the folder'}: the layout, the file and the time range in view.</span>{/if}
          <div class="wsbtns">
            <button class="btn" type="button" onclick={() => (tabMenu = null)}>Cancel</button><button class="btn primary" type="submit"
              >{tabMenu.naming === 'share' ? 'Share' : tabMenu.naming === 'rename' ? 'Rename' : 'Save'}</button
            >
          </div>
        </form>
      {:else}
        {#if menuTab}
          {@const sharedBy = menuTab.shared ? ui.shared.find((v) => v.file === menuTab.shared)?.ws?.by : undefined}
          <div class="tmhead"><b class="tmname" id="tmName">{menuTab.name}</b><button class="tmx" id="tmClose" aria-label="Close tab" data-tip="Close this tab" onclick={() => tabAct(actions.closeTab)}>{@html ICON.close}</button></div>
          <div class="tmstat" id="wsStatus">
            {#if menuTab.report}{@html ICON.checks}<span>The folder report of {menuTab.report}, kept with the set. Run again from the tab.</span>
            {:else if menuTab.live}<span class="dot live"></span><span>Live session. Its panes live with the tab.</span>
            {:else if menuTab.shared}
              {#if menuTab.dirty}<span class="dot warn"></span><span>Unsaved changes. Shared views are read-only here: Save as makes your own copy.</span>
              {:else}{@html ICON.folder}<span>Shared in this folder{sharedBy ? ` by ${sharedBy}` : ''} · .tlmbench/views/{menuTab.shared}</span>{/if}
            {:else if menuTab.ws}
              {#if menuTab.dirty}<span class="dot warn"></span><span>Unsaved changes</span>{:else}<span class="dot good"></span><span>Saved</span>{/if}
            {:else if ui.viewer}{@html ICON.file}<span>The view this export was made with.</span>
            {:else if ui.browserOnly}{@html ICON.file}<span>The panes this file opened with.</span>
            {:else}{@html ICON.file}<span>The panes this file opened with. Save as keeps them as a layout.</span>{/if}
          </div>
          {#if !menuTab.live && !menuTab.report}<div class="tmfiles" id="tabFiles"><span>This tab shows</span>{#if menuTab.key === tabs.active && ui.slots.length > 1}{#each ui.slots as r (r.slot)}<span class="fchip" data-tip={r.path}>{r.slot} · {r.name}</span>{/each}{:else}<span class="fchip" data-tip={menuTab.path}>1 · {(menuTab.path.split(/[\\/]/).pop() ?? menuTab.path) + (menuTab.part ? ` · ${menuTab.part}` : '')}</span>{/if}</div>{/if}
          <div class="msep"></div>
        {/if}
        <!-- a viewer saves nothing, shares nothing and has one window: those items are left out, not disabled; a browser tab keeps no layouts and has one window either -->
        {#if !ui.viewer && !ui.browserOnly}
        {#if !(menuTab?.live || menuTab?.report) && !menuTab?.shared}
          <button class="mi" role="menuitem" data-ws="save" disabled={!menuTab?.ws || !menuTab.dirty} onclick={() => tabAct(actions.saveWs)}><span class="nm">Save layout</span><kbd>{keyName('⌘S')}</kbd></button>
        {/if}
        {#if !(menuTab?.live || menuTab?.report)}
          <button
            class="mi"
            role="menuitem"
            data-ws="saveas"
            onclick={() => (startNaming(menuTab?.ws ? (menuTab.shared ? menuTab.ws : `${menuTab.ws} copy`) : ''), (tabMenu!.naming = 'saveas'))}
            ><span class="nm">Save as…</span>{#if menuTab?.shared}<span class="hint">your copy</span>{/if}</button
          >
        {/if}
        {#if !(menuTab?.live || menuTab?.report)}
          <button class="mi" role="menuitem" data-ws="revert" disabled={!menuTab?.ws || !menuTab.dirty} onclick={() => tabAct(actions.revertWs)}><span class="nm">Revert changes</span></button>
        {/if}
        {#if !(menuTab?.live || menuTab?.report) && !cfg.writable}
          <button class="mi" role="menuitem" data-ws="share" data-tip="This folder is read-only, so the layout can't be shared into it. Export file… writes it where you choose." onclick={() => tabAct(actions.exportWs)}
            ><span class="nm">Export file…</span><span class="hint">Folder is read-only</span></button
          >
        {:else if !(menuTab?.live || menuTab?.report)}
          <button
            class="mi"
            role="menuitem"
            data-ws="share"
            data-tip="Writes a .tlmview.json into the folder's .tlmbench/views/ with the layout, the file (by its path in the folder) and the time range. Anyone who opens the folder sees it."
            onclick={() => (startNaming(menuTab?.ws ?? ''), (tabMenu!.naming = 'share'))}><span class="nm">{menuTab?.shared ? 'Update shared copy…' : 'Share with this folder…'}</span></button
          >
          <button class="mi" role="menuitem" data-ws="export" onclick={() => tabAct(actions.exportWs)}><span class="nm">Export file…</span></button>
        {/if}
        {#if menuTab?.ws && !menuTab.shared}
          <button class="mi" role="menuitem" data-ws="rename" onclick={() => (startNaming(menuTab!.ws!), (tabMenu!.naming = 'rename'))}><span class="nm">Rename…</span></button>
          <button class="mi" role="menuitem" data-ws="delete" onclick={() => tabAct(() => actions.deleteWs(menuTab!.ws!))}><span class="nm">Delete layout</span></button>
        {/if}
        {#if !(menuTab?.live || menuTab?.report)}<div class="msep"></div>{/if}
        <button class="mi" role="menuitem" data-ws="newwin" onclick={() => tabAct((k) => actions.moveToWindow(k))}><span class="nm">Move to new window</span></button>
        {/if}
        {#if menuTab?.live}<button class="mi" role="menuitem" data-ws="showwin" data-tip="The source keeps recording once and shows in both windows" onclick={() => tabAct((k) => actions.moveToWindow(k, true))}><span class="nm">Show in new window</span></button>{/if}
        <button class="mi" role="menuitem" onclick={() => tabAct(actions.closeTab)}><span class="nm">Close tab</span></button>
      <button class="mi" role="menuitem" disabled={tabs.list.length < 2} onclick={() => tabAct(actions.closeOtherTabs)}
        ><span class="nm">Close other tabs</span></button
      >
      <button class="mi" role="menuitem" onclick={() => tabAct(() => actions.closeFile())}><span class="nm">Close all tabs</span></button>
      {/if}
    </Menu>
  {/if}
  {#if addMenu}
    <Menu class="addmenu" label="New tab" anchor={addMenu?.anchor} onclose={() => (addMenu = null)}>
      {#if addMenu.naming}
        <form class="wsname" onsubmit={(e) => nameSubmit(e, actions.newWs)}>
          <label for="wsName">Name of the new layout</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="wsName" class="sel" maxlength="60" autocomplete="off" spellcheck="false" autofocus bind:value={wsName} oninput={() => (wsErr = '')} />
          {#if wsErr}<span class="wserr" role="alert">{wsErr}</span>{/if}
          <div class="wsbtns"><button class="btn" type="button" onclick={() => (addMenu = null)}>Cancel</button><button class="btn primary" type="submit">Create</button></div>
        </form>
      {:else}
        <button class="mi" role="menuitem" data-wsnew disabled={!ui.file} onclick={() => (startNaming(''), (addMenu!.naming = 'new'))}>{@html ICON.plus}<span class="nm">Empty layout…</span></button>
        <div class="mgrp">Your layouts · this computer</div>
        {#each settings.workspaces as w (w.name)}
          {@const open = tabs.list.some((t) => t.ws === w.name)}
          <div class="lyrow">
            <button
              class="mi"
              role="menuitem"
              data-wsopen={w.name}
              disabled={!ui.file && !open}
              onclick={() => {
                addMenu = null;
                actions.openWs(w.name);
              }}>{@html ICON.layout}<span class="nm">{w.name}</span><span class="hint" class:need={!open && needs(w) > 0} data-needs={!open && needs(w) > 0 ? needs(w) : undefined}>{open ? 'open in a tab' : needs(w) > 0 ? `needs ${needs(w) === 1 ? 'file ' + (have + 1) : needs(w) + ' files'}` : `${w.panes.length} ${w.panes.length === 1 ? 'pane' : 'panes'}`}</span></button
            >
            <button class="lydel" data-wsdel={w.name} aria-label="Delete layout {w.name}" data-tip="Delete this layout (you can undo)" onclick={() => actions.deleteWs(w.name)}
              >{@html ICON.trash}</button
            >
          </div>
        {:else}
          <p class="mnote">None yet. Use Save as… in a tab's menu.</p>
        {/each}
        <div class="mgrp">Shared in this folder · .tlmbench/</div>
        {#each ui.shared as v (v.file)}
          {@const open = tabs.list.some((t) => t.shared === v.file)}
          <button
            class="mi"
            role="menuitem"
            data-shopen={v.file}
            disabled={!v.ws}
            data-tip={v.error ? `${v.file} can't be read: ${v.error}` : `${v.file}: opens its file with the layout and time range it was shared with`}
            onclick={() => {
              addMenu = null;
              actions.openShared(v.file);
            }}
            >{@html ICON.folder}<span class="nm">{v.ws?.name ?? v.file}</span><span class="hint"
              >{v.error ? "can't be read" : open ? 'open in a tab' : v.ws?.by ? `by ${v.ws.by}` : ''}</span
            ></button
          >
        {:else}
          <p class="mnote">{settings.folder ? 'Nothing shared here yet.' : 'Open a folder to see the views shared in it.'}</p>
        {/each}
        <p class="mnote">A layout says which channels sit in which panes, by name, so it works on any recording that has them. It opens on the file in front.</p>
      {/if}
    </Menu>
  {/if}
  {#if timeMenu}
    <Menu
      class="timemenu"
      label="Time format"
      anchor={timeMenu}
      onclose={() => (timeMenu = null)}
      items={[
        { heading: ui.time === 'samples' ? 'This file has no time: sample numbers' : 'Time format' },
        ...TIME_FORMATS.map(([k, name, ex]) => {
          const noClock = k === 'wall' && ui.time !== 'absolute';
          return { label: noClock ? `${name} (needs a file with absolute time)` : name, meta: noClock ? undefined : ex, disabled: noClock, checked: settings.timeFormat === k, radio: true, data: { tf: k }, run: () => setSetting('timeFormat', k) };
        }),
        { sep: true },
        { label: 'More in Settings…', run: () => (setSetting('settingsAll', true), setSetting('leftPanel', 'settings')) },
      ]}
    />
  {/if}
  {#if more}
    <Menu class="moremenu" label="More" first anchor={moreBtn} onclose={() => (more = false)} items={moreItems()} />
  {/if}
  {#if exporting}
    <Menu
      class="exportmenu"
      label="Export"
      first
      anchor={exportBtn}
      onclose={() => (exporting = false)}
      items={[
        { label: 'Export to HTML…', tip: 'A page anyone can open, with nothing to install', data: { xm: 'html' }, run: actions.htmlOpen },
        { label: 'Export data…', tip: 'CSV, Parquet, HDF5, MAT or Arrow, for other tools', data: { xm: 'data' }, run: actions.exportOpen },
        { label: 'Save the view as an image…', tip: 'PNG or SVG of the panes', data: { xm: 'image' }, disabled: !stack.panes.length, run: actions.imageOpen },
      ]}
    />
  {/if}
</header>

<style>
  .pins {
    display: flex;
    flex: none;
    gap: 2px;
    margin-left: 4px;
  }
  .toolbar {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    padding: 0 10px 0 0;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  /* the prototype's .pact: a group after a rule, icons only */
  .pact,
  .allmodes {
    margin-left: 4px;
    padding-left: 6px;
    border-left: 1px solid var(--border);
    border-radius: 0;
  }
  .pact .tb,
  .allmodes .tb {
    padding: 0 8px;
    min-height: 30px;
    color: var(--fg-muted);
  }
  .allmodes .tb[aria-pressed='true'] {
    color: var(--fg);
  }
  .pact .tb:hover,
  .allmodes .tb:hover {
    color: var(--fg);
  }
  .pact .tb:disabled,
  .allmodes .tb:disabled {
    opacity: 0.4;
  }
  .wstabs {
    display: flex;
    flex: 0 1 auto;
    align-self: stretch;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .wstabs::-webkit-scrollbar {
    display: none;
  }
  .livedot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--danger);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .tscroll {
    min-width: 22px;
    padding: 0;
    font-size: 15px;
  }
  .wstab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    min-width: 96px;
    max-width: 220px;
    padding: 0 6px 0 14px;
    color: var(--fg-muted);
    font-size: 13px;
    cursor: pointer;
  }
  .wstab:hover {
    background: var(--raised);
  }
  .wstab[aria-selected='true'] {
    color: var(--fg);
    font-weight: 600;
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .tm,
  .tx {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: var(--fg-muted);
    visibility: hidden;
  }
  .tm :global(svg),
  .tx :global(svg) {
    width: 11px;
    height: 11px;
  }
  .wstab:hover .tm,
  .wstab:hover .tx,
  .wstab[aria-selected='true'] .tm,
  .wstab[aria-selected='true'] .tx {
    visibility: visible;
  }
  .tm:hover,
  .tx:hover {
    background: var(--border);
    color: var(--fg);
  }
  :global(.tabmenu),
  :global(.addmenu) {
    width: min(380px, calc(100vw - 32px));
    max-height: min(520px, calc(100vh - 32px));
  }
  :global(.addmenu) .mi :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .tnew {
    display: inline-grid;
    flex: none;
    place-items: center;
    align-self: center;
    width: 30px;
    height: 30px;
    margin: 0 4px 0 2px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 15px;
    line-height: 1;
  }
  .tnew:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .dirty {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .wsname {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .wsname .sel {
    color: var(--fg);
    font-size: 13px;
  }
  .wshint {
    font-size: 11.5px;
  }
  .wserr {
    color: var(--warn);
    font-size: 11.5px;
  }
  .wsbtns {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .tmhead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
  }
  .tmname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tmx {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
  }
  .tmx:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .tmx :global(svg) {
    width: 10px;
    height: 10px;
  }
  .tmstat {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 8px 6px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .tmstat :global(svg) {
    flex: none;
    width: 13px;
    height: 13px;
  }
  .tmstat .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .tmstat .dot.warn {
    background: var(--warn);
  }
  .tmstat .dot.good {
    background: var(--good);
  }
  .tmstat .dot.live {
    background: var(--danger);
  }
  .tmfiles {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    padding: 0 8px 6px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .fchip {
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    color: var(--fg);
    font-size: 11.5px;
  }
  .lyrow {
    display: flex;
    align-items: center;
  }
  .lyrow .mi {
    flex: 1;
    min-width: 0;
  }
  .lydel {
    display: inline-grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    visibility: hidden;
  }
  .lydel :global(svg) {
    width: 13px;
    height: 13px;
  }
  .lyrow:hover .lydel,
  .lydel:focus-visible {
    visibility: visible;
  }
  .lydel:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .mnote {
    margin: 4px 8px 6px;
    font-size: 11.5px;
    color: var(--fg-muted);
  }
  .wstab > :global(svg) {
    flex: none;
    width: 11px;
    height: 11px;
    opacity: 0.7;
  }
  .wstab .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rangebar {
    display: flex;
    flex: 1 0 auto;
    align-items: center;
    justify-content: safe center;
    gap: 10px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    font-size: 13px;
  }
  .rangebtn {
    padding: 1px 4px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-family: var(--font-data);
    font-variant-numeric: tabular-nums;
    /* keeps its width while the decimals follow the zoom (D-ui-40): room for hh:mm:ss.nnnnnnnnn → hh:mm:ss.nnnnnnnnn */
    min-width: 26ch;
    text-align: center;
  }
  .rangebtn:hover {
    background: var(--raised);
  }
  :global(.timemenu .meta) {
    font-family: var(--font-data);
  }
  .rangebar #hoverTxt {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
  }
  .seg {
    display: flex;
    gap: 2px;
  }
  .tb {
    display: inline-grid;
    place-items: center;
    min-width: 32px;
    min-height: 30px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
  }
  .tb :global(svg) {
    width: 16px;
    height: 16px;
  }
  .tb:hover:not(:disabled) {
    background: var(--raised);
    color: var(--fg);
  }
  .tb[aria-pressed='true'] {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--fg);
  }
  /* as the prototype's: a locked view says so in the warning colour */
  #b-lock[aria-pressed='true'] {
    color: var(--warn);
  }
  .tb:disabled {
    opacity: 0.4;
  }
  .vsep {
    align-self: center;
    width: 1px;
    height: 18px;
    background: var(--border);
  }
  .viewinfo {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    max-width: 46vw;
    margin-right: 6px;
    font-size: 12px;
  }
  .vprov {
    overflow: hidden;
    color: var(--fg-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
