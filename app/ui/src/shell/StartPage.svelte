<!-- The start page, a full page as the prototype's: three cards (Open folder, Open recording, Connect live source), then
     recent sessions and recordings beside live sources and layouts. It is what shows with no file open; the logo in the
     rail opens it over the tabs, and Esc or any open closes it. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { live, liveActions } from '../live.svelte';
  import { setSetting, settings } from '../settings.svelte';
  import { actions, ui } from '../ui.svelte';
  import { tabs } from '../tabs.svelte';
  import { duration } from '../axes';

  let { overlay = false }: { overlay?: boolean } = $props();
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const dir = (p: string) => p.slice(0, Math.max(0, p.length - name(p).length - 1));
  /** The folders' sessions with files open, the last used first. */
  const sessions = $derived(Object.entries(settings.sessions).filter(([, s]) => s.files.length).sort((a, b) => (b[1].at ?? 0) - (a[1].at ?? 0)).slice(0, 6));
  /** How long ago: "just now", "5 min ago", "3 h ago", "2 days ago". */
  function ago(at: number): string {
    const m = Math.round((Date.now() - at) / 60000);
    if (m < 1) return 'just now';
    if (m < 60) return `${m} min ago`;
    if (m < 60 * 24) return `${Math.round(m / 60)} h ago`;
    const d = Math.round(m / 1440);
    return `${d} day${d === 1 ? '' : 's'} ago`;
  }
  /** What sits under a recent recording: its folder, channels and length (known once it has been opened). */
  const about = (p: string) => {
    const i = settings.recentInfo[p];
    return i ? `${dir(p)} · ${i.channels} channel${i.channels === 1 ? '' : 's'} · ${duration(i.seconds * 1e9)}` : dir(p);
  };
  const kindName = (k: string) => (k === 'playback' ? 'File playback' : k === 'serial' ? 'Serial port' : k === 'file' ? 'Tail a file' : k.toUpperCase());
  /** Your layouts, then the ones shared in the folder; they open on the file in front. */
  const layouts = $derived([
    ...settings.workspaces.map((w) => ({ name: w.name, panes: w.panes.length, shared: '', by: '' })),
    ...ui.shared.flatMap((v) => (v.ws ? [{ name: v.ws.name, panes: v.ws.panes.length, shared: v.file, by: v.ws.by ?? '' }] : [])),
  ]);
  const go = (f: () => void) => () => {
    ui.start = false;
    f();
  };
</script>

