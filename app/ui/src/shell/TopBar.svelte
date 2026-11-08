<!-- The top bar: tabs and + (open a saved layout) · time range · mouse mode, Clear, Back, Fit · ⋯ with the rest. A
     tab's ⌄ menu saves its layout (Save as… turns a file tab into a workspace tab), reverts or deletes it, and closes
     tabs. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { type Mode, PANELS, setSetting, settings, toggleAllPanels } from '../settings.svelte';
  import { play, playActions } from '../play.svelte';
  import { moveTab, tabs } from '../tabs.svelte';
  import { TIME_FORMATS } from '../axes';
  import { addPaneBelow, clearChannels, clearPanes, stack } from '../stack.svelte';
  import { actions, ui } from '../ui.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';

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
    tabsEl.querySelector('[aria-selected="true"]')?.scrollIntoView({ inline: 'nearest', block: 'nearest' });
    return () => ro.disconnect();
  });
  /** The bar's width: under 900 px Clear, Back, Fit, Play and Lock move into ⋯. */
  let barW = $state(1200);
  const narrow = $derived(barW < 900);
  /** The menu on the time range: how times read. */
  let timeMenu = $state<HTMLElement | null>(null);
  /** The tab whose menu is open, and where; `naming` while Save as… asks for a name. */
  let tabMenu = $state<null | { key: number; anchor?: HTMLElement; at?: { x: number; y: number }; naming?: 'saveas' | 'share' }>(null);
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
  const moreItems = (): MenuItem[] => [
    { label: 'Open recording…', kbd: '⌘O', run: actions.open },
    ...(narrow
      ? [
          { heading: 'Bar' },
          { label: 'Back', kbd: '⌥←', data: { more: 'back' }, disabled: !ui.canBack, run: actions.back },
          { label: 'Fit', kbd: 'F', data: { more: 'fit' }, disabled: !ui.file, run: actions.fit },
          { label: 'Play back', kbd: 'Space', checked: play.open, data: { more: 'play' }, disabled: !ui.file, run: () => (play.open ? playActions.close() : playActions.open()) },
        ]
      : []),
    { heading: 'View' },
    { label: 'Lock the time view', kbd: 'L', checked: settings.locked, run: () => setSetting('locked', !settings.locked) },
    { label: 'Clear cursors', kbd: 'Esc', disabled: !ui.anyCursor, run: actions.clearCursors },
    { label: 'Overview above the panes', checked: settings.overviewTop, run: () => setSetting('overviewTop', !settings.overviewTop) },
    { heading: 'All panes' },
    { label: 'New pane', kbd: '⌘⇧N', data: { more: 'addpane' }, disabled: !ui.file, run: () => addPaneBelow(Math.max(0, stack.panes.length - 1)) },
    { label: 'Remove every channel', data: { more: 'nochan' }, disabled: !stack.panes.some((p) => p.channels.length), run: clearChannels },
    { label: 'Remove every pane', data: { more: 'nopanes' }, disabled: !stack.panes.length, run: clearPanes },
    { heading: 'Window' },
    { label: settings.panelsClosed.length >= PANELS.length ? 'Expand all sidebar panels' : 'Collapse all sidebar panels', data: { more: 'panels' }, run: toggleAllPanels },
    { label: 'Switch theme', kbd: 'T', data: { more: 'theme' }, run: actions.toggleTheme },
    { label: 'Focus mode', kbd: '⌘.', data: { more: 'focus' }, run: () => actions.toggleFocus(true) },
    { label: 'Show performance details', checked: settings.showPerf, run: () => setSetting('showPerf', !settings.showPerf) },
    { label: 'Settings…', run: () => setSetting('leftPanel', 'settings') },
  ];
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
    // ⌘S saves the layout in front (a file tab asks for a name)
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's' && tabs.active >= 0) {
      e.preventDefault();
      const t = tabs.list.find((t) => t.key === tabs.active);
      if (t?.ws) actions.saveWs(t.key);
      else if (t && !t.live) {
        const el = document.querySelector<HTMLElement>(`[data-tab="${t.key}"]`);
        openTabMenu(t.key, el ? { anchor: el } : { at: { x: 8, y: 44 } });
        tabMenu!.naming = 'saveas';
        startNaming('');
      }
      return;
    }
  }
</script>

<svelte:window onkeydowncapture={key} />

