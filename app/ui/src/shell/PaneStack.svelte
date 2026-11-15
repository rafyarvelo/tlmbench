<!-- The pane stack: rows of columns of panes. A pane's title shows its number (drag it to move the pane) and its
     channel chips with their values at the pointer; on hover or selection it adds ⋯ (also on right-click), whose
     menu closes, duplicates and splits panes. app.ts draws each pane's plot and handles the pointer over it. -->
<script lang="ts">
  import { PAD } from '../axes';
  import { COPY_KEY, isCopy } from '../dnd';
  import { ICON } from '../icons';
  import { chCss, chLimits, chName, chUnit, limitText, openChannelEditor } from '../display.svelte';
  import {
    addChannels,
  newPaneBeside,
  newPaneBefore,
  MAX_PER_ROW,
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
    dropPane,
    type Zone,
    newPaneWith,
    removeChannel,
    rightPad,
    rows,
    setMode,
    setPlotH,
    setTable,
    setXy,
    splitDown,
    splitRight,
    stack,
    units,
    axisKey,
    xyAxisOf,
    xyEqualOf,
    paneEvents,
  } from '../stack.svelte';
  import { failingChannels } from '../checks.svelte';
  import { settings } from '../settings.svelte';
  import { actions, ui, viewLocked } from '../ui.svelte';
  import AxisDialog from './AxisDialog.svelte';
  import AddPane from './AddPane.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import ChecksPane from './ChecksPane.svelte';
  import Gauges from './Gauges.svelte';
  import XY from './XY.svelte';
  import Spectrum from './Spectrum.svelte';
  import Table from './Table.svelte';

  /** the pane menu: under its ⋯ button, or at the pointer on a right-click */
  const lk = $derived(viewLocked());
  const LOCKED = 'Unlock the view to change panes';
  let menu = $state<null | { i: number; anchor?: HTMLElement; at?: { x: number; y: number } }>(null);
  let views = $state<null | { i: number; anchor: HTMLElement }>(null);
  let axes = $state<null | { i: number; x: number; y: number }>(null);
  const VIEWS = [
    { mode: 'chart', label: 'Chart', hint: 'Channels over time' },
    { mode: 'table', label: 'Table', hint: 'The samples in view, one row each' },
    { mode: 'both', label: 'Chart and table', hint: 'The chart with its table below' },
    { mode: 'cv', label: 'Chart and values', hint: 'The chart with value tiles below it' },
    { mode: 'values', label: 'Values', hint: 'Value tiles at the pointer, cursor A or the end of the view' },
    { mode: 'gauges', label: 'Gauges', hint: 'One widget per channel: gauges, bars, rings, a compass, a horizon and more' },
    { mode: 'xy', label: 'XY plot', hint: 'One channel against another: a GPS track, current against throttle' },
    { mode: 'spectrum', label: 'Spectrum', hint: 'The frequency content of each channel over the time view' },
    { mode: 'checks', label: 'Checks', hint: 'What the rules and checks found: the sets, their groups and verdicts' },
  ] as const;
  const VIEW_ICON = { chart: ICON.vchart, table: ICON.vtable, both: ICON.vboth, cv: ICON.vboth, values: ICON.vvalues, spectrum: ICON.vspectrum, checks: ICON.vchecks, gauges: ICON.vgauges, xy: ICON.vxy } as const;
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
  /** A channel (or chip) is being dragged: "Drop here for a new pane" shows before every row. */
  let carrying = $state(false);
  let gapOver = $state<number | null>(null);
  const dragStart = (e: DragEvent) => (carrying = isChans(e) || isChip(e));
  function overGap(e: DragEvent, r: number) {
    if (!isChans(e) && !isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    gapOver = r;
  }
  function dropGap(e: DragEvent, first: number) {
    if (!isChans(e) && !isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    gapOver = chanOver = chanSide = null;
    carrying = false;
    if (isChip(e)) {
      const c = chipOf(e);
      if (c) newPaneBefore([c.ch], first, settings.dropNewView === 'same' ? stack.panes[c.from]?.mode : undefined, isCopy(e) ? undefined : c);
    } else {
      const chs = chansOf(e);
      if (chs.length) newPaneBefore(chs, first);
    }
  }
  /** Over a pane's left or right edge a dragged channel would get a pane of its own there (when the row has room). */
  let chanSide = $state<null | 'left' | 'right'>(null);
  function sideOf(e: DragEvent, to: number): null | 'left' | 'right' {
    if (to < 0 || !stack.panes[to]) return null;
    const row = rows().find((r) => r.includes(to)) ?? [to];
    const chip = chipOf(e);
    // a chip that is alone in its own column frees that column when it moves
    const freed = chip && !isCopy(e) && columns(row).some((c) => c.length === 1 && c[0] === chip.from && stack.panes[chip.from].channels.length === 1) ? 1 : 0;
    if (columns(row).length - freed >= MAX_PER_ROW) return null;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const f = (e.clientX - r.left) / r.width;
    return f < 0.22 ? 'left' : f > 0.78 ? 'right' : null;
  }
  function overChans(e: DragEvent, to: number) {
    if (!isChans(e) && !isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = to;
    chanSide = sideOf(e, to);
  }
  function dropChans(e: DragEvent, to: number) {
    if (isChip(e)) return dropChip(e, to);
    if (!isChans(e)) return;
    e.preventDefault();
    e.stopPropagation();
    const side = sideOf(e, to);
    chanOver = chanSide = null;
    const chs = chansOf(e);
    if (!chs.length) return;
    if (to < 0) newPaneWith(chs);
    else if (side) newPaneBeside(chs, to, side);
    else addChannels(chs, to);
  }
  /** Dragging the grip under a chart: its pane, where it started and the height then. */
  let grip: null | { i: number; y: number; h: number } = null;
  function gripDown(e: PointerEvent, i: number) {
    if (lk) return;
    const plot = (e.currentTarget as HTMLElement).parentElement?.querySelector('.plot, [data-xy] canvas');
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
    const h = (e.currentTarget as HTMLElement).parentElement?.querySelector('.plot, [data-xy] canvas')?.getBoundingClientRect().height ?? 150;
    setPlotH(i, h + (e.key === 'ArrowDown' ? 10 : -10));
  }
  /** A chip dragged by its name: its channel and pane (drop on a chip to reorder, on a pane to move; the copy key copies: isCopy). */
  const failing = $derived(ui.path ? failingChannels(ui.path) : new Map());
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
    const side = before == null ? sideOf(e, to) : null;
    chanOver = chanSide = null;
    const c = chipOf(e);
    if (!c) return;
    if (side) {
      const mode = settings.dropNewView === 'same' ? stack.panes[c.from]?.mode : undefined;
      newPaneBeside([c.ch], to, side, mode, isCopy(e) ? undefined : c);
    } else if (to < 0) {
      // onto empty space: a new pane, a chart or the view it came from; the channel leaves the old one unless the copy key is down
      const mode = settings.dropNewView === 'same' ? stack.panes[c.from]?.mode : undefined;
      newPaneWith([c.ch], mode, isCopy(e) ? undefined : c);
    } else moveChannel(c.ch, c.from, to, before, isCopy(e));
  }
  /** Panes whose header can't show every chip: how many are folded into "+N", and the open list. */
  let folded = $state<Record<number, number>>({});
  let chanList = $state<null | { i: number; anchor: HTMLElement }>(null);
  /** Hides the chips that don't fit the header, from the last, and counts them. */
  function fitChips(node: HTMLElement, i: number) {
    const fit = () => {
      const chips = [...node.querySelectorAll<HTMLElement>('.chip')];
      // the actions that can fit show (owner decision 9): the rarer ones give way first, each stays in ⋯; then chips fold
      const acts = ['.pact-split', '.pact-dup', '.pact-close', '.pact-axis', '.pact-ev'].map((q) => node.querySelector<HTMLElement>(q)).filter((a) => !!a);
      chips.forEach((c) => (c.hidden = false));
      acts.forEach((a) => (a.hidden = false));
      const over = () => node.scrollWidth > node.clientWidth + 1;
      for (let k = 0; over() && k < acts.length; k++) acts[k].hidden = true;
      let n = 0;
      while (over() && n < chips.length - 1) chips[chips.length - 1 - n++].hidden = true;
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
  /** Where on a pane a dragged pane would land: which pane and which part of it. */
  let zoneAt = $state<null | { i: number; zone: Zone }>(null);
  function zoneOf(e: DragEvent): Zone {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const [fx, fy] = [(e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height];
    return fx < 0.2 ? 'left' : fx > 0.8 ? 'right' : fy < 0.25 ? 'top' : fy > 0.75 ? 'bottom' : 'middle';
  }
  function paneOver(e: DragEvent, i: number) {
    if (dragging == null) return overChans(e, i);
    e.preventDefault();
    zoneAt = { i, zone: zoneOf(e) };
  }
  function paneDrop(e: DragEvent, i: number) {
    if (dragging == null) return dropChans(e, i);
    e.preventDefault();
    if (zoneAt) dropPane(dragging, i, zoneAt.zone, isCopy(e));
    dragging = dropAt = zoneAt = null;
  }

  const items = (i: number): MenuItem[] => [
    ...(hasChart(i)
      ? [
          { label: 'Event markers in this pane', checked: paneEvents(i), data: { pm: 'events' }, run: () => toggleEvents(i) },
          { label: 'Axis and scale…', run: () => (axes = { ...menuAt!, i }) },
          { sep: true } as const,
        ]
      : []),
    { label: 'Close others', run: () => closeOthers(i), disabled: stack.panes.length < 2 },
    { label: 'Close panes below', run: () => closeBelow(i), disabled: !hasBelow(i) },
    { label: 'Close pane', run: () => closePane(i) },
    { sep: true },
    { label: 'Duplicate', run: () => duplicate(i), disabled: !stack.panes[i]?.channels.length },
    { label: 'Duplicate beside', run: () => duplicate(i, true), disabled: !stack.panes[i]?.channels.length || !canSplitRight(i) },
    { label: 'Split right', run: () => splitRight(i), disabled: !canSplitRight(i) },
    { label: 'Split down', run: () => splitDown(i) },
    { label: 'Add a pane below', run: () => addPaneBelow(i) },
  ];

  /** This pane's own choice, the opposite of what it shows now; it then stays whatever the top bar's E does. */
  function toggleEvents(i: number) {
    stack.panes[i].events = !paneEvents(i);
    actions.eventsRedraw();
  }

  const viewItems = (i: number): MenuItem[] =>
    VIEWS.map((v) => ({ label: v.label, note: v.hint, icon: VIEW_ICON[v.mode], checked: (stack.panes[i]?.mode ?? 'chart') === v.mode, radio: true, data: { vm: v.mode }, run: () => setMode(i, v.mode) }));

  let menuAt: { x: number; y: number } | null = null;
  function open(i: number, how: { anchor?: HTMLElement; at?: { x: number; y: number } }) {
    stack.sel = i;
    views = null;
    menu = { i, ...how };
    // the axes dialog opens where the menu was
    const r = how.anchor?.getBoundingClientRect();
    const [x, y] = r ? [r.left, r.bottom + 4] : [how.at!.x, how.at!.y];
    menuAt = { x: Math.max(8, Math.min(x, window.innerWidth - 400)), y: Math.max(8, Math.min(y, window.innerHeight - 460)) };
  }
  function outside(e: PointerEvent) {
    if (axes && !(e.target as HTMLElement).closest('.axdlg, .panemenu')) axes = null;
  }
  function key(e: KeyboardEvent) {
    if (axes && e.key === 'Escape') {
      axes = null;
      e.stopPropagation();
    }
  }
  function context(e: MouseEvent, i: number) {
    // a chip's right-click is the channel's menu (app.ts); elsewhere on the title, the pane's
    if ((e.target as HTMLElement).closest('[data-ch]')) return;
    e.preventDefault();
    if (lk) return;
    open(i, { at: { x: e.clientX, y: e.clientY } });
  }
</script>

{#snippet heightGrip(i: number, now: number, label: string, tip: string)}
  <div
    class="hgrip"
    role="slider"
    aria-orientation="vertical"
    aria-valuemin={60}
    aria-valuemax={800}
    aria-valuenow={now}
    aria-label={label}
    tabindex="0"
    aria-disabled={lk}
    data-tip={lk ? 'Unlock the view to resize panes' : tip}
    onpointerdown={(e) => gripDown(e, i)}
    onpointermove={gripMove}
    onpointerup={() => (grip = null)}
    ondblclick={() => !lk && setPlotH(i, undefined)}
    onkeydown={(e) => !lk && gripKey(e, i)}
  ></div>
{/snippet}

{#snippet tileGrid(p: (typeof stack.panes)[number])}
                <div class="tiles" data-tiles>
                  <div class="twhere" data-where-line>{ui.valuesAt}</div>
                  {#each p.channels as ch (ch)}
                    {@const lvl = stack.values[ch] != null ? stack.levels[ch] : stack.restingLevels[ch]}
                    <div class="tile {lvl ?? ''}" style="--c: {chCss(ch)}" data-tile={ch} data-level={lvl ?? ''} role="button" tabindex="0" data-tip="Warn and alarm limits" onclick={() => openChannelEditor(ch, 'limits')} onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), openChannelEditor(ch, 'limits'))}>
                      <span class="tname">{chName(ch)}</span>
                      <span class="tval">{stack.values[ch] ?? stack.resting[ch] ?? '–'}</span>
                      {#if chUnit(ch)}<span class="tunit">{chUnit(ch)}</span>{/if}
                      <span class="tlim">{limitText(chLimits(ch)) || 'Click to set limits'}</span>
                    </div>
                  {:else}
                    <span class="mnote">Tick channels on the left to show them here</span>
                  {/each}
                </div>
{/snippet}

<svelte:document onpointerdown={outside} ondragstart={dragStart} ondragend={() => (carrying = false)} />
<svelte:window onkeydowncapture={key} />

<div id="stack" class="stack" class:nopanes={!stack.panes.length} class:chanover={chanOver === -1} ondragover={(e) => overChans(e, -1)} ondragleave={() => (chanOver = chanSide = null)} ondrop={(e) => dropChans(e, -1)} role="presentation" style="--ph: {rows().length <= 3 ? 150 : rows().length <= 5 ? 125 : 105}px">
  {#each rows() as row, r (r)}
    {#if carrying}
      <div class="dropgap" class:over={gapOver === r} data-dropgap={r} role="presentation" ondragover={(e) => overGap(e, r)} ondragleave={() => (gapOver = null)} ondrop={(e) => dropGap(e, row[0])}>Drop here for a new pane</div>
    {/if}
    <div class="prow">
      {#each columns(row) as col, c (c)}
        <div class="pcol">
          {#each col as i (i)}
            {@const p = stack.panes[i]}
            <section class="pane" class:tablemode={p.mode === 'table'} class:sel={i === stack.sel} class:drop={dropAt === i} class:chanover={chanOver === i} data-side={chanOver === i ? (chanSide ?? undefined) : undefined} class:zone={zoneAt?.i === i} data-zone={zoneAt?.i === i ? zoneAt.zone : undefined} data-pane={i} style={p.h ? `--ph: ${p.h}px` : undefined} ondragover={(e) => paneOver(e, i)} ondragleave={() => (zoneAt?.i === i && (zoneAt = null), chanOver === i && (chanSide = null))} ondrop={(e) => paneDrop(e, i)} role="presentation">
              <div
                class="phead"
                use:fitChips={i}
                role="group"
                aria-label="Pane {i + 1}"
                oncontextmenu={(e) => context(e, i)}
              >
                <span
                  class="pnum"
                  role="button"
                  tabindex="-1"
                  draggable={lk ? undefined : 'true'}
                  data-tip={lk ? 'Unlock the view to move panes' : `Drag to move this pane; hold ${COPY_KEY} to copy it`}
                  ondragstart={(e) => {
                    dragging = i;
                    e.dataTransfer?.setData('text/plain', `pane ${i + 1}`);
                  }}
                  ondragend={() => (dragging = dropAt = zoneAt = null)}>{i + 1}</span
                >
                {#each p.channels as ch (ch)}
                  {@const info = ui.channels[ch]}
                  <span
                    class="chip"
                    data-ch={ch}
                    data-tip={lk ? 'Click for its display settings. Unlock the view to move it' : `Click for its display settings. Drag to another pane to move it; hold ${COPY_KEY} to copy`}
                    style="--c: {chCss(ch)}"
                    draggable={lk ? undefined : 'true'}
                    role="presentation"
                    onclick={(e) => !(e.target as HTMLElement).closest('.cx') && openChannelEditor(ch)}
                    ondragstart={(e) => {
                      e.dataTransfer?.setData('application/x-tlm-chip', JSON.stringify({ ch, from: i }));
                      e.dataTransfer?.setData('text/plain', info?.name ?? String(ch));
                    }}
                    ondragover={(e) => isChip(e) && (e.preventDefault(), e.stopPropagation())}
                    ondrop={(e) => dropChip(e, i, ch)}
                    >{chName(ch)}{#if failing.has(info?.name ?? '')}<span class="chkdot" class:warn={failing.get(info?.name ?? '') === 'warn'} data-chkdot data-tip="Used by a failing check"></span>{/if}{#if chUnit(ch)}<span class="unit">{chUnit(ch)}</span>{/if}{#if units(i).length > 1}<span class="side" data-side>· {axisKey(i, ch) === 'u1' ? 'R' : 'L'}</span>{/if}<span class="val" data-val={ch}
                      >{stack.values[ch] ?? ''}</span
                    ><button class="cx" data-chip-x={ch} disabled={lk} aria-label="Take {info?.name ?? ch} out of pane {i + 1}" data-tip={lk ? LOCKED : 'Take out of this pane'} onclick={(e) => (e.stopPropagation(), removeChannel(ch, i))}>{@html ICON.close}</button></span
                  >
                {:else}
                  <span class="mnote" data-empty-pane>Empty pane. Drag a channel here, or select this pane and click a channel.</span>
                {/each}
                {#if folded[i]}
                  <button class="more" data-more-chips={i} aria-haspopup="menu" data-tip="Every channel in this pane, with its value" onclick={(e) => (chanList = chanList?.i === i ? null : { i, anchor: e.currentTarget as HTMLElement })}>+{folded[i]}</button>
                {/if}
                <span class="spacer"></span>
                {#if stack.coarse[i]}
                  <span class="pill" data-coarse data-tip="Drawn from summaries; the exact view follows">coarse</span>
                {/if}
                {#if p.mode === 'xy' && p.channels.length > 1}
                  <!-- the prototype's XY controls, in the header beside the view button (sheet B5) -->
                  <select class="xysel" data-xsel aria-label="X axis channel" data-tip="Channel on the horizontal axis" onchange={(e) => setXy(i, { x: Number(e.currentTarget.value), xyView: undefined })}>
                    {#each p.channels as c (c)}<option value={c} selected={c === xyAxisOf(i)}>X: {chName(c)}</option>{/each}
                  </select>
                  <button class="tbtn xyb" data-xyeq aria-pressed={xyEqualOf(i)} data-tip="Equal scale: one unit is the same length on both axes (longitude is corrected for latitude), so shapes aren't stretched" onclick={() => setXy(i, { xyEqual: !xyEqualOf(i), xyView: undefined })}>1:1</button>
                  <button class="tbtn xyb" data-xyfit disabled={!p.xyView} data-tip="Fit: back to automatic ranges (or double-click the plot). Scroll zooms at the pointer; drag with Box zoom or Pan as the toolbar's mouse mode says." onclick={() => setXy(i, { xyView: undefined })}>Fit</button>
                {/if}
                <button
                  class="pmore vmenu"
                  data-vmenu
                  aria-haspopup="menu"
                  aria-label="View of pane {i + 1}: {viewName(i)}"
                  disabled={lk}
                  data-tip={lk ? `View: ${viewName(i).toLowerCase()}. Unlock the view to change it` : `View: ${viewName(i).toLowerCase()}. Click to change`}
                  aria-expanded={views?.i === i}
                  onclick={(e) => {
                    menu = null;
                    views = views?.i === i ? null : { i, anchor: e.currentTarget as HTMLElement };
                  }}>{@html VIEW_ICON[p.mode ?? 'chart']}<span class="vchev">{@html ICON.chev}</span></button
                >
                <!-- the prototype's pane actions, in its order, on hover or focus; those that don't fit give way, rarer first
                     (each is also in ⋯; see fitChips) (owner decision 9) -->
                {#if hasChart(i)}
                  <button class="pmore pact-axis" data-paxis aria-label="Axis and scale of pane {i + 1}" data-tip="Pane settings: axis range, label, decimals, log scale" onclick={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); axes = { i, x: Math.max(8, Math.min(r.left, window.innerWidth - 400)), y: Math.min(r.bottom + 4, window.innerHeight - 460) }; }}>{@html ICON.settings}</button>
                {/if}
                {#if p.channels.length && (!p.mode || p.mode === 'chart' || p.mode === 'both' || p.mode === 'cv' || p.mode === 'table')}
                  <button class="pmore pact-ev" data-pevents aria-pressed={paneEvents(i)} aria-label="Event markers in pane {i + 1}" data-tip={paneEvents(i) ? 'Hide event markers in this pane' : 'Show event markers in this pane'} onclick={() => toggleEvents(i)}>{@html ICON.events}</button>
                {/if}
                {#if p.channels.length}
                  <button class="pmore pact-dup" data-pdup disabled={lk} aria-label="Duplicate pane {i + 1}" data-tip={lk ? LOCKED : "Duplicate this pane below, with the same channels, view, sort and filters. Shift-click puts the copy beside it."} onclick={(e) => duplicate(i, e.shiftKey && canSplitRight(i))}>{@html ICON.dup}</button>
                {/if}
                <button class="pmore pact-split" data-psplit disabled={lk || !canSplitRight(i)} aria-label="Split pane {i + 1}" data-tip={lk ? LOCKED : canSplitRight(i) ? 'Split: add an empty pane on the right. To pick a side, drag a channel onto the left or right edge of a pane.' : 'A row holds up to 4 panes'} onclick={() => splitRight(i)}>{@html ICON.split}</button>
                <button
                  class="pmore"
                  data-pmenu
                  aria-haspopup="menu"
                  aria-label="More for pane {i + 1}"
                  disabled={lk}
                  data-tip={lk ? LOCKED : 'Close, duplicate or split (also on right-click)'}
                  aria-expanded={menu?.i === i}
                  onclick={(e) => (menu?.i === i ? (menu = null) : open(i, { anchor: e.currentTarget as HTMLElement }))}>{@html ICON.more}</button
                >
                <button class="pmore pact-close" data-pclose disabled={lk} aria-label="Close pane {i + 1}" data-tip={lk ? LOCKED : 'Close this pane'} onclick={() => closePane(i)}>{@html ICON.close}</button>
              </div>
              {#if p.mode === 'values'}
                {@render tileGrid(p)}
              {:else if p.mode === 'table'}
                <Table {i} alone height={p.tableH ?? null} />
              {:else if p.mode === 'spectrum'}
                <Spectrum {i} />
              {:else if p.mode === 'checks'}
                <ChecksPane {i} />
              {:else if p.mode === 'gauges'}
                <Gauges {i} />
              {:else if p.mode === 'xy'}
                <XY {i} />
                {@render heightGrip(i, p.h ?? 340, 'Plot height', 'Drag to resize the plot (double-click resets, ↑ ↓ step)')}
              {:else}
                {#if ui.filtered && ui.filtered.pane !== i}
                  <div class="fchip" data-fchip>
                    Filtered · {ui.filtered.rows.toLocaleString()} rows hidden
                    <button aria-label="Show all rows again" data-tip="Show every row again" onclick={() => setTable(ui.filtered!.pane, { tfilt: { ...stack.panes[ui.filtered!.pane]?.tfilt, apply: undefined } })}>{@html ICON.close}</button>
                  </div>
                {/if}
                <div class="plot" style="--pb: {labelled(i) ? PAD.timeLabels : 4}px; --pr: {rightPad()}px"></div>
                <!-- the chart's height grip: under the plot, except Chart and values, where it sits under the tiles at the pane's
                     bottom (sheet B5); Chart and table also has the table's grip at its bottom, as the prototype -->
                {#if p.mode !== 'cv'}{@render heightGrip(i, p.h ?? 150, 'Chart height', 'Drag to resize the chart (double-click resets, ↑ ↓ step)')}{/if}
                {#if p.mode === 'both'}<Table {i} height={p.tableH ?? 210} />{/if}
                {#if p.mode === 'cv'}{@render tileGrid(p)}{@render heightGrip(i, p.h ?? 150, 'Chart height', 'Drag to resize the chart above the values (double-click resets, ↑ ↓ step)')}{/if}
              {/if}
            </section>
          {/each}
        </div>
      {/each}
    </div>
    {#if r < rows().length - 1}
      <div class="rgap"><button data-gap={r} disabled={lk} aria-label="Add a pane here" data-tip={lk ? LOCKED : 'Add a pane between these rows'} onclick={() => addPaneBelow(row[row.length - 1])}>{@html ICON.plus}</button></div>
    {/if}
  {/each}
  {#if !stack.panes.length && ui.file}
    <div class="nopanes" data-nopanes>
      <p class="np-title">No channels plotted</p>
      <p class="muted">Drag a channel here, click a channel in Explorer, or use its Chart and Table buttons.</p>
      <button class="addpane big" id="addFirst" onclick={() => addPaneBelow(0)}>{@html ICON.plus}Add pane</button>
    </div>
  {/if}
  <!-- in the stack, as the prototype's: it scrolls with the panes, and a channel dropped on it makes a new pane -->
  <AddPane />
</div>

{#if views}
  <Menu
    class="viewmenu"
    label="View of pane {(views?.i ?? 0) + 1}"
    anchor={views?.anchor}
    onclose={() => (views = null)}
    items={views ? viewItems(views.i) : []}
  />
{/if}

{#if axes && stack.panes[axes.i]}
  <AxisDialog i={axes.i} x={axes.x} y={axes.y} onclose={() => (axes = null)} />
{/if}

{#if chanList && stack.panes[chanList.i]}
  <Menu class="chanlist" label="Channels in pane {(chanList?.i ?? 0) + 1}" anchor={chanList?.anchor} onclose={() => (chanList = null)}>
    {#each chanList ? (stack.panes[chanList.i]?.channels ?? []) : [] as ch (ch)}
      <div class="mi" role="menuitem" tabindex="-1" style="--c: {chCss(ch)}"><span class="sw8" style="background: var(--c)"></span><span class="nm">{chName(ch)}</span>{#if chUnit(ch)}<span class="hint">{chUnit(ch)}</span>{/if}<span class="hint num">{stack.values[ch] ?? stack.resting[ch] ?? ''}</span></div>
    {/each}
  </Menu>
{/if}

{#if menu}
  <Menu class="panemenu" label="Pane {(menu?.i ?? 0) + 1}" anchor={menu?.anchor} at={menu?.at} onclose={() => (menu = null)} items={menu ? items(menu.i) : []} />
{/if}

<style>
  .chkdot {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-left: 5px;
    border-radius: 50%;
    background: var(--danger);
  }
  .chkdot.warn {
    background: var(--warn);
  }
  .stack.nopanes {
    flex: 1;
  }
  .prow {
    display: flex;
    flex: none;
    gap: 8px;
  }
  .pcol {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  /* a table pane: the prototype's height, the chart's and 96 px, 240 px at least */
  .pane.tablemode :global(.ptable) {
    flex: none;
    height: max(240px, calc(var(--ph) + 96px));
  }
  .rgap {
    position: relative;
    z-index: 2;
    flex: none;
    width: 56px;
    height: 6px;
    margin: -3px 0;
  }
  .rgap button {
    position: absolute;
    left: 18px;
    top: -8px;
    display: none;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--accent-line);
    border-radius: 50%;
    background: var(--surface);
    color: var(--accent);
  }
  .rgap button :global(svg) {
    width: 12px;
    height: 12px;
  }
  .rgap:hover button,
  .rgap button:focus-visible {
    display: inline-grid;
  }
  .nopanes {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin: 40px 0;
  }
  .dropgap {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: center;
    height: 34px;
    margin: 0 0 6px;
    border: 1.5px dashed var(--border);
    border-radius: 8px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .dropgap.over {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--fg);
  }
  .addpane.big {
    margin: 10px 0 0;
    border-style: solid;
  }
  .np-title {
    margin: 0;
    font-size: 15px;
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
  .pane {
    position: relative;
  }
  .fchip {
    position: absolute;
    top: 6px;
    right: 8px;
    z-index: 4;
    display: flex;
    gap: 6px;
    align-items: center;
    padding: 2px 4px 2px 8px;
    font-size: 11.5px;
    color: var(--fg);
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 12px;
  }
  .fchip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    color: var(--fg-muted);
    cursor: pointer;
    background: transparent;
    border: 0;
    border-radius: 50%;
  }
  .fchip button:hover {
    color: var(--fg);
    background: var(--grid);
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
  /* a channel over a pane's edge: "Place on the left / right" in a box on that side, as the prototype's */
  .pane[data-side]::after {
    content: 'Place on the left';
    position: absolute;
    z-index: 5;
    top: 4px;
    bottom: 4px;
    left: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22%;
    border: 1.5px dashed var(--accent);
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent) 16%, var(--plot-bg));
    color: var(--fg);
    font-size: 12px;
    font-weight: 500;
    text-align: center;
    pointer-events: none;
  }
  .pane[data-side='right']::after {
    content: 'Place on the right';
    right: 4px;
    left: auto;
  }
  .pane.chanover[data-side] {
    background: none;
    outline: none;
  }
  .pane.zone {
    background: color-mix(in srgb, var(--accent-soft) 40%, transparent);
  }
  .pane[data-zone='top'] {
    box-shadow: inset 0 3px 0 var(--accent);
  }
  .pane[data-zone='bottom'] {
    box-shadow: inset 0 -3px 0 var(--accent);
  }
  .pane[data-zone='left'] {
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .pane[data-zone='right'] {
    box-shadow: inset -3px 0 0 var(--accent);
  }
  .pane[data-zone='middle'] {
    outline: 2px dashed var(--accent-line);
    outline-offset: -2px;
  }
  .pnum {
    cursor: grab;
  }
  .pane.sel .pnum,
  .pnum:hover {
    color: var(--accent);
    font-weight: 600;
  }
  .pnum:hover::before {
    content: '⋮';
    margin-right: 1px;
  }
  .pmore {
    display: inline-grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    gap: 2px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    visibility: hidden;
  }
  .pmore :global(svg) {
    width: 12px;
    height: 12px;
  }
  .pmore:hover {
    background: var(--raised);
    color: var(--fg);
  }
  /* the view button, as the prototype's .vmenu: the view's icon and a ▾ in a raised box */
  .vmenu {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 0 4px 0 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    color: var(--fg);
  }
  .vmenu :global(svg) {
    width: 15px;
    height: 15px;
  }
  .vmenu:hover {
    border-color: var(--accent-line);
    background: var(--raised);
  }
  .phead:hover .pmore,
  .pane.sel .pmore,
  .pmore:focus-visible {
    visibility: visible;
  }
  /* XY's controls in the header, as the prototype's .xysel and .xyb: shown with the other header tools */
  .xysel {
    min-height: 24px;
    max-width: 140px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--raised);
    color: var(--fg);
    font: 11.5px var(--font-ui);
  }
  .xyb {
    min-height: 24px;
    padding: 0 7px;
  }
  .xyb[aria-pressed='true'] {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .xyb:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .xysel,
  .xyb {
    visibility: hidden;
  }
  .phead :global(.pmore[hidden]) {
    display: none;
  }
  .pmore[data-pevents][aria-pressed='true'] {
    color: var(--accent);
  }
  .phead:hover :is(.xysel, .xyb),
  .pane.sel :is(.xysel, .xyb),
  .phead :is(.xysel, .xyb):focus-visible {
    visibility: visible;
  }
  .twhere {
    flex: 1 0 100%;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .vchev {
    display: inline-flex;
    color: var(--fg-muted);
  }
  .vmenu .vchev :global(svg) {
    width: 10px;
    height: 10px;
  }
  :global(.viewmenu .mi) {
    min-height: 40px;
  }
  :global(.viewmenu .mi .nm) {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
    white-space: normal;
  }
  :global(.viewmenu .mi[aria-checked='true']) {
    background: var(--accent-soft);
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
  .tile {
    cursor: pointer;
  }
  .tile.ok {
    border-color: color-mix(in srgb, var(--good) 45%, var(--grid));
    background: color-mix(in srgb, var(--good) 8%, transparent);
  }
  .tile.warn {
    border-color: color-mix(in srgb, var(--warn) 60%, var(--grid));
    background: color-mix(in srgb, var(--warn) 12%, transparent);
  }
  .tile.alarm {
    border-color: color-mix(in srgb, var(--danger) 60%, var(--grid));
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .tile.ok,
  .tile.warn,
  .tile.alarm {
    border-left-color: var(--c);
  }
  .tlim {
    font-size: 11px;
    color: var(--fg-muted);
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
  :global(.viewmenu small) {
    font-size: 11px;
    color: var(--fg-muted);
  }
</style>
