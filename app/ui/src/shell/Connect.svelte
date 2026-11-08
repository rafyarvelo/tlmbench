<!-- Connect a live source: a serial port, TCP (the app connects to the device), UDP (it listens) or a growing file
     (followed from its start), with the frames' descriptor for the first three. It opens a live tab, recording into the Recordings folder. -->
<script lang="ts">
  import { live, liveActions } from '../live.svelte';
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';

  type Kind = 'serial' | 'udp' | 'tcp' | 'file';
  let kind = $state<Kind>('tcp');
  /** The kinds as the prototype lists them: an icon, the name and what it's for. */
  const KINDS: [Kind, string, string][] = [
    ['serial', 'Serial port', 'USB or RS-232 adapter'],
    ['udp', 'UDP', 'Datagrams on a port'],
    ['tcp', 'TCP', 'Connect to a server'],
    ['file', 'Tail a file', 'A file still being written'],
  ];
  const KIND_ICON: Record<Kind, string> = {
    serial: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M4 8h16l-2 8H6z"/><circle cx="9" cy="12" r=".8" fill="currentColor"/><circle cx="12" cy="12" r=".8" fill="currentColor"/><circle cx="15" cy="12" r=".8" fill="currentColor"/></svg>',
    udp: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M5 12.5a10 10 0 0114 0M8 15.5a5.5 5.5 0 018 0"/><circle cx="12" cy="19" r="1.2" fill="currentColor"/></svg>',
    tcp: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M8 12h8M10 8l-4 4 4 4M14 8l4 4-4 4"/></svg>',
    file: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M6 3h8l4 4v14H6z"/><path d="M9 13h6M9 17h4"/></svg>',
  };
  let port = $state('');
  let baud = $state('115200');
  let address = $state('127.0.0.1:9000');
  let path = $state('');
  let descriptor = $state('');
  const endpoint = $derived(
    kind === 'file' ? `file://${path}` : kind === 'serial' ? `serial://${port}?baud=${baud}` : `${kind}://${address}`,
  );
  const ready = $derived(kind === 'file' ? !!path.trim() : !!descriptor.trim() && !!(kind === 'serial' ? port : address).trim());

  async function choose(which: 'path' | 'descriptor') {
    const p = await actions.pickFile(which === 'descriptor' ? ['toml'] : ['csv', 'tsv', 'txt', 'tlm']);
    if (p) {
      if (which === 'path') path = p;
      else descriptor = p;
    }
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      live.connect = false;
    }
  }
</script>

<svelte:window onkeydowncapture={(e) => live.connect && key(e)} />

<div class="scrim" role="presentation" onclick={() => (live.connect = false)}></div>
<div class="dlg livedlg" id="liveDlg" role="dialog" aria-label="Connect a live source">
  <header><b>Connect a live source</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={() => (live.connect = false)}>{@html ICON.close}</button></header>
  <div class="lvdlg">
    <div class="lvkinds" role="group" aria-label="Source type">
      {#each KINDS as [k, name, sub] (k)}
        <button class="lvkind" data-kind={k} aria-pressed={kind === k} onclick={() => (kind = k)}>{@html KIND_ICON[k]}<b>{name}</b><small>{sub}</small></button>
      {/each}
    </div>
    <div class="lvform">
      {#if kind === 'file'}
        <label class="lvf"><span>File</span><div class="lvpick"><input class="sel" id="lvPath" bind:value={path} placeholder="/path/to/log.csv" spellcheck="false" autocomplete="off" /><button class="tbtn" onclick={() => choose('path')}>Choose…</button></div><small>Follows a file another program is still writing, like tail -f. Recorded from its start; the file is only read.</small></label>
      {:else if kind === 'serial'}
        <label class="lvf"><span>Port</span><input class="sel" id="lvPort" bind:value={port} placeholder="/dev/tty.usbserial-1 or COM3" spellcheck="false" autocomplete="off" /></label>
        <label class="lvf"><span>Baud rate</span><select class="sel" id="lvBaud" bind:value={baud}>
            {#each ['9600', '19200', '38400', '57600', '115200', '230400', '460800', '921600', '2000000'] as b (b)}<option value={b}>{b}</option>{/each}
          </select></label>
        {@render desc()}
      {:else}
        <label class="lvf"><span>{kind === 'tcp' ? 'Connect to' : 'Listen on'}</span><input class="sel" id="lvAddr" bind:value={address} placeholder="host:port" spellcheck="false" autocomplete="off" /></label>
        {@render desc()}
      {/if}
      {#if live.error}<p class="lverr" id="lvErr">{live.error}</p>{/if}
    </div>
  </div>
  <footer>
    <span class="muted">Opens in a new live tab, recorded to the Recordings folder</span><span class="spacer"></span>
    <button class="tbtn" onclick={() => (live.connect = false)}>Cancel</button>
    <button class="tbtn primary" id="lvConnect" disabled={!ready || live.connecting} onclick={() => liveActions.connect(endpoint, kind === 'file' ? '' : descriptor)}
      >{live.connecting ? 'Connecting…' : 'Connect'}</button
    >
  </footer>
</div>

{#snippet desc()}
  <label class="lvf"><span>Format descriptor</span><div class="lvpick"><input class="sel" id="lvDesc" bind:value={descriptor} placeholder="frames.toml" spellcheck="false" autocomplete="off" /><button class="tbtn" onclick={() => choose('descriptor')}>Choose…</button></div><small>The same descriptor decodes this logger's recorded files and its live stream.</small></label>
{/snippet}

<style>
  /* the prototype's .slotdlg.livedlg: kinds on the left, the form on the right */
  .livedlg {
    top: 8%;
    left: 50%;
    display: flex;
    flex-direction: column;
    width: min(680px, calc(100vw - 32px));
    max-height: 84vh;
    overflow-y: auto;
    padding: 0;
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    font-size: 13px;
    transform: translateX(-50%);
  }
  .livedlg > header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--grid);
  }
  .livedlg > header b {
    font-weight: 600;
  }
  .dclose {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
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
  .lvdlg {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
  }
  @media (max-width: 600px) {
    .lvdlg {
      grid-template-columns: 1fr;
    }
  }
  .lvkinds {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px;
    border-right: 1px solid var(--grid);
  }
  .lvkind {
    display: grid;
    grid-template-columns: 18px 1fr;
    column-gap: 8px;
    align-items: center;
    padding: 7px 8px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .lvkind :global(svg) {
    grid-row: span 2;
    width: 15px;
    height: 15px;
    color: var(--fg-muted);
  }
  .lvkind b {
    font-size: 13px;
    font-weight: 500;
  }
  .lvkind small {
    grid-column: 2;
    color: var(--fg-muted);
    font-size: 11px;
  }
  .lvkind:hover {
    background: var(--raised);
  }
  .lvkind[aria-pressed='true'] {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .lvform {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    padding: 12px 14px;
  }
  .lvf {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    font-size: 12px;
  }
  .lvf > span:first-child,
  .lvf small {
    color: var(--fg-muted);
  }
  .lvf small {
    font-size: 11px;
  }
  .lvf .sel {
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
  }
  .lvpick {
    display: flex;
    gap: 6px;
  }
  .lvpick .sel {
    flex: 1;
  }
  .lverr {
    margin: 0;
    color: var(--danger);
    font-size: 12px;
  }
  .livedlg > footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--grid);
  }
  .livedlg > footer .muted {
    font-size: 12px;
  }
  .livedlg > footer .tbtn {
    min-height: 30px;
    padding: 0 12px;
    border-radius: 4px;
    font: 500 11.5px var(--font-ui);
  }
  .livedlg > footer .tbtn.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
</style>
