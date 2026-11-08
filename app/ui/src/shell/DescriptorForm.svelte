<!-- The descriptor form (the prototype's): shown for a binary file nothing claims, and from a file's menu. Pick a descriptor
     you have, or describe the byte layout (framing, time, fields) and watch the core decode the file's first records
     with it. The core words every problem; the form shows the hex of the first bytes, the decoded rows and the TOML it
     writes. Save keeps the descriptor in the app cache; Share… writes it to the folder's .tlmbench/decoders/ (asks
     first: it is the one write beside the data); Open saves it and opens the file through it. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { settings } from '../settings.svelte';
  import { bitsText, CHECKS, COMPRESSIONS, type DField, type DType, dsc, hexLen, INT_TYPES, nameOf, parseBits, SIZES, swatch, TIME_UNITS, TYPES } from '../descriptors.svelte';

  const d = $derived(dsc.d);
  const name = $derived(nameOf(dsc.path));
  const packets = $derived(!!d.packets);
  const pv = $derived(dsc.pv);
  const typed = $derived(!!d.packet_types?.length);
  const timeField = $derived(d.time.source === 'field' || d.time.source === 'counter' ? d.time.field : null);
  /** What the preview was made from: its columns and the colours of its bytes (the form may have moved on). */
  const pd = $derived(dsc.pvD ?? d);
  const pTimeField = $derived(pd.time.source === 'field' || pd.time.source === 'counter' ? pd.time.field : null);
  const shown = $derived(pd.fields.map((_, i) => i).filter((i) => pd.fields[i].name !== pTimeField));
  const problem = $derived(pv?.problem ?? '');
  const stride = $derived(pv && pv.stride > 0 ? pv.stride : Math.max(1, pd.record_size + (pd.packets?.header_size ?? 0)));
  const hexRowLen = $derived(Math.max(16, Math.min(32, stride)));
  const syncLen = $derived(hexLen(d.packets?.sync));
  const file = $derived(`.tlmbench/decoders/${(d.name || 'decoder').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'decoder'}.toml`);
  const folderOk = $derived(!!settings.folder && dsc.path.startsWith(settings.folder));

  // the preview follows every edit a moment after the last one
  $effect(() => {
    if (!dsc.open) return;
    // reading the whole descriptor makes every edit rerun this
    const edited = JSON.stringify(d);
    const t = setTimeout(() => edited && actions.dscPreview(), 60);
    return () => clearTimeout(t);
  });

  const fmtV = (v: number | null) => (v == null || !Number.isFinite(v) ? '—' : Math.abs(v) >= 1e6 || Number.isInteger(v) ? String(v) : (+v.toFixed(4)).toString());
  const timeText = (t: number | null) => (t == null ? '—' : pd.time.source === 'index' ? String(t) : `${fmtV(t)} s`);
  const hexAt = (k: number) => ((pv?.hex_at ?? 0) + k).toString(16).padStart(6, '0');
  const rowsOfHex = $derived.by(() => {
    const out: { at: number; bytes: { x: number; fi: number; sync: boolean }[] }[] = [];
    const bytes = pv?.hex ?? [];
    for (let k = 0; k < Math.min(bytes.length, 2 * hexRowLen); k += hexRowLen) {
      out.push({
        at: k,
        bytes: bytes.slice(k, k + hexRowLen).map((x, j) => {
          const at = (k + j) % stride;
          return { x, fi: pd.fields.findIndex((f) => at >= f.offset && at < f.offset + SIZES[f.type]), sync: !!pd.packets && at < hexLen(pd.packets.sync) };
        }),
      });
    }
    return out;
  });
  const checksNote = $derived(!pv?.rows.length ? '' : pv.bad_checks ? `${pv.bad_checks} bad checksum${pv.bad_checks === 1 ? '' : 's'}` : pv.checked ? 'checksums ok' : 'first records');

  function setFraming(v: string) {
    if (v === 'packets' && !d.packets) d.packets = { header_size: 2, checksum: 'none', compression: 'none' };
    if (v === 'records') delete d.packets;
  }
  function setLength(v: string) {
    const p = d.packets!;
    if (v === 'none') delete p.length;
    else p.length = { offset: p.length?.offset ?? syncLen, type: p.length?.type ?? 'u16', counts: v as 'payload' | 'packet', ...(p.length?.adjust ? { adjust: p.length.adjust } : {}) };
  }
  const guessTime = () => d.fields.find((f) => /time|^t$|_us$|_ms$|_ns$/i.test(f.name) && !f.type.startsWith('f'))?.name ?? '';
  function setSource(v: string) {
    const keep = timeField ?? guessTime();
    if (v === 'rate') d.time = { source: 'rate', rate_hz: d.time.source === 'rate' ? d.time.rate_hz : 100 };
    else if (v === 'field') d.time = { source: 'field', field: keep, unit: 'us' };
    else if (v === 'counter') d.time = { source: 'counter', field: keep, clock_hz: 1e6 };
    else d.time = { source: 'index' };
  }
  function rename(f: DField, to: string) {
    const was = f.name;
    f.name = to.trim();
    if (d.time.source !== 'rate' && d.time.source !== 'index' && d.time.field === was) d.time.field = f.name;
    if (d.sequence === was) d.sequence = f.name;
  }
  function setBits(f: DField, text: string) {
    const b = parseBits(text);
    if (b === null) return;
    if (b) f.bits = b;
    else delete f.bits;
  }
  function addField() {
    const end = Math.max(d.packets?.header_size ?? 0, ...d.fields.map((f) => f.offset + SIZES[f.type]));
    let n = d.fields.length + 1;
    while (d.fields.some((f) => f.name === `field_${n}`)) n++;
    d.fields.push({ name: `field_${n}`, type: 'u16', offset: end });
    if (!dsc.autoSize && end + 2 > d.record_size + (d.packets?.header_size ?? 0)) dsc.autoSize = true;
    requestAnimationFrame(() => {
      const last = document.querySelector<HTMLInputElement>('#dscDlg .dscf:last-child [data-ff="name"]');
      last?.focus();
      last?.select();
    });
  }
  const int = (v: string, min = 0) => Math.max(min, Math.round(+v || 0));
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      actions.dscClose();
    }
  }
  const optLabel = (k: (typeof dsc.known)[number]) => `${k.descriptor.name ?? k.id}${k.descriptor.files?.length ? ` · ${k.descriptor.files.join(', ')}` : ''}${k.shared ? ' (shared)' : ''}`;
