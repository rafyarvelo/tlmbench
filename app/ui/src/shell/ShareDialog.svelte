<!-- Share…: every object you have, listed by kind with how it stands against the folder's .tlmbench/. New objects are
     shared by default, the folder's own are taken by default, and what differs waits for a choice. A folder that looks
     read-only offers Export… instead, which writes the same files anywhere you choose. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { cfg, choices, KIND_TITLE, KINDS, rowKey, share, type Pick, type ShareRow } from '../share.svelte';

  const groups = $derived(
    KINDS.map((k) => ({ kind: k, rows: share.rows.filter((r) => r.kind === k && (share.showSame || r.state !== 'same')) })).filter((g) => g.rows.length),
  );
  const same = $derived(share.rows.filter((r) => r.state === 'same').length);
  const nShare = $derived(share.rows.filter((r) => share.pick[rowKey(r)] === 'share').length);
  const nUse = $derived(share.rows.filter((r) => share.pick[rowKey(r)] === 'use').length);
  const stateText: Record<ShareRow['state'], string> = { new: 'Only yours', changed: 'Differs', same: 'Same', folder: 'Only in the folder' };

  function key(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    actions.shareClose();
  }
  const set = (r: ShareRow, v: Pick) => (share.pick[rowKey(r)] = v);
  function all(v: 'share' | 'use' | '') {
    for (const r of share.rows) if (choices(r, share.writable).some((c) => c.value === v)) share.pick[rowKey(r)] = v;
  }
</script>

<svelte:window onkeydowncapture={(e) => share.open && key(e)} />

{#if share.open}
  <div class="scrim" role="presentation" onclick={actions.shareClose}></div>
  <div class="dlg sharedlg" id="shareDlg" role="dialog" aria-label="Share">
    <header><b>{share.writable ? 'Share' : 'Export'} configuration</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.shareClose}>{@html ICON.close}</button></header>
    <div class="sbody">
      {#if share.writable}
        <p class="hint" id="shareHint">Writes what you pick to this folder's <b>.tlmbench/</b>, one small file per object. Nothing else in the folder is touched.</p>
      {:else}
        <p class="hint" id="shareRo">This folder looks read-only, so nothing can be shared to it. Export writes the same files to a folder you choose; put it next to the recordings, or point <b>--config</b> at it.</p>
      {/if}
      {#if share.error}<p class="bad" id="shareErr">{share.error}</p>{/if}
      {#each share.problems as p (p.path)}<p class="bad" data-problem>{p.path}: {p.why}</p>{/each}
      {#if share.busy && !share.rows.length}
        <p class="hint">Reading…</p>
      {:else if !groups.length}
        <p class="hint" id="shareNone">{share.rows.length ? 'Everything is the same as the folder’s.' : 'You have nothing to share yet. Recipes, rules, layouts, widgets and the events you add appear here.'}</p>
      {/if}
      <div class="slist" id="shareList">
        {#each groups as g (g.kind)}
          <section data-kind={g.kind}>
            <h4>{KIND_TITLE[g.kind]}</h4>
            {#each g.rows as r (rowKey(r))}
              {@const opts = choices(r, share.writable)}
              <div class="srow" data-share-row={rowKey(r)} data-state={r.state}>
                <span class="nm" data-tip={r.key}>{r.name}</span>
                <span class="st {r.state}">{stateText[r.state]}{r.detail ? ` · ${r.detail}` : ''}</span>
                {#if opts.length > 1}
                  <select class="sel" aria-label="What to do with {r.name}" value={share.pick[rowKey(r)] ?? ''} onchange={(e) => set(r, e.currentTarget.value as Pick)}>
                    {#each opts as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
                  </select>
                {:else}<span></span>{/if}
              </div>
            {/each}
          </section>
        {/each}
      </div>
      {#if same}
        <label class="chk"><input type="checkbox" id="shareSame" bind:checked={share.showSame} />Show the {same} that {same === 1 ? 'is' : 'are'} the same as the folder's</label>
      {/if}
      {#if share.git && share.writable}
        <label class="chk" data-tip="Adds the recording file patterns to the folder's .gitignore so only configuration is committed"><input type="checkbox" id="shareIgnore" bind:checked={share.ignore} />Keep recordings out of git (adds them to .gitignore)</label>
      {/if}
      {#if share.done}<p class="ok" id="shareDone">{share.done}</p>{/if}
      {#if cfg.dir}<p class="hint">Reading <b>{cfg.dir}</b>. Yours win a name; the folder's fill in the rest.</p>{/if}
    </div>
    <footer>
      <button class="linkbtn" data-all="share" disabled={!share.writable} onclick={() => all('share')}>Share all</button>
      <button class="linkbtn" data-all="" onclick={() => all('')}>Clear</button>
      <span class="spacer"></span>
      {#if !share.writable}<button class="tbtn" id="shareExport" disabled={share.busy} onclick={actions.shareExport}>Export…</button>{/if}
      <button class="tbtn" onclick={actions.shareClose}>Close</button>
      <button class="tbtn primary" id="shareGo" disabled={share.busy || (!nShare && !nUse)} onclick={actions.shareApply}>
        {nShare && nUse ? `Share ${nShare}, use ${nUse}` : nShare ? `Share ${nShare}` : nUse ? `Use ${nUse}` : 'Apply'}
      </button>
    </footer>
  </div>
{/if}

<style>
  .sharedlg {
    width: min(680px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
  }
  .dclose {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
  }
  .dclose:hover {
    border-color: var(--border);
    background: var(--raised);
    color: var(--fg);
  }
  .dclose :global(svg) {
    width: 10px;
    height: 10px;
  }
  .sbody {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    padding: 10px 14px;
    overflow-y: auto;
  }
  .sbody p {
    margin: 0;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .bad {
    color: var(--danger);
    font-size: 12px;
  }
  .ok {
    color: var(--good);
    font-size: 12px;
  }
  .chk {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
  }
  .slist {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h4 {
    margin: 0 0 4px;
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .srow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 150px;
    gap: 10px;
    align-items: center;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 12px;
  }
  .srow:hover {
    background: var(--raised);
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st {
    color: var(--fg-muted);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .st.new {
    color: var(--accent);
  }
  .st.changed {
    color: var(--warn);
  }
</style>