<div class="start" class:overlay id="startPage">
  <div class="wrap">
    <header>
      <div class="hero">
        <h1>TelemetryBench</h1>
        <p class="muted">Open a recording or connect a live source. Nothing is written next to your data.</p>
      </div>
      {#if overlay}<button class="ibtn" aria-label="Close the start page" data-tip="Close (Esc)" onclick={() => (ui.start = false)}>{@html ICON.close}</button>{/if}
    </header>
    {#if live.cards.some((c) => c.recording)}
      {@const rec = live.cards.find((c) => c.recording)!}
      <p class="recbanner" id="startRec"><span class="rdot"></span>Recording to <span class="num">{rec.session}</span></p>
    {/if}
    <div class="cards">
      {#if !ui.browserOnly}
        <button class="card" data-start="folder" onclick={go(actions.openFolder)}>{@html ICON.folder}<span class="tx"><b>Open folder…</b><span>A folder of runs: CSV, Parquet, or binary logs with a descriptor</span></span></button>
      {/if}
      <button class="card" data-start="open" onclick={go(actions.open)}>{@html ICON.file}<span class="tx"><b>Open recording… <kbd>⌘O</kbd></b><span>One file, in its own tab with a starter layout</span></span></button>
      {#if !ui.browserOnly}
        <button class="card live" data-start="live" onclick={go(() => liveActions.openConnect(false))}>{@html ICON.live}<span class="tx"><b>Connect live source…</b><span>Serial, UDP, TCP, or tail a file being written. Try file playback.</span></span></button>
      {/if}
    </div>
    <div class="cols">
      <section>
        <h2>Recent sessions</h2>
        {#each sessions as [key, s] (key)}
          {@const current = key === settings.folder && tabs.list.length > 0}
          <button class="rec sess" data-session={key} data-tip={key || 'Files opened without a folder'} onclick={go(() => actions.openSession(key))}
            >{@html ICON.session}<span class="nm">{key ? name(key) : 'No folder'}</span><span class="dir muted"
              >{s.files.length} file{s.files.length === 1 ? '' : 's'} · {s.files.slice(0, 2).map(name).join(', ')}{s.files.length > 2 ? ` +${s.files.length - 2}` : ''}</span
            ><span class="when muted">{#if current}<span class="pill">current</span>{:else}{ago(s.at ?? 0)}{/if}</span></button
          >
        {:else}
          <p class="muted none">No sessions yet. Opening a folder or a recording starts one, and it saves as you work.</p>
        {/each}
        <p class="muted hint">A session is a folder plus your tabs, open files, time range and cursors. Each folder keeps its own.</p>
        <h2>Recent recordings</h2>
        {#each settings.recent as r (r)}
          <button class="rec" data-recent={r} data-tip={r} onclick={go(() => actions.openPath(r))}>{@html ICON.file}<span class="nm">{name(r)}</span><span class="dir muted">{about(r)}</span></button>
        {:else}
          <p class="muted none">None yet.</p>
        {/each}
      </section>
      <section>
        {#if !ui.browserOnly}
          <h2>Live sources<span class="spacer"></span><button class="linkbtn" data-start="newconn" onclick={go(() => liveActions.openConnect(false))}>New connection…</button></h2>
          {#each settings.savedConnections as c (c.id)}
            <div class="rec conn" data-conn={c.id}>
              <button class="connopen" data-tip="Open the Connect dialog with this connection" onclick={go(() => liveActions.openConnect(false, c))}>{@html ICON.live}<span class="nm">{c.name}</span><span class="dir muted">{kindName(c.kind)}</span></button>
              <button class="ibtn connx" aria-label="Remove {c.name}" data-tip="Remove from the start page" onclick={() => setSetting('savedConnections', settings.savedConnections.filter((x) => x.id !== c.id))}>{@html ICON.close}</button>
            </div>
          {:else}
            <p class="muted none">None saved. Tick "Save this connection" when you connect.</p>
          {/each}
        {/if}
        <h2>Layouts{settings.folder ? ` in ${name(settings.folder)}` : ''}</h2>
        {#each layouts as l (l.shared || l.name)}
          <button class="rec" data-layout={l.shared || l.name} disabled={!ui.file} data-tip={ui.file ? 'Open it on the file in front' : 'Open a recording first: a layout opens on the file in front'}
            onclick={go(() => (l.shared ? actions.openShared(l.shared) : actions.openWs(l.name)))}>{@html l.shared ? ICON.folder : ICON.layout}<span class="nm">{l.name}</span><span class="dir muted">{l.shared ? `shared${l.by ? ` by ${l.by}` : ''}` : 'yours'} · {l.panes} pane{l.panes === 1 ? '' : 's'}</span></button
          >
        {:else}
          <p class="muted none">No layouts yet. Save one from a tab's menu.</p>
        {/each}
      </section>
    </div>
    <footer>
      <label class="switch"
        ><input type="checkbox" role="switch" data-set="showStart" checked={settings.showStart} onchange={(e) => setSetting('showStart', e.currentTarget.checked)} /><span>Show this page at launch</span></label
      >
      <span class="spacer"></span><span class="muted">The logo at the top of the rail brings you back here.</span>
    </footer>
  </div>
</div>

<style>
  .start {
    display: block;
    width: 100%;
    height: 100%;
    overflow-y: auto;
    padding: 40px 24px 24px;
    pointer-events: auto;
  }
  .start.overlay {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--bg);
  }
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 22px;
    max-width: 1040px;
    margin: 0 auto;
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 600;
  }
  h2 {
    display: flex;
    align-items: center;
    margin: 18px 0 6px;
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  h2:first-child {
    margin-top: 0;
  }
  p {
    margin: 4px 0 14px;
  }
  .hero p {
    margin: 6px 0 0;
  }
  .none,
  .hint {
    margin: 4px 8px 6px;
    font-size: 12px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 12px;
  }
  .card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: var(--fg);
    font: inherit;
    text-align: left;
  }
  .card:hover {
    border-color: var(--accent-line, var(--accent));
    background: color-mix(in srgb, var(--accent) 6%, var(--surface));
  }
  .card > :global(svg) {
    flex: none;
    box-sizing: content-box;
    width: 18px;
    height: 18px;
    padding: 9px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .card.live > :global(svg) {
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--danger);
  }
  .card .tx {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .card b {
    font-size: 15px;
    font-weight: 600;
  }
  .card .tx span {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .card kbd {
    margin-left: 6px;
    color: var(--fg-muted);
    font: 11px var(--font-data);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    align-items: start;
    gap: 16px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
  .rec {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
  }
  .rec:hover:not(:disabled) {
    background: var(--raised);
  }
  .rec:disabled {
    opacity: 0.5;
  }
  .rec :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .rec .when {
    flex: none;
    margin-left: 8px;
    font-size: 11.5px;
  }
  .pill {
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 11px;
  }
  .rec .dir {
    min-width: 0;
    margin-left: auto;
    overflow: hidden;
    font-size: 11.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .recbanner {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0 0;
    padding: 6px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    font-size: 12px;
  }
  .recbanner .rdot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }
  .conn {
    display: flex;
    align-items: center;
    padding: 0;
  }
  .connopen {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 6px 8px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .connx {
    margin-right: 4px;
    opacity: 0;
  }
  .conn:hover .connx,
  .connx:focus-visible {
    opacity: 1;
  }
</style>
