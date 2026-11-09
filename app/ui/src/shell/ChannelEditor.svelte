<!-- The channel editor: how a channel looks everywhere (name shown, colour, decimals, unit label, line style). It is
     kept in settings by channel name, so it follows the channel into every pane and file. Changes apply as made. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { settings, setSetting } from '../settings.svelte';
  import { closeChannelEditor, dispOf, editor, limitsOf, setDisplay, setLimits } from '../display.svelte';
  import { ui } from '../ui.svelte';

  const info = $derived(editor.ch == null ? null : ui.channels[editor.ch]);
  const d = $derived(info ? dispOf(info.name) : {});
  const set = (patch: Record<string, unknown>) => info && setDisplay(info.name, { ...d, ...patch });
  const num = (v: string) => (v === '' || !Number.isFinite(+v) ? undefined : +v);
  const custom = $derived(typeof d.color === 'string' ? d.color : '');
  const lim = $derived(info ? limitsOf(info.name) : {});
  const setLim = (k: string, v: string) => {
    if (!info) return;
    const next = { ...lim } as Record<string, number | undefined>;
    if (v.trim() === '' || !Number.isFinite(+v)) delete next[k];
    else next[k] = +v;
    setLimits(info.name, next);
  };
  const LIMS: [string, string][] = [['al', 'Alarm at or below'], ['ah', 'Alarm at or above'], ['wl', 'Warn at or below'], ['wh', 'Warn at or above']];
  const mapText = $derived(Object.entries(info ? (settings.valueMaps[info.name] ?? {}) : {}).map(([k, l]) => `${k} = ${l}`).join('\n'));
  function commitLabels(text: string) {
    if (!info) return;
    const m: Record<string, string> = {};
    for (const line of text.split(/[\n,;]+/)) {
      const x = /^\s*(-?\d+(?:\.\d+)?)\s*[=:]\s*(.+?)\s*$/.exec(line);
      if (x) m[String(+x[1])] = x[2];
    }
    const all = { ...settings.valueMaps };
    if (Object.keys(m).length) all[info.name] = m;
    else delete all[info.name];
    setSetting('valueMaps', all);
  }
  const miss = $derived(d.missing ?? {});
  const setMiss = (k: 'vals' | 'lo' | 'hi' | 'when', raw: string) => {
    const m: NonNullable<typeof d.missing> = { ...miss };
    if (k === 'when') {
      if (raw.trim()) m.when = raw.trim();
      else delete m.when;
    } else if (k === 'vals') {
      const vals = raw.split(/[,;\s]+/).filter((t) => t !== '' && Number.isFinite(+t)).map(Number);
      if (vals.length) m.vals = vals;
      else delete m.vals;
    } else if (raw === '' || !Number.isFinite(+raw)) delete m[k];
    else m[k] = +raw;
    set({ missing: Object.keys(m).length ? m : undefined });
  };
  const WIDTHS: [string, string][] = [['', 'Normal'], ['1', 'Thin'], ['2', 'Thick'], ['3', 'Heavy']];
</script>

