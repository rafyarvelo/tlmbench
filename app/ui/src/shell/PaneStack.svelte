<!-- The pane stack: rows of columns of panes. A pane's title shows its number (drag it to move the pane) and its
     channel chips with their values at the pointer; on hover or selection it adds ⋯ (also on right-click), whose
     menu closes, duplicates and splits panes. app.ts draws each pane's plot and handles the pointer over it. -->
<script lang="ts">
  import { PAD } from '../axes';
  import { ICON } from '../icons';
  import {
    addPaneBelow,
    canSplitRight,
    closeBelow,
    closeOthers,
    closePane,
    columns,
    duplicate,
    hasBelow,
    labelled,
    movePane,
    rows,
    splitDown,
    splitRight,
    stack,
  } from '../stack.svelte';
  import { ui } from '../ui.svelte';

  let menu = $state<null | { i: number; x: number; y: number }>(null);
  let dragging = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  const items = (i: number) => [
    { label: 'Close others', run: () => closeOthers(i), off: stack.panes.length < 2 },
    { label: 'Close panes below', run: () => closeBelow(i), off: !hasBelow(i) },
    { label: 'Close pane', run: () => closePane(i) },
    { sep: true },
    { label: 'Duplicate', run: () => duplicate(i), off: !stack.panes[i]?.channels.length },
    { label: 'Split right', run: () => splitRight(i), off: !canSplitRight(i) },
    { label: 'Split down', run: () => splitDown(i) },
    { label: 'Add a pane below', run: () => addPaneBelow(i) },
  ];

  function open(i: number, x: number, y: number) {
    stack.sel = i;
    menu = { i, x: Math.min(x, window.innerWidth - 230), y: Math.min(y, window.innerHeight - 300) };
  }
  function outside(e: PointerEvent) {
    if (menu && !(e.target as HTMLElement).closest('.panemenu, [data-pmenu]')) menu = null;
  }
  function key(e: KeyboardEvent) {
    if (menu && e.key === 'Escape') {
      menu = null;
      e.stopPropagation();
    }
  }
  function context(e: MouseEvent, i: number) {
    // a chip's right-click is the channel's menu (app.ts); elsewhere on the title, the pane's
    if ((e.target as HTMLElement).closest('[data-ch]')) return;
    e.preventDefault();
    open(i, e.clientX, e.clientY);
  }
</script>

<svelte:document onpointerdown={outside} />
<svelte:window onkeydowncapture={key} />

<div id="stack" class="stack">
  {#each rows() as row, r (r)}
    <div class="prow">
      {#each columns(row) as col, c (c)}
        <div class="pcol">
          {#each col as i (i)}
            {@const p = stack.panes[i]}
            <section class="pane" class:sel={i === stack.sel} class:drop={dropAt === i} data-pane={i}>
              <div
                class="phead"
                role="group"
                aria-label="Pane {i + 1}"
                oncontextmenu={(e) => context(e, i)}
                ondragover={(e) => {
                  if (dragging != null) {
                    e.preventDefault();
                    dropAt = i;
                  }
                }}
                ondragleave={() => (dropAt = null)}
                ondrop={(e) => {
                  e.preventDefault();
                  if (dragging != null) movePane(dragging, i);
                  dragging = dropAt = null;
                }}
              >
                <span
                  class="pnum"
                  role="button"
                  tabindex="-1"
                  draggable="true"
                  data-tip="Drag to move this pane"
                  ondragstart={(e) => {
                    dragging = i;
                    e.dataTransfer?.setData('text/plain', `pane ${i + 1}`);
                  }}
                  ondragend={() => (dragging = dropAt = null)}>{i + 1}</span
                >
                {#each p.channels as ch (ch)}
                  {@const info = ui.channels[ch]}
                  <span class="chip" data-ch={ch} style="--c: var(--trace-{(ch % 8) + 1})"
                    >{info?.name ?? ch}{#if info?.unit}<span class="unit">{info.unit}</span>{/if}<span class="val" data-val={ch}
                      >{stack.values[ch] ?? ''}</span
                    ></span
                  >
                {:else}
                  <span class="mnote">Tick channels on the left to plot them here</span>
                {/each}
                <span class="spacer"></span>
                {#if stack.coarse[i]}
                  <span class="pill" data-coarse data-tip="Drawn from summaries; the exact view follows">coarse</span>
                {/if}
                <button
                  class="pmore"
                  data-pmenu
                  aria-haspopup="menu"
                  aria-label="More for pane {i + 1}"
                  data-tip="Close, duplicate or split (also on right-click)"
                  onclick={(e) => {
                    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
                    open(i, b.right - 220, b.bottom + 4);
                  }}>{@html ICON.more}</button
                >
              </div>
              <div class="plot" style="--pb: {labelled(i) ? PAD.timeLabels : 4}px"></div>
            </section>
          {/each}
        </div>
      {/each}
    </div>
  {/each}
</div>

{#if menu}
  <div class="menu panemenu" role="menu" aria-label="Pane {menu.i + 1}" style="left: {menu.x}px; top: {menu.y}px">
    {#each items(menu.i) as it, k (k)}
      {#if it.sep}
        <div class="msep"></div>
      {:else}
        <button
          class="mi"
          role="menuitem"
          disabled={it.off}
          onclick={() => {
            menu = null;
            it.run?.();
          }}><span class="nm">{it.label}</span></button
        >
      {/if}
    {/each}
  </div>
{/if}

<style>
  .stack {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .prow {
    display: flex;
    flex: 1;
    min-height: 0;
    gap: 8px;
  }
  .pcol {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .pane.drop {
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .pnum {
    cursor: grab;
  }
  .pmore {
    display: inline-grid;
    place-items: center;
    width: 26px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    visibility: hidden;
  }
  .pmore :global(svg) {
    width: 14px;
    height: 14px;
  }
  .pmore:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .phead:hover .pmore,
  .pane.sel .pmore,
  .pmore:focus-visible {
    visibility: visible;
  }
  .panemenu {
    min-width: 210px;
  }
  .msep {
    height: 1px;
    margin: 4px 6px;
    background: var(--grid);
  }
  .mi:disabled {
    opacity: 0.45;
  }
</style>
