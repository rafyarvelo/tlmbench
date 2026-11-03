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
    setMode,
    splitDown,
    splitRight,
    stack,
  } from '../stack.svelte';
  import { ui } from '../ui.svelte';
  import Table from './Table.svelte';

  let menu = $state<null | { i: number; x: number; y: number }>(null);
  let views = $state<null | { i: number; x: number; y: number }>(null);
  const VIEWS = [
    { mode: 'chart', label: 'Chart', hint: 'Channels over time' },
    { mode: 'table', label: 'Table', hint: 'The samples in view, one row each' },
    { mode: 'both', label: 'Chart and table', hint: 'The chart with its table below' },
    { mode: 'values', label: 'Values', hint: 'Value tiles at the pointer, cursor A or the end of the view' },
  ] as const;
  const viewName = (i: number) => VIEWS.find((v) => v.mode === (stack.panes[i]?.mode ?? 'chart'))!.label;
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
    if (views && !(e.target as HTMLElement).closest('.viewmenu, [data-vmenu]')) views = null;
  }
  function key(e: KeyboardEvent) {
    if ((menu || views) && e.key === 'Escape') {
      menu = views = null;
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
    <div class="prow" style:flex-grow={row.some((k) => stack.panes[k].mode === 'both') ? 2 : null}>
      {#each columns(row) as col, c (c)}
        <div class="pcol">
          {#each col as i (i)}
            {@const p = stack.panes[i]}
            <section class="pane" class:both={p.mode === 'both'} class:sel={i === stack.sel} class:drop={dropAt === i} data-pane={i}>
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
                  data-vmenu
                  aria-haspopup="menu"
                  aria-label="View of pane {i + 1}: {viewName(i)}"
                  data-tip="View: {viewName(i).toLowerCase()}. Click to change"
                  onclick={(e) => {
                    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
                    menu = null;
                    views = views?.i === i ? null : { i, x: Math.min(b.right - 250, window.innerWidth - 270), y: b.bottom + 4 };
                  }}>{@html ICON.view}</button
                >
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
              {#if p.mode === 'values'}
                <div class="tiles" data-tiles>
                  {#each p.channels as ch (ch)}
                    {@const info = ui.channels[ch]}
                    <div class="tile" style="--c: var(--trace-{(ch % 8) + 1})" data-tile={ch}>
                      <span class="tname">{info?.name ?? ch}</span>
                      <span class="tval">{stack.values[ch] ?? stack.resting[ch] ?? '–'}</span>
                      {#if info?.unit}<span class="tunit">{info.unit}</span>{/if}
                    </div>
                  {:else}
                    <span class="mnote">Tick channels on the left to show them here</span>
                  {/each}
                </div>
              {:else if p.mode === 'table'}
                <Table {i} />
              {:else}
                <div class="plot" style="--pb: {labelled(i) ? PAD.timeLabels : 4}px"></div>
                {#if p.mode === 'both'}<Table {i} height={p.tableH ?? 210} />{/if}
              {/if}
            </section>
          {/each}
        </div>
      {/each}
    </div>
  {/each}
</div>

{#if views}
  <div class="menu viewmenu" role="menu" aria-label="View of pane {views.i + 1}" style="left: {views.x}px; top: {views.y}px">
    {#each VIEWS as v (v.mode)}
      {@const on = (stack.panes[views.i]?.mode ?? 'chart') === v.mode}
      <button
        class="mi"
        role="menuitemradio"
        aria-checked={on}
        data-vm={v.mode}
        onclick={() => {
          const i = views!.i;
          views = null;
          setMode(i, v.mode);
        }}><span class="nm">{v.label}<small>{v.hint}</small></span>{#if on}{@html ICON.check}{/if}</button
      >
    {/each}
  </div>
{/if}

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
  /* a chart with its table below takes twice the room, and the chart keeps some of it */
  .pane.both {
    flex-grow: 2;
  }
  .pane.both .plot {
    min-height: 100px;
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
  .tiles {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-content: flex-start;
    gap: 8px;
    min-height: 0;
    padding: 8px;
    overflow: auto;
  }
  .tile {
    display: flex;
    flex-direction: column;
    min-width: 120px;
    padding: 8px 12px;
    border: 1px solid var(--grid);
    border-left: 3px solid var(--c);
    border-radius: 8px;
  }
  .tname {
    font-size: 11.5px;
    color: var(--fg-muted);
  }
  .tval {
    font-family: var(--font-data);
    font-size: 26px;
  }
  .tunit {
    font-family: var(--font-data);
    font-size: 11px;
    color: var(--fg-muted);
  }
  .viewmenu small {
    display: block;
    font-size: 11px;
    color: var(--fg-muted);
  }
  .panemenu,
  .viewmenu {
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
