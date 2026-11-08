<!-- One menu, as the prototype's: placed under its button (above when there's no room below) and growing out of it, or
     at the pointer for a right-click; the first item takes focus; arrow keys, Home and End move between items; Esc,
     a pick or a press outside closes it. Give it `items`, or children for anything else (forms, rows with buttons). -->
<script lang="ts" module>
  export type MenuItem =
    | { sep: true }
    | { heading: string }
    | {
        label: string;
        run: () => void;
        icon?: string;
        /** a line under the label */
        note?: string;
        /** muted text at the end of the row */
        meta?: string;
        kbd?: string;
        /** a checkbox item (or with `radio`, one of a set): ticked when true */
        checked?: boolean;
        radio?: boolean;
        disabled?: boolean;
        /** data-* attributes, without the prefix */
        data?: Record<string, string>;
        tip?: string;
      };
</script>

<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte';
  import { ICON } from '../icons';

  let {
    items,
    children,
    anchor = null,
    at = null,
    label,
    first = false,
    class: cls = '',
    onclose,
  }: {
    items?: MenuItem[];
    children?: Snippet;
    /** the button that opened it: the menu sits under it, and pressing it again isn't a press outside */
    anchor?: HTMLElement | null;
    /** a point (right-click) when there's no button */
    at?: { x: number; y: number } | null;
    label?: string;
    /** the first item takes focus even when another is ticked */
    first?: boolean;
    class?: string;
    onclose: () => void;
  } = $props();

  let el = $state<HTMLDivElement>();
  let pos = $state({ left: -9999, top: -9999, origin: '0 0' });

  /** As the prototype's placePop: under the anchor, its left edge kept 16 px inside the window, above it when it
   * doesn't fit below; in the middle of the screen on a phone. At a point, it stays 8 px inside the window. */
  function place() {
    if (!el) return;
    const [w, h, W, H] = [el.offsetWidth, el.offsetHeight, window.innerWidth, window.innerHeight];
    if (W <= 640) {
      pos = { left: Math.max(8, (W - w) / 2), top: Math.max(8, (H - h) / 2), origin: '50% 50%' };
    } else if (anchor) {
      const r = anchor.getBoundingClientRect();
      const left = Math.max(16, Math.min(W - w - 16, r.left));
      const above = r.bottom + 4 + h > H - 16;
      const top = above ? Math.max(16, r.top - h - 4) : r.bottom + 4;
      pos = { left, top, origin: `${Math.max(0, Math.min(w, r.left + r.width / 2 - left))}px ${above ? h : 0}px` };
    } else if (at) {
      pos = { left: Math.max(8, Math.min(W - w - 8, at.x)), top: Math.max(8, Math.min(H - h - 8, at.y)), origin: '0 0' };
    }
  }

  const enabled = () => [...(el?.querySelectorAll<HTMLElement>('.mi:not(:disabled)') ?? [])];

  onMount(() => {
    place();
    // the checked item, else the first; not when something inside already has focus (a name field)
    void tick().then(() => !el?.contains(document.activeElement) && ((first ? null : el?.querySelector<HTMLElement>('.mi[aria-checked="true"]:not(:disabled)')) ?? enabled()[0] ?? el)?.focus({ preventScroll: true }));
  });

  function keydown(e: KeyboardEvent) {
    const list = enabled();
    if (!list.length || !['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
    if ((e.target as HTMLElement).closest('input, textarea, select')) return;
    e.preventDefault();
    const k = list.indexOf(document.activeElement as HTMLElement);
    const next = e.key === 'Home' ? 0 : e.key === 'End' ? list.length - 1 : e.key === 'ArrowDown' ? (k + 1) % list.length : (k - 1 + list.length) % list.length;
    list[next].focus();
  }

  function outside(e: PointerEvent) {
    const t = e.target as Node;
    if (el?.contains(t) || anchor?.contains(t)) return;
    onclose();
  }

  function escape(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    onclose();
    anchor?.focus({ preventScroll: true });
  }

  const dataAttrs = (d?: Record<string, string>) => Object.fromEntries(Object.entries(d ?? {}).map(([k, v]) => [`data-${k}`, v]));
</script>

<svelte:document onpointerdown={outside} />
<svelte:window onkeydowncapture={escape} onresize={place} />

<div
  class="menu {cls}"
  role="menu"
  tabindex="-1"
  aria-label={label}
  bind:this={el}
  style="left: {pos.left}px; top: {pos.top}px; transform-origin: {pos.origin}"
  onkeydown={keydown}
>
  {#if items}
    {#each items as it, k (k)}
      {#if 'sep' in it}
        <div class="msep"></div>
      {:else if 'heading' in it}
        <div class="mgrp">{it.heading}</div>
      {:else}
        <button
          class="mi"
          role={it.checked === undefined ? 'menuitem' : it.radio ? 'menuitemradio' : 'menuitemcheckbox'}
          aria-checked={it.checked}
          disabled={it.disabled}
          data-tip={it.tip}
          {...dataAttrs(it.data)}
          onclick={() => {
            // `it` follows `items`, which closing may change: take the action first
            const run = it.run;
            onclose();
            run();
          }}
          >{#if it.icon}{@html it.icon}{/if}<span class="nm">{it.label}{#if it.note}<small>{it.note}</small>{/if}</span>{#if it.meta}<span
              class="meta">{it.meta}</span
            >{/if}{#if it.kbd}<kbd>{it.kbd}</kbd>{/if}{#if it.checked}<span class="ck">{@html ICON.check}</span>{/if}</button
        >
      {/if}
    {/each}
  {/if}
  {@render children?.()}
</div>

<style>
  .menu:focus {
    outline: none;
  }
  .ck {
    display: inline-flex;
    flex: none;
    width: 13px;
    margin-left: 8px;
    color: var(--accent);
  }
  .ck :global(svg) {
    color: var(--accent);
  }
  .mi kbd {
    margin-left: 0;
  }
</style>