<header class="toolbar" bind:clientWidth={barW}>
  {#if tabsOver}<button class="tb tscroll" id="tabsLeft" aria-label="Scroll tabs left" disabled={tabsAt <= 0} onclick={() => tabsEl?.scrollBy({ left: -200, behavior: 'smooth' })}>‹</button>{/if}
  <div class="wstabs" role="tablist" aria-label="Open tabs" id="wsTabs" bind:this={tabsEl} onscroll={measureTabs}>
    {#each tabs.list as t, i (t.key)}
      <div
        class="wstab"
        role="tab"
        tabindex="0"
        aria-selected={t.key === tabs.active}
        data-tab={t.key}
        data-tip={t.path}
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
          if (dragTab != null) moveTab(dragTab, i);
          dragTab = null;
          actions.saveSession();
        }}
        ondragend={() => (dragTab = null)}
      >
        {@html t.ws ? ICON.layout : ICON.file}<span class="nm">{t.name}</span>{#if t.live}<span class="livedot" data-live-tab aria-label="live"></span>{/if}{#if t.dirty}<span class="dirty" data-dirty aria-label="unsaved changes" data-tip="Unsaved changes: Save layout (⌘S) or Revert changes in the tab menu"></span>{/if}
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
    <button
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
    >
  </div>
  {#if tabsOver}<button class="tb tscroll" id="tabsRight" aria-label="Scroll tabs right" disabled={tabsAt >= tabsMax - 1} onclick={() => tabsEl?.scrollBy({ left: 200, behavior: 'smooth' })}>›</button>{/if}
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
    <button class="tb" id="b-lock" aria-pressed={settings.locked} aria-label="Lock" data-tip="Lock the time view so zoom, pan and fit gestures can't change it (L)" onclick={() => setSetting('locked', !settings.locked)}>{@html ICON.lock}</button>
  {/if}
  <button class="tb" id="b-more" aria-label="More" aria-haspopup="menu" aria-expanded={more} bind:this={moreBtn} onclick={() => (more = !more)}>{@html ICON.moreBar}</button>
  {#if tabMenu}
    <Menu class="tabmenu" label="Tab" anchor={tabMenu?.anchor} at={tabMenu?.at} onclose={() => (tabMenu = null)}>
      {#if tabMenu.naming}
        <form class="wsname" onsubmit={(e) => nameSubmit(e, (n) => (tabMenu!.naming === 'share' ? actions.share : actions.saveAs)(tabMenu!.key, n))}>
          <label for="wsName">{tabMenu.naming === 'share' ? 'Share with this folder as' : 'Save as a layout named'}</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="wsName" class="sel" maxlength="60" autocomplete="off" spellcheck="false" autofocus bind:value={wsName} oninput={() => (wsErr = '')} />
          {#if wsErr}<span class="wserr" role="alert">{wsErr}</span>{/if}
          {#if tabMenu.naming === 'share'}<span class="wshint">Writes .tlmbench/views/ in {settings.folder || 'the folder'}: the layout, the file and the time range in view.</span>{/if}
          <div class="wsbtns">
            <button class="btn" type="button" onclick={() => (tabMenu = null)}>Cancel</button><button class="btn primary" type="submit"
              >{tabMenu.naming === 'share' ? 'Share' : 'Save'}</button
            >
          </div>
        </form>
      {:else}
        {#if menuTab?.ws && !menuTab.shared}
          <button class="mi" role="menuitem" data-ws="save" disabled={!menuTab.dirty} onclick={() => tabAct(actions.saveWs)}><span class="nm">Save layout</span><kbd>⌘S</kbd></button>
        {/if}
        {#if !menuTab?.live}
          <button
            class="mi"
            role="menuitem"
            data-ws="saveas"
            onclick={() => (startNaming(menuTab?.ws ? (menuTab.shared ? menuTab.ws : `${menuTab.ws} copy`) : ''), (tabMenu!.naming = 'saveas'))}
            ><span class="nm">Save as…</span>{#if menuTab?.shared}<span class="hint">your copy</span>{/if}</button
          >
        {/if}
        {#if menuTab?.ws}
          <button class="mi" role="menuitem" data-ws="revert" disabled={!menuTab.dirty} onclick={() => tabAct(actions.revertWs)}><span class="nm">Revert changes</span></button>
        {/if}
        {#if !menuTab?.live}
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
          <button class="mi" role="menuitem" data-ws="delete" onclick={() => tabAct(() => actions.deleteWs(menuTab!.ws!))}><span class="nm">Delete layout</span></button>
        {/if}
        {#if !menuTab?.live}<div class="msep"></div>{/if}
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
              }}>{@html ICON.layout}<span class="nm">{w.name}</span><span class="hint">{open ? 'open in a tab' : `${w.panes.length} ${w.panes.length === 1 ? 'pane' : 'panes'}`}</span></button
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
        { heading: 'Time format' },
        ...TIME_FORMATS.map(([k, name, ex]) => ({ label: name, meta: ex, checked: settings.timeFormat === k, radio: true, data: { tf: k }, run: () => setSetting('timeFormat', k) })),
        { sep: true },
        { label: 'More in Settings…', run: () => (setSetting('settingsAll', true), setSetting('leftPanel', 'settings')) },
      ]}
    />
  {/if}
  {#if more}
    <Menu class="moremenu" label="More" anchor={moreBtn} onclose={() => (more = false)} items={moreItems()} />
  {/if}
</header>

<style>
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
    width: min(320px, calc(100vw - 32px));
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
    border: 0;
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
    background: var(--accent-soft);
    color: var(--fg);
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
</style>
