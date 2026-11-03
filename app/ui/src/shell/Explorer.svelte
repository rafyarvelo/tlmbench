<!-- Explorer: the folder's recordings and the open file's channels in one tree (folder → file → channel). Rows
     select first: a click selects a row and shows its actions beside the name; double-click or Enter opens a file,
     and a channel's checkbox plots it in the selected pane. The title row opens a folder or a recording and expands
     or collapses everything; the filter matches files and channels (a regex with .*); the open-files strip closes
     them. The folder is only read. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { setSetting, settings } from '../settings.svelte';
  import { stack, toggleChannel } from '../stack.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, type FolderEntry, ui } from '../ui.svelte';

  let q = $state('');
  let regex = $state(false);
  let sel = $state<string | null>(null);

  const matcher = $derived.by(() => {
    const t = q.trim();
    if (!t) return { ok: true, test: (_: string) => true };
    if (regex) {
      try {
        const r = new RegExp(t, 'i');
        return { ok: true, test: (s: string) => r.test(s) };
      } catch {
        return { ok: false, test: (_: string) => true };
      }
    }
    const low = t.toLowerCase();
    return { ok: true, test: (s: string) => s.toLowerCase().includes(low) };
  });

  const isOpen = (path: string) => path === ui.path;
  const inTab = (path: string) => tabs.list.some((t) => t.path === path);
  /** The open files, each with the tab a click brings forward (its file tab, else a layout shown on it). */
  const openFiles = $derived(
    [...new Set(tabs.list.map((t) => t.path))].map((path) => {
      const on = tabs.list.filter((t) => t.path === path);
      return { path, name: path.split(/[\\/]/).pop() ?? path, tab: on.find((t) => !t.ws) ?? on[0], front: on.some((t) => t.key === tabs.active) };
    }),
  );
  /** A file shows when its name, or (the open one) one of its channels, matches. */
  const chanMatch = (ch: { name: string }) => matcher.test(ch.name);
  const fileShown = (f: FolderEntry) => matcher.test(f.name) || (isOpen(f.path) && ui.channels.some(chanMatch));
  const dirShown = (d: FolderEntry): boolean => d.children.some((c) => (c.dir ? dirShown(c) : fileShown(c)));
  const expanded = (path: string) => !!q.trim() || settings.exExpanded.includes(path);
  function toggle(path: string) {
    const set = new Set(settings.exExpanded);
    if (set.has(path)) set.delete(path);
    else set.add(path);
    setSetting('exExpanded', [...set]);
  }
  function allDirs(d: FolderEntry | null, out: string[] = []): string[] {
    if (!d) return out;
    for (const c of d.children) {
      if (c.dir) {
        out.push(c.path);
        allDirs(c, out);
      } else if (isOpen(c.path)) out.push(c.path);
    }
    return out;
  }
  const inSel = (ch: number) => stack.panes[stack.sel]?.channels.includes(ch) ?? false;
  const size = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

  function key(e: KeyboardEvent, row: { kind: 'file'; path: string } | { kind: 'chan'; ch: number }) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    if (row.kind === 'file') actions.openPath(row.path);
    else toggleChannel(row.ch);
  }
</script>

