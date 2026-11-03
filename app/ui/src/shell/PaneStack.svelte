<!-- The pane stack: rows of columns of panes. A pane's title shows its number (drag it to move the pane) and its
     channel chips with their values at the pointer; on hover or selection it adds ⋯ (also on right-click), whose
     menu closes, duplicates and splits panes. app.ts draws each pane's plot and handles the pointer over it. -->
<script lang="ts">
  import { PAD } from '../axes';
  import { ICON } from '../icons';
  import {
    addChannels,
    addPaneBelow,
    canSplitRight,
    closeBelow,
    closeOthers,
    closePane,
    columns,
    duplicate,
    hasBelow,
    hasChart,
    labelled,
    moveChannel,
    movePane,
    newPaneWith,
    removeChannel,
    rightPad,
    rows,
    setMode,
    setPlotH,
    splitDown,
    splitRight,
    stack,
    units,
    axisKey,
  } from '../stack.svelte';
  import { ui } from '../ui.svelte';
  import AxisDialog from './AxisDialog.svelte';
  import Table from './Table.svelte';

  let menu = $state<null | { i: number; x: number; y: number }>(null);
  let views = $state<null | { i: number; x: number; y: number }>(null);
  let axes = $state<null | { i: number; x: number; y: number }>(null);
  const VIEWS = [
    { mode: 'chart', label: 'Chart', hint: 'Channels over time' },
    { mode: 'table', label: 'Table', hint: 'The samples in view, one row each' },
    { mode: 'both', label: 'Chart and table', hint: 'The chart with its table below' },
    { mode: 'values', label: 'Values', hint: 'Value tiles at the pointer, cursor A or the end of the view' },
  ] as const;
  const viewName = (i: number) => VIEWS.find((v) => v.mode === (stack.panes[i]?.mode ?? 'chart'))!.label;
  /** A channel dragged from Explorer: the pane it would land in (-1: a new pane). */
  let chanOver = $state<number | null>(null);
  const isChans = (e: DragEvent) => !!e.dataTransfer?.types.includes('application/x-tlm-channels');
  const chansOf = (e: DragEvent): number[] => {
    try {
      return JSON.parse(e.dataTransfer?.getData('application/x-tlm-channels') ?? '[]');
    } catch {
      return [];
    }
  };
  function overChans(e: DragEvent, to: number) {
    if (!isChans(e) && !isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = to;
  }
  function dropChans(e: DragEvent, to: number) {
    if (isChip(e)) return dropChip(e, to);
    if (!isChans(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = null;
    const chs = chansOf(e);
    if (!chs.length) return;
    if (to < 0) newPaneWith(chs);
    else addChannels(chs, to);
  }
  /** Dragging the grip under a chart: its pane, where it started and the height then. */
  let grip: null | { i: number; y: number; h: number } = null;
  function gripDown(e: PointerEvent, i: number) {
    const plot = (e.currentTarget as HTMLElement).parentElement?.querySelector('.plot');
    grip = { i, y: e.clientY, h: plot?.getBoundingClientRect().height ?? 150 };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function gripMove(e: PointerEvent) {
    if (grip) setPlotH(grip.i, grip.h + e.clientY - grip.y);
  }
  function gripKey(e: KeyboardEvent, i: number) {
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    e.preventDefault();
    const h = (e.currentTarget as HTMLElement).parentElement?.querySelector('.plot')?.getBoundingClientRect().height ?? 150;
    setPlotH(i, h + (e.key === 'ArrowDown' ? 10 : -10));
  }
  /** A chip dragged by its name: its channel and pane (drop on a chip to reorder, on a pane to move; Alt copies). */
  const isChip = (e: DragEvent) => !!e.dataTransfer?.types.includes('application/x-tlm-chip');
  const chipOf = (e: DragEvent): null | { ch: number; from: number } => {
    try {
      return JSON.parse(e.dataTransfer?.getData('application/x-tlm-chip') ?? 'null');
    } catch {
      return null;
    }
  };
  function dropChip(e: DragEvent, to: number, before: number | null = null) {
    if (!isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = null;
    const c = chipOf(e);
    if (c) moveChannel(c.ch, c.from, to, before, e.altKey);
  }
  /** Panes whose header can't show every chip: how many are folded into "+N", and the open list. */
  let folded = $state<Record<number, number>>({});
  let chanList = $state<null | { i: number; x: number; y: number }>(null);
  /** Hides the chips that don't fit the header, from the last, and counts them. */
  function fitChips(node: HTMLElement, i: number) {
    const fit = () => {
      const chips = [...node.querySelectorAll<HTMLElement>('.chip')];
      chips.forEach((c) => (c.hidden = false));
      let n = 0;
      while (node.scrollWidth > node.clientWidth + 1 && n < chips.length - 1) chips[chips.length - 1 - n++].hidden = true;
      if ((folded[i] ?? 0) !== n) folded[i] = n;
    };
    const ro = new ResizeObserver(fit);
    ro.observe(node);
    const mo = new MutationObserver(fit);
    mo.observe(node, { childList: true });
    fit();
    return {
      update(k: number) {
        i = k;
        fit();
      },
      destroy() {
        ro.disconnect();
        mo.disconnect();
      },
    };
  }
  let dragging = $state<number | null>(null);
  let dropAt = $state<number | null>(null);

  const items = (i: number) => [
    ...(hasChart(i) ? [{ label: 'Axis and scale…', run: () => (axes = { ...menuAt!, i }) }, { sep: true }] : []),
    { label: 'Close others', run: () => closeOthers(i), off: stack.panes.length < 2 },
    { label: 'Close panes below', run: () => closeBelow(i), off: !hasBelow(i) },
    { label: 'Close pane', run: () => closePane(i) },
    { sep: true },
    { label: 'Duplicate', run: () => duplicate(i), off: !stack.panes[i]?.channels.length },
    { label: 'Split right', run: () => splitRight(i), off: !canSplitRight(i) },
    { label: 'Split down', run: () => splitDown(i) },
    { label: 'Add a pane below', run: () => addPaneBelow(i) },
  ];

  let menuAt: { x: number; y: number } | null = null;
  function open(i: number, x: number, y: number) {
    stack.sel = i;
    menu = { i, x: Math.min(x, window.innerWidth - 230), y: Math.min(y, window.innerHeight - 300) };
    // the axes dialog opens where the menu was
    menuAt = { x: Math.min(menu.x, window.innerWidth - 400), y: Math.min(menu.y, window.innerHeight - 460) };
  }
  function outside(e: PointerEvent) {
    if (menu && !(e.target as HTMLElement).closest('.panemenu, [data-pmenu]')) menu = null;
    if (views && !(e.target as HTMLElement).closest('.viewmenu, [data-vmenu]')) views = null;
    if (chanList && !(e.target as HTMLElement).closest('.chanlist, [data-more-chips]')) chanList = null;
    if (axes && !(e.target as HTMLElement).closest('.axdlg, .panemenu')) axes = null;
  }
  function key(e: KeyboardEvent) {
    if ((menu || views || axes) && e.key === 'Escape') {
      menu = views = axes = null;
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

<div id="stack" class="stack" class:chanover={chanOver === -1} ondragover={(e) => overChans(e, -1)} ondragleave={() => (chanOver = null)} ondrop={(e) => dropChans(e, -1)} role="presentation" style="--ph: {rows().length <= 3 ? 150 : rows().length === 4 ? 125 : 105}px">
  {#each rows() as row, r (r)}
    <div class="prow" class:grow={row.some((k) => stack.panes[k].mode === 'table' || stack.panes[k].mode === 'values')}>
      {#each columns(row) as col, c (c)}
        <div class="pcol">
          {#each col as i (i)}
            {@const p = stack.panes[i]}
            <section class="pane" class:grow={p.mode === 'table' || p.mode === 'values'} class:sel={i === stack.sel} class:drop={dropAt === i} class:chanover={chanOver === i} data-pane={i} style={p.h ? `--ph: ${p.h}px` : undefined} ondragover={(e) => overChans(e, i)} ondrop={(e) => dropChans(e, i)} role="presentation">
              <div
                class="phead"
                use:fitChips={i}
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
                  <span
                    class="chip"
                    data-ch={ch}
                    style="--c: var(--trace-{(ch % 8) + 1})"
                    draggable="true"
                    role="presentation"
                    ondragstart={(e) => {
                      e.dataTransfer?.setData('application/x-tlm-chip', JSON.stringify({ ch, from: i }));
                      e.dataTransfer?.setData('text/plain', info?.name ?? String(ch));
                    }}
                    ondragover={(e) => isChip(e) && (e.preventDefault(), e.stopPropagation())}
                    ondrop={(e) => dropChip(e, i, ch)}
                    >{info?.name ?? ch}{#if info?.unit}<span class="unit">{info.unit}</span>{/if}{#if units(i).length > 1}<span class="side" data-side>· {axisKey(i, ch) === 'u1' ? 'R' : 'L'}</span>{/if}<span class="val" data-val={ch}
                      >{stack.values[ch] ?? ''}</span
                    ><button class="cx" data-chip-x={ch} aria-label="Take {info?.name ?? ch} out of pane {i + 1}" data-tip="Take out of this pane" onclick={(e) => (e.stopPropagation(), removeChannel(ch, i))}>{@html ICON.close}</button></span
                  >
                {:else}
                  <span class="mnote">Tick channels on the left to plot them here</span>
                {/each}
                {#if folded[i]}
                  <button class="more" data-more-chips={i} aria-haspopup="menu" data-tip="Every channel in this pane, with its value" onclick={(e) => {
                    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
                    chanList = chanList?.i === i ? null : { i, x: Math.min(b.left, window.innerWidth - 260), y: b.bottom + 4 };
                  }}>+{folded[i]}</button>
                {/if}
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
                <div class="plot" style="--pb: {labelled(i) ? PAD.timeLabels : 4}px; --pr: {rightPad()}px"></div>
                {#if p.mode !== 'both'}
                  <div
                    class="hgrip"
                    role="slider"
                    aria-orientation="vertical"
                    aria-valuemin={60}
                    aria-valuemax={800}
                    aria-valuenow={p.h ?? 150}
                    aria-label="Chart height"
                    tabindex="0"
                    data-tip="Drag to resize the chart (double-click resets, ↑ ↓ step)"
                    onpointerdown={(e) => gripDown(e, i)}
                    onpointermove={gripMove}
                    onpointerup={() => (grip = null)}
                    ondblclick={() => setPlotH(i, undefined)}
                    onkeydown={(e) => gripKey(e, i)}
                  ></div>
                {/if}
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

{#if axes && stack.panes[axes.i]}
  <AxisDialog i={axes.i} x={axes.x} y={axes.y} onclose={() => (axes = null)} />
{/if}

{#if chanList && stack.panes[chanList.i]}
  <div class="menu chanlist" role="menu" aria-label="Channels in pane {chanList.i + 1}" style="left: {chanList.x}px; top: {chanList.y}px">
    {#each stack.panes[chanList.i].channels as ch (ch)}
      {@const info = ui.channels[ch]}
      <div class="mi" role="menuitem" tabindex="-1" style="--c: var(--trace-{(ch % 8) + 1})"><span class="sw8" style="background: var(--c)"></span><span class="nm">{info?.name ?? ch}</span>{#if info?.unit}<span class="hint">{info.unit}</span>{/if}<span class="hint num">{stack.values[ch] ?? stack.resting[ch] ?? ''}</span></div>
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
    flex: 0 1 auto;
    min-height: 0;
    gap: 8px;
  }
  .pcol {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .prow.grow,
  .pane.grow {
    flex: 1 1 0;
  }
  .more {
    flex: none;
    height: 22px;
    padding: 0 6px;
    border: 0;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font: 11.5px var(--font-data);
  }
  .cx {
    display: none;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
  }
  .cx :global(svg) {
    width: 9px;
    height: 9px;
  }
  .cx:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .chip:hover .cx,
  .cx:focus-visible {
    display: inline-grid;
  }
  .chip[draggable='true'] {
    cursor: grab;
  }
  .side {
    color: var(--fg-muted);
    font: 11.5px var(--font-data);
  }
  .hgrip {
    flex: none;
    height: 6px;
    margin-top: -2px;
    cursor: row-resize;
    border-radius: 3px;
  }
  .hgrip:hover,
  .hgrip:active {
    background: var(--accent-line);
  }
  .pane.chanover {
    background: var(--accent-soft);
    outline: 2px dashed var(--accent-line);
    outline-offset: -2px;
  }
  .stack.chanover {
    box-shadow: inset 0 -2px 0 var(--accent);
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