</script>

<svelte:window onkeydowncapture={(e) => dsc.open && key(e)} />

{#if dsc.open}
  <div class="scrim" role="presentation" onclick={actions.dscClose}></div>
  <div class="dlg livedlg dscdlg" id="dscDlg" role="dialog" aria-label="Format descriptor">
    <header>
      <b>Describe {name}</b><span class="spacer"></span>
      <button class="dclose" aria-label="Close" onclick={actions.dscClose}>{@html ICON.close}</button>
    </header>
    <div class="impbody"><div class="impmain">
      <p class="lvnote">{name} isn't a format tlmbench reads on its own. Describe its byte layout once: the descriptor decodes every file it names, and the logger's live stream.</p>
      <div class="improw">
        <label class="lvf"><span>Descriptor</span>
          <select class="sel" id="dscPick" value={dsc.pick} onchange={(e) => actions.dscPick(e.currentTarget.value)}>
            <option value="">New descriptor</option>
            {#each dsc.known as k (k.path)}<option value={k.path}>{optLabel(k)}</option>{/each}
          </select></label>
        <label class="lvf"><span>Name</span><input class="sel" id="dscName" value={d.name} onchange={(e) => (d.name = e.currentTarget.value.trim())} autocomplete="off" spellcheck="false" /></label>
        <label class="lvf"><span>Use for files named</span><input class="sel" id="dscFiles" value={d.files.join(', ')} onchange={(e) => (d.files = e.currentTarget.value.split(',').map((s) => s.trim()).filter(Boolean))} autocomplete="off" spellcheck="false" /><small>* matches any text; separate patterns with commas</small></label>
      </div>
      {#if typed}<p class="lvnote warn" id="dscTyped">This descriptor has packet types: each opens as its own recording. The form leaves them as they are; edit the TOML file to change them.</p>{/if}

      <h4>Framing</h4>
      <div class="improw">
        <label class="lvf"><span>Records</span>
          <select class="sel" id="dscFraming" value={packets ? 'packets' : 'records'} onchange={(e) => setFraming(e.currentTarget.value)}>
            <option value="records">Fixed-width records</option><option value="packets">Packets</option>
          </select></label>
        <label class="lvf"><span>Byte order</span>
          <select class="sel" id="dscEndian" bind:value={d.endian}><option value="little">Little-endian</option><option value="big">Big-endian</option></select></label>
        <label class="lvf"><span>File header</span><input class="sel" id="dscHeader" type="number" min="0" step="1" value={d.header_size} onchange={(e) => (d.header_size = int(e.currentTarget.value))} /><small>bytes before the first {packets ? 'packet' : 'record'}</small></label>
        <label class="lvf"><span>Record size</span>
          <input class="sel" id="dscSize" type="number" min="1" step="1" value={d.record_size} data-tip={dsc.autoSize ? 'Follows the fields; type a size to fix it' : ''} onchange={(e) => ((d.record_size = int(e.currentTarget.value, 1)), (dsc.autoSize = false))} /><small>{dsc.autoSize ? 'from the fields' : 'bytes'}</small></label>
      </div>
      {#if d.packets}
        {@const p = d.packets}
        <div class="improw">
          <label class="lvf"><span>Sync word</span><input class="sel" id="dscSync" value={p.sync ?? ''} placeholder="hex, e.g. A55A" onchange={(e) => (p.sync = e.currentTarget.value.trim().toUpperCase() || undefined)} autocomplete="off" spellcheck="false" /></label>
          <label class="lvf"><span>Packet header</span><input class="sel" id="dscPHeader" type="number" min="0" step="1" value={p.header_size} onchange={(e) => (p.header_size = int(e.currentTarget.value))} /><small>bytes before the payload, sync included</small></label>
          <label class="lvf"><span>Length field</span>
            <select class="sel" id="dscLen" value={p.length ? p.length.counts : 'none'} onchange={(e) => setLength(e.currentTarget.value)}>
              <option value="none">None: one record per packet</option><option value="payload">Counts the payload</option><option value="packet">Counts the whole packet</option>
            </select></label>
          {#if p.length}
            {@const l = p.length}
            <label class="lvf"><span>Length at</span>
              <span class="lvf inl"><input class="sel" id="dscLenAt" type="number" min="0" step="1" style="width: 70px" value={l.offset} onchange={(e) => (l.offset = int(e.currentTarget.value))} />
                <select class="sel" id="dscLenType" bind:value={l.type}>{#each INT_TYPES as t (t)}<option value={t}>{t}</option>{/each}</select></span></label>
          {/if}
        </div>
        <div class="improw">
          <label class="lvf"><span>Checksum</span><select class="sel" id="dscCheck" bind:value={p.checksum}>{#each CHECKS as [v, t] (v)}<option value={v}>{t}</option>{/each}</select><small>after the payload, over the header and payload</small></label>
          <label class="lvf"><span>Compression</span><select class="sel" id="dscComp" bind:value={p.compression}>{#each COMPRESSIONS as [v, t] (v)}<option value={v}>{t}</option>{/each}</select><small>of the payload</small></label>
        </div>
        <p class="lvnote">Field offsets count from the start of each packet: the header, then the record. Header fields such as a packet counter read like any other field.</p>
        <div class="impbar sm"><button class="linkbtn" id="dscFind" onclick={actions.dscFind}>Find frames</button><span class="muted" id="dscFound">{dsc.found === false ? 'No repeating sync word found in the first bytes.' : dsc.found ? `Sync ${dsc.found.sync} every ${dsc.found.stride} bytes: applied.` : 'Looks for a sync word repeating at a fixed spacing.'}</span></div>
      {:else}
        <div class="impbar sm"><button class="linkbtn" id="dscFind" onclick={actions.dscFind}>Find frames</button><span class="muted" id="dscFound">{dsc.found === false ? 'No repeating sync word found in the first bytes.' : dsc.found ? `Sync ${dsc.found.sync} every ${dsc.found.stride} bytes: applied.` : 'Looks for a sync word repeating at a fixed spacing; if there is one, the file is packets.'}</span></div>
      {/if}

      <h4>Time</h4>
      <div class="improw">
        <label class="lvf"><span>Time from</span>
          <select class="sel" id="dscTime" value={d.time.source} onchange={(e) => setSource(e.currentTarget.value)}>
            <option value="rate">A fixed sample rate</option><option value="field">A field (a clock)</option><option value="counter">A counter that wraps</option><option value="index">Nothing: use sample numbers</option>
          </select></label>
        {#if d.time.source === 'rate'}
          {@const t = d.time}
          <label class="lvf"><span>Sample rate (Hz)</span><input class="sel" id="dscRate" type="number" min="0" step="any" value={t.rate_hz} onchange={(e) => (t.rate_hz = +e.currentTarget.value)} /></label>
        {:else if d.time.source === 'field' || d.time.source === 'counter'}
          {@const t = d.time}
          <label class="lvf"><span>Field</span>
            <select class="sel" id="dscTimeField" bind:value={t.field}><option value="">Pick a field</option>{#each d.fields as f, i (i)}<option value={f.name}>{f.name}</option>{/each}</select></label>
          {#if t.source === 'field'}
            <label class="lvf"><span>Unit</span><select class="sel" id="dscTimeUnit" bind:value={t.unit}>{#each TIME_UNITS as [v, u] (v)}<option value={v}>{u}</option>{/each}</select></label>
          {:else}
            <label class="lvf"><span>Clock (Hz)</span><input class="sel" id="dscClock" type="number" min="0" step="any" value={t.clock_hz} onchange={(e) => (t.clock_hz = +e.currentTarget.value)} /><small>ticks a second</small></label>
            <label class="lvf"><span>Wraps at</span><input class="sel" id="dscWrap" type="number" min="2" step="1" value={t.wraps_at ?? ''} placeholder="its range" onchange={(e) => (t.wraps_at = e.currentTarget.value ? int(e.currentTarget.value, 2) : undefined)} /><small>the counter's range; blank for the field's own</small></label>
          {/if}
        {/if}
      </div>

      <h4>Fields <small>{d.fields.length} · shown value = raw × scale + offset</small></h4>
      <div class="dscfields">
        <div class="dscf head"><span></span><span>Name</span><span>Type</span><span>Offset</span><span>Bits</span><span>Unit</span><span>Scale</span><span>Offset</span><span></span></div>
        {#each d.fields as f, i (i)}
          <div class="dscf" class:bad={!!problem && (problem.startsWith(`field "${f.name}"`) || problem.includes(`"${f.name}"`))} data-fi={i}>
            <span class="sw" style="background: {swatch(i)}"></span>
            <input data-ff="name" value={f.name} aria-label="Field {i + 1} name" spellcheck="false" onchange={(e) => rename(f, e.currentTarget.value)} />
            <select data-ff="type" aria-label="{f.name} type" value={f.type} onchange={(e) => (f.type = e.currentTarget.value as DType)}>{#each TYPES as t (t)}<option value={t}>{t}</option>{/each}</select>
            <input data-ff="offset" type="number" min="0" step="1" value={f.offset} aria-label="{f.name} byte offset" onchange={(e) => (f.offset = int(e.currentTarget.value))} />
            <input data-ff="bits" value={bitsText(f.bits)} placeholder="all" aria-label="{f.name} bits" data-tip="Only some bits, counting from the least significant: 4-6 takes three. Blank takes all of them." spellcheck="false" disabled={f.type.startsWith('f')} onchange={(e) => (setBits(f, e.currentTarget.value), (e.currentTarget.value = bitsText(f.bits)))} />
            <input data-ff="unit" value={f.unit ?? ''} placeholder="unit" aria-label="{f.name} unit" spellcheck="false" onchange={(e) => (e.currentTarget.value.trim() ? (f.unit = e.currentTarget.value.trim()) : delete f.unit)} />
            <input data-ff="scale" type="number" step="any" value={f.scale ?? 1} aria-label="{f.name} scale" onchange={(e) => (e.currentTarget.value === '' ? delete f.scale : (f.scale = +e.currentTarget.value))} />
            <input data-ff="value_offset" type="number" step="any" value={f.value_offset ?? 0} aria-label="{f.name} value offset" onchange={(e) => (f.value_offset = +e.currentTarget.value || 0)} />
            <button class="ib" data-fdel aria-label="Remove {f.name}" data-tip="Remove this field" onclick={() => d.fields.splice(i, 1)}>×</button>
          </div>
        {/each}
      </div>
      <div class="impbar sm"><button class="tbtn" id="dscAdd" onclick={addField}>+ Add field</button></div>

      <h4>Preview <small>{name}{pv?.rows.length ? ` · ${pv.skipped ? `${pv.skipped} bytes skipped to the first sync · ` : ''}${checksNote}` : ''}</small></h4>
      <div class="dschex" id="dscHex" aria-label="First bytes of the file">
        {#each rowsOfHex as row (row.at)}
          <div><span class="at">{hexAt(row.at)}</span>{#each row.bytes as b, j (j)}{#if j}{' '}{/if}<span class:sync={b.sync} style={b.fi >= 0 ? `background: color-mix(in srgb, ${swatch(b.fi)} 30%, transparent)` : ''} title={b.fi >= 0 ? pd.fields[b.fi].name : b.sync ? 'sync' : ''}>{b.x.toString(16).padStart(2, '0').toUpperCase()}</span>{/each}</div>
        {/each}
      </div>
      {#if pv?.rows.length}
        <table class="dsctab" id="dscTable">
          <thead><tr><th>Time</th>{#each shown as i (i)}<th>{pd.fields[i].name}{pd.fields[i].unit ? ` (${pd.fields[i].unit})` : ''}</th>{/each}{#if pv.checked}<th>Check</th>{/if}</tr></thead>
          <tbody>
            {#each pv.rows as r, k (k)}
              <tr><td>{timeText(r.time)}</td>{#each shown as i (i)}<td>{fmtV(r.values[i])}</td>{/each}{#if pv.checked}<td class:bad={!r.ok}>{r.ok ? 'ok' : 'bad'}</td>{/if}</tr>
            {/each}
          </tbody>
        </table>
      {:else if pv}
        <p class="dscerr" id="dscErr">{pv.problem ?? pv.error ?? ''}</p>
      {/if}
      {#if pv?.problem && pv.rows.length}<p class="dscerr" id="dscErr">{pv.problem}</p>{/if}
      <details class="dscview" open={dsc.tomlOpen} ontoggle={(e) => (dsc.tomlOpen = e.currentTarget.open)}>
        <summary class="lvnote">TOML ({file.split('/').pop()})</summary><pre class="dsctoml" id="dscToml">{pv?.toml ?? ''}</pre>
      </details>
    </div></div>
    {#if dsc.error}<p class="lverr" id="dscFail">{dsc.error}</p>{/if}
    {#if dsc.confirmShare}
      <div class="dscshare" id="dscShareAsk">
        <span>Write <code>{file}</code> next to the data? Everyone with the folder (or its git repository) decodes these files the same way.</span>
        <button class="tbtn" id="dscShareGo" onclick={actions.dscShareGo}>Write it</button>
        <button class="tbtn" onclick={() => (dsc.confirmShare = false)}>Cancel</button>
      </div>
    {/if}
    <footer>
      <button class="tbtn" id="dscShare" disabled={!!problem || !folderOk} data-tip={folderOk ? `Write it to ${file} so everyone with the folder decodes these files` : 'Share writes into the folder Explorer shows, and this file isn’t in it'} onclick={() => (dsc.confirmShare = true)}>Share…</button>
      <span class="spacer"></span>
      <button class="tbtn" onclick={actions.dscClose}>Cancel</button>
      <button class="tbtn" id="dscSave" disabled={!!problem || dsc.busy} onclick={actions.dscSave}>Save</button>
      <button class="tbtn primary" id="dscOpen" disabled={!!problem || dsc.busy} onclick={actions.dscOpen}>Open</button>
    </footer>
  </div>
{/if}

<style>
  .livedlg {
    top: 6%;
    left: 50%;
    display: flex;
    flex-direction: column;
    width: min(1000px, calc(100vw - 32px));
    max-height: 88vh;
    overflow: hidden;
    padding: 0;
    gap: 0;
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    font-size: 13px;
    transform: translateX(-50%);
  }
  .livedlg > header,
  .livedlg > footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
  }
  .livedlg > header {
    border-bottom: 1px solid var(--grid);
  }
  .livedlg > header b {
    font-weight: 600;
  }
  .livedlg > footer {
    padding: 10px 14px;
    border-top: 1px solid var(--grid);
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
  .livedlg > footer .tbtn:disabled {
    opacity: 0.5;
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
  .lverr {
    margin: 0;
    padding: 0 14px 8px;
    color: var(--danger);
    font-size: 12px;
  }
  .impbody {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .impmain {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    padding: 10px 14px;
    overflow-y: auto;
  }
  .impmain h4 {
    margin: 6px 0 0;
    color: var(--fg-muted);
    font-size: 11.5px;
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .impmain h4 small {
    margin-left: 6px;
    letter-spacing: 0;
    text-transform: none;
  }
  .lvnote {
    margin: 0;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .lvnote.warn {
    color: var(--warn);
  }
  .improw {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .improw .lvf {
    flex: 1;
    min-width: 150px;
  }
  .lvf {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    font-size: 12px;
  }
  .lvf > span:first-child {
    color: var(--fg-muted);
  }
  .lvf small {
    color: var(--fg-muted);
    font-size: 11px;
  }
  .lvf .sel {
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
  }
  .lvf.inl {
    flex-direction: row;
    gap: 6px;
  }
  .impbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .impbar .muted {
    font-size: 12px;
  }
  .impbar .linkbtn {
    padding: 0;
    color: var(--accent);
    font-size: 12px;
  }
  .dscfields,
  .dschex,
  .dsctab,
  .dscview {
    flex: none;
  }
  .dscfields {
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .dscf {
    display: grid;
    grid-template-columns: 10px minmax(110px, 1.4fr) 76px 64px 60px 70px 70px 70px 24px;
    gap: 6px;
    align-items: center;
    padding: 3px 6px;
    border-radius: 4px;
    font-size: 12px;
  }
  .dscf.head {
    color: var(--fg-muted);
    font-size: 11px;
  }
  .dscf.bad {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .dscf .sw {
    width: 10px;
    height: 10px;
    border-radius: 4px;
  }
  .dscf input,
  .dscf select {
    min-width: 0;
    min-height: 24px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--raised);
    color: var(--fg);
    font: 12px var(--font-ui);
  }
  .dscf input[type='number'],
  .dscf input[data-ff='bits'] {
    font-family: var(--font-data);
  }
  .dscf input:disabled {
    opacity: 0.4;
  }
  .dscf .ib {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 15px;
    cursor: pointer;
    opacity: 0;
  }
  .dscf:hover .ib,
  .dscf:focus-within .ib {
    opacity: 1;
  }
  .dscf .ib:hover {
    border-color: var(--border);
    background: var(--surface);
    color: var(--fg);
  }
  @media (hover: none) {
    .dscf .ib {
      opacity: 1;
    }
  }
  .dschex {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    font: 11.5px var(--font-data);
  }
  .dschex div {
    white-space: nowrap;
  }
  .dschex span {
    padding: 0 1px;
    border-radius: 4px;
  }
  .dschex .at {
    margin-right: 8px;
    color: var(--fg-muted);
  }
  .dschex .sync {
    text-decoration: underline;
  }
  .dsctab {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .dsctab th {
    padding: 2px 6px;
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 500;
    text-align: left;
  }
  .dsctab td {
    padding: 2px 6px;
    border-top: 1px solid var(--grid);
    font-family: var(--font-data);
    white-space: nowrap;
  }
  .dsctab td.bad {
    color: var(--danger);
  }
  .dscerr {
    margin: 0;
    color: var(--danger);
    font-size: 12px;
  }
  .dsctoml {
    max-height: 220px;
    margin: 6px 0 0;
    padding: 8px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    font: 11.5px var(--font-data);
    white-space: pre;
  }
  .dscshare {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-top: 1px solid var(--grid);
    background: var(--accent-soft);
    font-size: 12px;
  }
  .dscshare span {
    flex: 1;
    min-width: 200px;
  }
</style>