<div class="explorer" id="explorer">
  <div class="phd">
    <span>Explorer</span><span class="spacer"></span>
    <button class="ibtn" id="exFolder" aria-label="Open folder" data-tip="Open a folder of recordings (only read)" onclick={actions.openFolder}>{@html ICON.folder}</button>
    <button class="ibtn" id="exOpen" aria-label="Open recording" data-tip="Open a CSV or .tlm file (⌘O)" onclick={actions.open}>{@html ICON.open}</button>
    <button class="ibtn" id="treeExpand" aria-label="Expand all" data-tip="Expand all" onclick={() => setSetting('exExpanded', allDirs(ui.folder))}>{@html ICON.expand}</button>
    <button class="ibtn" id="treeCollapse" aria-label="Collapse all" data-tip="Collapse all" onclick={() => setSetting('exExpanded', [])}>{@html ICON.collapse}</button>
  </div>
  <label class="exfilter">
    <input type="search" placeholder="Filter files and channels" aria-label="Filter files and channels" aria-invalid={!matcher.ok} bind:value={q} />
    <button class="rx" aria-pressed={regex} aria-label="Regular expression" data-tip="Regular expression" onclick={() => (regex = !regex)}>.*</button>
  </label>

  {#if tabs.list.length}
    <div class="openstrip">
      <span class="cap">Open · {openFiles.length}</span><span class="spacer"></span>
      <button class="linkbtn" id="closeAll" onclick={actions.closeFile}>Close all</button>
    </div>
    <div class="openfiles" id="openFiles">
      {#each openFiles as f (f.path)}
        <span class="ofile" class:on={f.front} data-tip={f.path}>
          <button class="ofnm" onclick={() => actions.showTab(f.tab.key)}>{f.name}</button><button
            class="ofx"
            aria-label="Close {f.name}"
            onclick={() => actions.closePath(f.path)}>{@html ICON.close}</button
          >
        </span>
      {/each}
    </div>
  {/if}

  <div class="tree" role="tree" aria-label="Recordings">
    {#if ui.folder}
      {#snippet dir(d: FolderEntry, depth: number)}
        {#each d.children.filter((c) => (c.dir ? dirShown(c) : fileShown(c))) as c (c.path)}
          {#if c.dir}
            <div class="row dir" role="treeitem" aria-selected="false" aria-expanded={expanded(c.path)} style="--d: {depth}" tabindex="0" data-path={c.path}
              onclick={() => toggle(c.path)} onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), toggle(c.path))}>
              <span class="chev" class:on={expanded(c.path)}>›</span>{@html ICON.folder}<span class="nm">{c.name}</span>
            </div>
            {#if expanded(c.path)}{@render dir(c, depth + 1)}{/if}
          {:else}
            {@render file(c.path, c.name, c.bytes, depth)}
          {/if}
        {/each}
      {/snippet}
      <div class="row dir root" style="--d: 0" data-path={ui.folder.path}>{@html ICON.folder}<span class="nm">{ui.folder.name}</span></div>
      {@render dir(ui.folder, 1)}
    {:else if ui.file}
      {@render file(ui.path, ui.file, null, 0)}
    {:else}
      <p class="exnone">No recording open. <button class="linkbtn" onclick={actions.openFolder}>Open a folder</button> or a <button class="linkbtn" onclick={actions.open}>recording</button>, or drop a CSV or .tlm file on the window.</p>
    {/if}
  </div>
</div>

{#snippet file(path: string, name: string, bytes: number | null, depth: number)}
  {@const open = isOpen(path)}
  <div
    class="row file"
    class:open
    class:sel={sel === path}
    role="treeitem"
    aria-selected={sel === path}
    aria-expanded={open ? !q.trim() || true : undefined}
    tabindex="0"
    style="--d: {depth}"
    data-path={path}
    data-tip={path}
    onclick={() => (sel = path)}
    ondblclick={() => actions.openPath(path)}
    onkeydown={(e) => key(e, { kind: 'file', path })}
  >
    {@html ICON.file}<span class="nm">{name}</span>{#if inTab(path) && !open}<span class="dot" aria-label="open in a tab"></span>{/if}
    {#if sel === path && !open}
      <button class="tbtn act" onclick={(e) => (e.stopPropagation(), actions.openPath(path))}>Open</button>
    {:else if bytes != null}
      <span class="meta num">{size(bytes)}</span>
    {/if}
  </div>
  {#if open}
    <div id="chans" class="chans" role="group">
      {#each ui.channels as c, ch (ch)}
        {#if chanMatch(c) || matcher.test(name)}
          <div
            class="row chan"
            class:on={inSel(ch)}
            class:sel={sel === `ch:${ch}`}
            role="treeitem"
            aria-selected={sel === `ch:${ch}`}
            tabindex="0"
            style="--d: {depth + 1}; --c: var(--trace-{(ch % 8) + 1})"
            data-ch={ch}
            onclick={() => (sel = `ch:${ch}`)}
            ondblclick={() => toggleChannel(ch)}
            onkeydown={(e) => key(e, { kind: 'chan', ch })}
          >
            <button class="box" role="checkbox" aria-checked={inSel(ch)} aria-label="Plot {c.name} in pane {stack.sel + 1}" onclick={(e) => (e.stopPropagation(), toggleChannel(ch))}></button>
            <span class="nm">{c.name}</span>{#if c.unit}<span class="unit">{c.unit}</span>{/if}
          </div>
        {/if}
      {/each}
    </div>
  {/if}
{/snippet}

<style>
  .explorer {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .exfilter {
    display: flex;
    align-items: center;
    margin: 0 10px 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
  }
  .exfilter:focus-within {
    border-color: var(--accent-line);
  }
  .exfilter input {
    flex: 1;
    min-width: 0;
    min-height: 28px;
    padding: 0 8px;
    border: 0;
    outline: none;
    background: transparent;
  }
  .exfilter input[aria-invalid='true'] {
    color: var(--danger, #ff6b78);
  }
  .rx {
    margin-right: 3px;
    padding: 0 6px;
    min-height: 22px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font: 12px var(--font-data);
  }
  .rx[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .openstrip {
    display: flex;
    align-items: center;
    margin: 0 14px 4px;
    font-size: 11.5px;
  }
  .cap {
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .openstrip .linkbtn {
    color: var(--accent);
    font-size: 11.5px;
  }
  .openfiles {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 0 12px 8px;
  }
  .ofile {
    display: inline-flex;
    align-items: center;
    max-width: 100%;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    font-size: 12px;
  }
  .ofile.on {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .ofnm {
    min-width: 0;
    overflow: hidden;
    padding: 2px 4px 2px 8px;
    border: 0;
    background: transparent;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ofx {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--fg-muted);
  }
  .ofx :global(svg) {
    width: 10px;
    height: 10px;
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .tree {
    flex: 1;
    overflow: auto;
    padding: 0 6px 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 8px 0 calc(8px + var(--d) * 14px);
    border-radius: 8px;
    font-size: 12px;
    cursor: default;
    outline: none;
  }
  .row:hover {
    background: var(--raised);
  }
  .row.sel,
  .row:focus-visible {
    background: var(--accent-soft);
  }
  .row :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .row .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.file.open .nm {
    color: var(--accent);
    font-weight: 600;
  }
  .row.root {
    font-weight: 600;
  }
  .chev {
    display: inline-block;
    width: 10px;
    color: var(--fg-muted);
    transition: transform 0.12s;
  }
  .chev.on {
    transform: rotate(90deg);
  }
  .meta,
  .unit {
    margin-left: auto;
    color: var(--fg-muted);
    font: 11px var(--font-data);
  }
  .act {
    margin-left: auto;
    min-height: 22px;
  }
  .box {
    flex: none;
    width: 14px;
    height: 14px;
    padding: 0;
    border: 1.5px solid var(--fg-muted);
    border-radius: 4px;
    background: transparent;
  }
  .chan.on .box {
    border-color: var(--c);
    background: var(--c);
  }
  .exnone {
    margin: 4px 8px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .exnone .linkbtn {
    padding: 0;
    color: var(--accent);
    font-size: 12px;
  }
</style>
