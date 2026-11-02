<!-- The top bar: tabs · time range · mouse mode, Clear, Back, Fit · ⋯ with the rest. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { type Mode, setSetting, settings } from '../settings.svelte';
  import { play, playActions } from '../play.svelte';
  import { moveTab, tabs } from '../tabs.svelte';
  import { actions, ui } from '../ui.svelte';

  const MODES: [Mode, string, string][] = [
    ['box', 'Box zoom', 'Drag across a plot to zoom into that range (B)'],
    ['pan', 'Pan', 'Drag to pan (P). In other modes, Shift-drag or middle-drag pans.'],
    ['cursor', 'Cursors', 'Click to place cursors A and B (C)'],
  ];
  let more = $state(false);
  /** The tab whose menu is open, and where. */
  let tabMenu = $state<null | { key: number; x: number; y: number }>(null);
  let dragTab = $state<number | null>(null);
  /** Runs a tab menu item on the menu's tab, closing the menu first. */
  function tabAct(f: (key: number) => void) {
    const key = tabMenu?.key;
    tabMenu = null;
    if (key != null) f(key);
  }
  function openTabMenu(key: number, x: number, y: number) {
    tabMenu = { key, x: Math.min(x, window.innerWidth - 220), y };
  }
  let moreBtn: HTMLButtonElement | undefined = $state();
  const pick = (f: () => void) => () => {
    more = false;
    f();
  };
  function outside(e: PointerEvent) {
    if (more && !(e.target as HTMLElement).closest('.moremenu, #b-more')) more = false;
    if (tabMenu && !(e.target as HTMLElement).closest('.tabmenu, .tm')) tabMenu = null;
  }
  function key(e: KeyboardEvent) {
    if (tabMenu && e.key === 'Escape') {
      tabMenu = null;
      e.stopPropagation();
      return;
    }
    if (more && e.key === 'Escape') {
      more = false;
      e.stopPropagation();
      moreBtn?.focus();
    }
  }
</script>

<svelte:document onpointerdown={outside} />
<svelte:window onkeydowncapture={key} />

<header class="toolbar">
  <div class="wstabs" role="tablist" aria-label="Open tabs" id="wsTabs">
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
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), actions.showTab(t.key))}
        onauxclick={(e) => e.button === 1 && actions.closeTab(t.key)}
        oncontextmenu={(e) => (e.preventDefault(), openTabMenu(t.key, e.clientX, e.clientY))}
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
        {@html ICON.file}<span class="nm">{t.name}</span>
        <span
          class="tm"
          role="button"
          tabindex="-1"
          aria-label="Menu for {t.name}"
          aria-haspopup="menu"
          onclick={(e) => {
            e.stopPropagation();
            const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
            openTabMenu(t.key, r.left, r.bottom + 4);
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
  </div>
  <div class="rangebar">
    <span class="num" id="rangeTxt">{ui.range}</span>
    <span class="muted" id="spanTxt">{ui.span}</span>
    <span class="num muted" id="hoverTxt">{ui.pointer}</span>
  </div>
  <div class="seg" role="group" aria-label="Mouse mode">
    {#each MODES as [m, label, tip] (m)}
      <button class="tb" id="m-{m}" aria-pressed={settings.mode === m} aria-label={label} data-tip={tip} onclick={() => setSetting('mode', m)}>{@html ICON[m]}</button>
    {/each}
  </div>
  {#if ui.anyCursor}
    <button class="tb" id="b-clear" aria-label="Clear cursors" data-tip="Remove cursors A and B (Esc)" onclick={actions.clearCursors}>{@html ICON.clear}</button>
  {/if}
  <button class="tb" id="b-back" aria-label="Back" disabled={!ui.canBack} data-tip="Previous view (⌥←)" onclick={actions.back}>{@html ICON.back}</button>
  <button class="tb" id="b-fit" aria-label="Fit" disabled={!ui.file} data-tip="Show the whole recording (F)" onclick={actions.fit}>{@html ICON.fit}</button>
  <button class="tb" id="b-play" aria-label="Play back" aria-pressed={play.open} disabled={!ui.file} data-tip="Play back: a playhead moves through the file and the view follows it (Space plays and pauses)" onclick={() => (play.open ? playActions.close() : playActions.open())}>{@html ICON.play}</button>
  <button class="tb" id="b-more" aria-label="More" aria-haspopup="menu" aria-expanded={more} bind:this={moreBtn} onclick={() => (more = !more)}>{@html ICON.more}</button>
  {#if tabMenu}
    <div class="menu tabmenu" role="menu" style="left: {tabMenu.x}px; top: {tabMenu.y}px">
      <button class="mi" role="menuitem" onclick={() => tabAct(actions.closeTab)}><span class="nm">Close tab</span></button>
      <button class="mi" role="menuitem" disabled={tabs.list.length < 2} onclick={() => tabAct(actions.closeOtherTabs)}
        ><span class="nm">Close other tabs</span></button
      >
      <button class="mi" role="menuitem" onclick={() => tabAct(() => actions.closeFile())}><span class="nm">Close all tabs</span></button>
    </div>
  {/if}
  {#if more}
    <div class="menu moremenu" role="menu">
      <button class="mi" role="menuitem" onclick={pick(actions.open)}><span class="nm">Open recording…</span><kbd>⌘O</kbd></button>
      <button class="mi" role="menuitem" disabled={!ui.anyCursor} onclick={pick(actions.clearCursors)}><span class="nm">Clear cursors</span><kbd>Esc</kbd></button>
      <button class="mi" role="menuitemcheckbox" aria-checked={settings.overviewTop} onclick={pick(() => setSetting('overviewTop', !settings.overviewTop))}><span class="nm">Overview above the panes</span></button>
      <button class="mi" role="menuitemcheckbox" aria-checked={settings.showPerf} onclick={pick(() => setSetting('showPerf', !settings.showPerf))}><span class="nm">Show performance details</span></button>
      <button class="mi sep" role="menuitem" onclick={pick(() => setSetting('leftPanel', 'settings'))}><span class="nm">Settings…</span></button>
    </div>
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
  }
  .wstab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
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
    width: 18px;
    height: 18px;
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
  .tabmenu {
    position: fixed;
    min-width: 200px;
  }
  .wstab :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
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
  .moremenu {
    position: absolute;
    top: 40px;
    right: 8px;
  }
  .mi[aria-checked='true']::after {
    content: '✓';
    margin-left: 12px;
    color: var(--accent);
  }
  .mi kbd {
    color: var(--fg-muted);
    font: 11px var(--font-data);
  }
  .mi.sep {
    margin-top: 4px;
    border-top: 1px solid var(--grid);
    border-radius: 0 0 8px 8px;
  }
  .mi:disabled {
    opacity: 0.45;
  }
</style>
