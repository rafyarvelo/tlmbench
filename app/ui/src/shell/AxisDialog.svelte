<!-- A pane's axes ("Axis and scale…" in its ⋯ menu): for each unit's axis (the first on the left, a second on the
     right) its range, decimals, label and log scale. Changes apply as they're made and are kept with the pane. -->
<script lang="ts">
  import { type AxisKey, axisOf, resetAxes, setAxis, units } from '../stack.svelte';
  import { actions } from '../ui.svelte';

  let { i, x, y, onclose }: { i: number; x: number; y: number; onclose: () => void } = $props();

  const axes = $derived((units(i).length ? units(i) : ['']).map((unit, k) => ({ unit, key: `u${k}` as AxisKey, ax: axisOf(i, `u${k}` as AxisKey) })));
  const num = (v: string) => (v.trim() === '' || !Number.isFinite(Number(v)) ? undefined : Number(v));

  function range(key: AxisKey, v: string) {
    const ax = axisOf(i, key);
    const range = (v || undefined) as 'robust' | 'zero' | 'fixed' | undefined;
    // a fixed range starts from what the axis shows now
    const now = range === 'fixed' && (ax.min == null || ax.max == null) ? actions.axisRange(i, key) : null;
    setAxis(i, key, now ? { range, min: +now[0].toPrecision(4), max: +now[1].toPrecision(4) } : { range });
  }
</script>

<div class="dlg axdlg" role="dialog" aria-label="Axes of pane {i + 1}" style="left: {x}px; top: {y}px">
  <header><b>Pane {i + 1} axes</b><span class="spacer"></span><button class="ibtn" data-close aria-label="Close" onclick={onclose}>×</button></header>
  <div class="dbody">
    {#each axes as { unit, key, ax } (key)}
      <section class="axsec" data-axis={key}>
        <div class="axhead"><b>{key === 'u0' ? 'Left axis' : 'Right axis'}</b><span class="dsub">{unit || 'no unit'}</span></div>
        <div class="axrow">
          <label class="axf grow"
            ><span>Range</span>
            <select class="sel" data-ax="range" value={ax.range ?? ''} onchange={(e) => range(key, e.currentTarget.value)}>
              <option value="">Fit the data in view</option>
              <option value="robust">Fit, ignore outliers (spikes are clipped and marked)</option>
              <option value="zero">Fit, always include zero</option>
              <option value="fixed">Fixed</option>
            </select></label
          >
          <label class="axf"
            ><span>Decimals</span>
            <select class="sel" data-ax="dec" value={ax.dec == null ? '' : String(ax.dec)} onchange={(e) => setAxis(i, key, { dec: num(e.currentTarget.value) })}>
              <option value="">Auto</option>
              {#each [0, 1, 2, 3, 4, 5, 6] as n (n)}<option value={String(n)}>{n}</option>{/each}
            </select></label
          >
        </div>
        {#if ax.range === 'fixed'}
          <div class="axrow">
            <label class="axf grow"
              ><span>Min</span><input class="sel" type="number" step="any" data-ax="min" value={ax.min ?? ''} onchange={(e) => setAxis(i, key, { min: num(e.currentTarget.value) })} /></label
            >
            <label class="axf grow"
              ><span>Max</span><input class="sel" type="number" step="any" data-ax="max" value={ax.max ?? ''} onchange={(e) => setAxis(i, key, { max: num(e.currentTarget.value) })} /></label
            >
          </div>
        {/if}
        <div class="axrow">
          <label class="axf grow"
            ><span>Label</span><input
              class="sel"
              data-ax="label"
              value={ax.label ?? ''}
              placeholder={unit || 'none'}
              onchange={(e) => setAxis(i, key, { label: e.currentTarget.value.trim() || undefined })}
            /></label
          >
          <div class="axf">
            <span>Scale</span>
            <label class="axchk"><input type="checkbox" data-ax="log" checked={!!ax.log} onchange={(e) => setAxis(i, key, { log: e.currentTarget.checked || undefined })} />Logarithmic</label>
          </div>
        </div>
      </section>
    {/each}
    <p class="dnote">Kept with the pane, so saved layouts keep their axes. A fixed range turns off auto-fit; double-click still fits the time axis.</p>
  </div>
  <footer>
    <button class="btn" data-axreset onclick={() => resetAxes(i)}>Reset axes</button><span class="spacer"></span><button class="btn primary" data-close onclick={onclose}
      >Done</button
    >
  </footer>
</div>

<style>
  .axdlg {
    width: min(380px, calc(100vw - 32px));
  }
  .axsec + .axsec {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--grid);
  }
  .axhead {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 4px 0;
    font-size: 12px;
  }
  .axrow {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    margin: 6px 0;
  }
  .axf {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: 11.5px;
    color: var(--fg-muted);
  }
  .axf.grow {
    flex: 1;
  }
  .axf .sel {
    min-width: 0;
    color: var(--fg);
    font-size: 12px;
  }
  .axf input[type='number'] {
    font-family: var(--font-data);
  }
  .axchk {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    color: var(--fg);
    font-size: 12px;
  }
</style>
