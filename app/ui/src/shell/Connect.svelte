<!-- Connect a live source: a serial port, TCP (the app connects to the device), UDP (it listens) or a growing file
     (followed from its start), with the frames' descriptor for the first three. It opens a live tab, recording into the Recordings folder. -->
<script lang="ts">
  import { live, liveActions } from '../live.svelte';
  import { actions } from '../ui.svelte';

  let kind = $state<'serial' | 'tcp' | 'udp' | 'file'>('tcp');
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
<div class="dlg cdlg" id="liveDlg" role="dialog" aria-label="Connect a live source">
  <header><b>Connect a live source</b></header>
  <div class="dbody">
    <div class="kinds" role="radiogroup" aria-label="Source">
      {#each [['serial', 'Serial'], ['tcp', 'TCP'], ['udp', 'UDP'], ['file', 'Growing file']] as [k, label] (k)}
        <button role="radio" aria-checked={kind === k} data-kind={k} onclick={() => (kind = k as typeof kind)}>{label}</button>
      {/each}
    </div>
    {#if kind === 'file'}
      <label class="f"><span>File</span><span class="row"><input class="sel" id="lvPath" bind:value={path} placeholder="/path/to/log.csv" spellcheck="false" /><button class="tbtn" onclick={() => choose('path')}>Choose…</button></span></label>
      <p class="dnote">Recorded from its start, then followed as it grows. The file is only read.</p>
    {:else if kind === 'serial'}
      <label class="f"><span>Port</span><input class="sel" id="lvPort" bind:value={port} placeholder="/dev/tty.usbserial-1 or COM3" spellcheck="false" /></label>
      <label class="f"><span>Baud rate</span><select class="sel" id="lvBaud" bind:value={baud}>
          {#each ['9600', '19200', '38400', '57600', '115200', '230400', '460800', '921600'] as b (b)}<option value={b}>{b}</option>{/each}
        </select></label>
      <label class="f"><span>Descriptor</span><span class="row"><input class="sel" id="lvDesc" bind:value={descriptor} placeholder="frames.toml" spellcheck="false" /><button class="tbtn" onclick={() => choose('descriptor')}>Choose…</button></span></label>
    {:else}
      <label class="f"><span>{kind === 'tcp' ? 'Connect to' : 'Listen on'}</span><input class="sel" id="lvAddr" bind:value={address} placeholder="host:port" spellcheck="false" /></label>
      <label class="f"><span>Descriptor</span><span class="row"><input class="sel" id="lvDesc" bind:value={descriptor} placeholder="frames.toml" spellcheck="false" /><button class="tbtn" onclick={() => choose('descriptor')}>Choose…</button></span></label>
      <p class="dnote">The descriptor lays out each frame (a TOML file, as for fixed-width recordings).</p>
    {/if}
    {#if live.error}<p class="err" id="lvErr">{live.error}</p>{/if}
  </div>
  <footer>
    <span class="dnote">Recorded to the Recordings folder</span><span class="spacer"></span>
    <button class="btn" onclick={() => (live.connect = false)}>Cancel</button>
    <button class="btn primary" id="lvConnect" disabled={!ready || live.connecting} onclick={() => liveActions.connect(endpoint, kind === 'file' ? '' : descriptor)}
      >{live.connecting ? 'Connecting…' : 'Connect'}</button
    >
  </footer>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 59;
    background: rgba(0, 0, 0, 0.25);
  }
  .cdlg {
    top: 80px;
    left: 50%;
    width: min(440px, calc(100vw - 32px));
    transform: translateX(-50%);
  }
  .kinds {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 2px;
    margin: 6px 0 10px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
  }
  .kinds button {
    min-height: 26px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .kinds button[aria-checked='true'] {
    background: var(--accent-soft);
    color: var(--fg);
  }
  .f {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 8px;
    font-size: 12px;
  }
  .f > span:first-child {
    color: var(--fg-muted);
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row input,
  .f > input {
    flex: 1;
    font-family: var(--font-data);
    font-size: 12px;
  }
  .err {
    margin: 6px 0 0;
    color: var(--danger, #ff6b78);
    font-size: 12px;
  }
</style>