{#if info}
  <div class="scrim" role="presentation" onclick={closeChannelEditor}></div>
  <div class="dlg chdlg" id="chEditor" role="dialog" aria-label="Channel {info.name}">
    <header><b>{info.name}</b><span class="dsub">{info.unit ?? ''}</span><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={closeChannelEditor}>{@html ICON.close}</button></header>
    <div class="tabs" role="tablist">
      <button role="tab" data-tab="display" aria-selected={editor.tab === 'display'} onclick={() => (editor.tab = 'display')}>Display</button>
      <button role="tab" data-tab="limits" aria-selected={editor.tab === 'limits'} onclick={() => (editor.tab = 'limits')}>Limits</button>
      <button role="tab" data-tab="labels" aria-selected={editor.tab === 'labels'} onclick={() => (editor.tab = 'labels')}>Value labels</button>
    </div>
    {#if editor.tab === 'labels'}
    <div class="chbody">
      <label class="lvf"><span>Value labels</span>
        <textarea class="sel" data-vl rows="7" spellcheck="false" placeholder={'0 = IDLE\n1 = ARMED\n2 = CRUISE'} value={mapText} onchange={(e) => commitLabels(e.currentTarget.value)}></textarea>
        <small>One per line: value = text. Tiles, tables and Measure show the text; values without a label show as numbers. Expressions can use it: <code>state == 'CRUISE'</code>.</small></label>
      <p class="hint">Saved in settings for “{info.name}”, in every pane and file.</p>
    </div>
    <footer><button class="linkbtn" data-vlclear onclick={() => commitLabels('')}>No labels</button><span class="spacer"></span><button class="tbtn primary" data-close onclick={closeChannelEditor}>Done</button></footer>
    {:else if editor.tab === 'limits'}
    <div class="chbody">
      <div class="lvrow">{#each LIMS.slice(0, 2) as [k, t] (k)}<label class="lvf"><span>{t}</span><input class="sel" type="number" step="any" data-lim={k} value={lim[k as 'al'] ?? ''} placeholder="none" oninput={(e) => setLim(k, e.currentTarget.value)} /></label>{/each}</div>
      <div class="lvrow">{#each LIMS.slice(2) as [k, t] (k)}<label class="lvf"><span>{t}</span><input class="sel" type="number" step="any" data-lim={k} value={lim[k as 'al'] ?? ''} placeholder="none" oninput={(e) => setLim(k, e.currentTarget.value)} /></label>{/each}</div>
      <p class="hint">Charts draw each limit as a dashed line with shading beyond it: yellow past warn, red past alarm. Value tiles turn green, yellow or red.</p>
      <p class="hint">Saved in settings for “{info.name}”, in every pane and file.</p>
    </div>
    <footer><button class="linkbtn" data-clear onclick={() => setLimits(info.name, {})}>No limits</button><span class="spacer"></span><button class="tbtn primary" data-close onclick={closeChannelEditor}>Done</button></footer>
    {:else}
    <div class="chbody">
      <label class="lvf"><span>Name shown</span>
        <input class="sel" data-d="alias" value={d.alias ?? ''} placeholder={info.name} onchange={(e) => set({ alias: e.currentTarget.value.trim() })} />
        <small>Used in chips, tables and tiles. Search still finds the original name.</small></label>
      <div class="lvf"><span>Colour</span>
        <div class="swrow">
          {#each [1, 2, 3, 4, 5, 6, 7, 8] as n (n)}
            <button class="sw" class:on={d.color === n} data-sw={n} style="background: var(--trace-{n})" aria-label="Colour {n}" onclick={() => set({ color: n })}></button>
          {/each}
          <label class="swc" data-tip="Custom colour"><input type="color" data-d="color" value={custom || '#888888'} onchange={(e) => set({ color: e.currentTarget.value })} /><span class:on={!!custom} style={custom ? `background: ${custom}` : ''}>+</span></label>
          <button class="tbtn" data-autocolor disabled={d.color == null} onclick={() => set({ color: undefined })}>Auto</button>
        </div></div>
      <div class="lvrow">
        <label class="lvf"><span>Decimals</span>
          <select class="sel" data-d="dec" value={d.dec == null ? '' : String(d.dec)} onchange={(e) => set({ dec: num(e.currentTarget.value) })}>
            <option value="">Auto</option>{#each [0, 1, 2, 3, 4, 5, 6] as n (n)}<option value={String(n)}>{n}</option>{/each}
          </select></label>
        <label class="lvf"><span>Unit label</span>
          <input class="sel" data-d="unit" value={d.unit ?? ''} placeholder={info.unit ?? 'none'} onchange={(e) => set({ unit: e.currentTarget.value.trim() })} /></label>
      </div>
      <div class="lvrow">
        <label class="lvf"><span>Scale</span>
          <input class="sel" type="number" step="any" data-d="scale" value={d.scale ?? ''} placeholder="1" onchange={(e) => set({ scale: num(e.currentTarget.value) })} /></label>
        <label class="lvf"><span>Offset</span>
          <input class="sel" type="number" step="any" data-d="offset" value={d.offset ?? ''} placeholder="0" onchange={(e) => set({ offset: num(e.currentTarget.value) })} /></label>
      </div>
      <p class="hint">Shown value = stored × scale + offset, for example ADC counts to volts. A format descriptor can set this for every file of that logger.</p>
      <div class="lvrow">
        <label class="lvf"><span>Treat as missing</span>
          <input class="sel" data-m="vals" value={(miss.vals ?? []).join(', ')} placeholder="-9999, 65535" spellcheck="false" onchange={(e) => setMiss('vals', e.currentTarget.value)} /></label>
        <label class="lvf"><span>Valid from</span>
          <input class="sel" type="number" step="any" data-m="lo" value={miss.lo ?? ''} placeholder="any" onchange={(e) => setMiss('lo', e.currentTarget.value)} /></label>
        <label class="lvf"><span>Valid to</span>
          <input class="sel" type="number" step="any" data-m="hi" value={miss.hi ?? ''} placeholder="any" onchange={(e) => setMiss('hi', e.currentTarget.value)} /></label>
      </div>
      <label class="lvf"><span>Missing when</span>
        <input class="sel" data-m="when" value={miss.when ?? ''} placeholder="gps_fix < 3 or status == 'ERR'" spellcheck="false" onchange={(e) => setMiss('when', e.currentTarget.value)} />
        <small>Values, range and condition use the stored numbers, before scale. A condition can use any channel of the file.</small></label>
      {#if editor.error}<p class="hint err" id="chErr">⚠ {editor.error}</p>{/if}
      <p class="hint">Missing values become gaps everywhere: charts, autoscale, tables, statistics, derived channels and exports.{#if info.missing} <b>{info.missing.toLocaleString()}</b> are missing in this file now.{/if}</p>
      <div class="lvrow">
        <label class="lvf"><span>Line width</span>
          <select class="sel" data-d="width" value={d.width == null ? '' : String(d.width)} onchange={(e) => set({ width: num(e.currentTarget.value) })}>
            {#each WIDTHS as [v, t] (v)}<option value={v}>{t}</option>{/each}
          </select></label>
        <div class="lvf"><span>Style</span>
          <label class="chk"><input type="checkbox" data-d="dash" checked={!!d.dash} onchange={(e) => set({ dash: e.currentTarget.checked })} />Dashed</label>
          <label class="chk"><input type="checkbox" data-d="step" checked={!!d.step} onchange={(e) => set({ step: e.currentTarget.checked })} />Steps (flags, modes)</label></div>
      </div>
      <p class="hint">Saved in settings for “{info.name}”, in every pane and file.</p>
    </div>
    <footer><button class="linkbtn" data-dreset onclick={() => setDisplay(info.name, {})}>Reset display</button><span class="spacer"></span><button class="tbtn primary" data-close onclick={closeChannelEditor}>Done</button></footer>
    {/if}
  </div>
{/if}

<style>
  .chdlg {
    width: min(380px, calc(100vw - 32px));
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
  .tabs {
    display: flex;
    gap: 4px;
    padding: 6px 14px 0;
  }
  .tabs button {
    padding: 4px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .tabs button[aria-selected='true'] {
    border-bottom-color: var(--accent);
    color: var(--fg);
  }
  .chbody {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 14px;
    overflow-y: auto;
  }
  .lvf {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }
  .lvf > span:first-child {
    color: var(--fg-muted);
    font-size: 11px;
  }
  .lvf small,
  .hint {
    color: var(--fg-muted);
    font-size: 11px;
    margin: 0;
  }
  .lvrow {
    display: flex;
    gap: 10px;
  }
  .swrow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .sw {
    width: 20px;
    height: 20px;
    border: 2px solid transparent;
    border-radius: 50%;
    padding: 0;
  }
  .sw.on {
    border-color: var(--fg);
  }
  .swc {
    position: relative;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
  }
  .swc {
    overflow: hidden;
    border-radius: 50%;
  }
  .swc input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    opacity: 0;
    cursor: pointer;
  }
  .swc span {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border: 1px dashed var(--border);
    border-radius: 50%;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .swc span.on {
    border: 2px solid var(--fg);
    color: transparent;
  }
  .err {
    color: var(--danger);
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }
</style>
