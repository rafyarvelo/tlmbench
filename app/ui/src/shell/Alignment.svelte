<!-- One file's Alignment: how it lines up with the primary (file 1). A method, what it found (residuals, quality), a
     look at the result before it stays (Keep or Undo), a nudge on top of any method, a lock, and the file's channel
     prefix. Everything reads through the file's time map; nothing in the data is rewritten. -->
<script lang="ts">
  import { duration } from '../axes';
  import { ui, actions, type AlignMethod, type SlotRow } from '../ui.svelte';
  import { estimators, readPlugins } from '../plugins.svelte';

  let { slot }: { slot: SlotRow } = $props();
  const a = $derived(slot.align);
  const m = $derived(a.method);
  let step = $state(0.01);
  let seconds = $state<number | null>(null);
  // alignment plugins, listed after the built-in methods
  $effect(() => void readPlugins());
  const plug = $derived(estimators());
  /** The select's value: a built-in method, or `plugin:<name>` (a plugin that isn't loaded now stays listed). */
  const choice = $derived(m === 'plugin' ? `plugin:${a.plugin ?? ''}` : m);

  const METHODS: [AlignMethod, string][] = [
    ['starts', 'Align starts'],
    ['clock', 'Same clock'],
    ['timesrc', 'Time channel'],
    ['xcorr', 'Cross-correlation'],
    ['events', 'Event matching'],
    ['sync', 'Sync points'],
    ['manual', 'Offset'],
  ];
  /** Channels this file has under a name the primary has too (what a method can match on). */
  const shared = $derived.by(() => {
    const prim = ui.slots[0];
    const mine = new Set(ui.channels.slice(slot.first, slot.first + slot.count).map((c) => c.name.replace(new RegExp(`·${slot.slot}$`), '')));
    return ui.channels.slice(prim.first, prim.first + prim.count).map((c) => c.name).filter((n) => mine.has(n));
  });
  const signed = (s: number) => `${s < 0 ? '−' : '+'}${duration(Math.abs(s) * 1e9)}`;
  const pct = (q: number) => `${Math.round(q * 100)}%`;
  const clean = (n: string) => n;

  function choose(e: Event) {
    const raw = (e.currentTarget as HTMLSelectElement).value;
    if (raw.startsWith('plugin:')) return actions.alignSet(slot.slot, 'plugin', { plugin: raw.slice(7), params: a.plugin === raw.slice(7) ? a.params : '' });
    const v = raw as AlignMethod;
    actions.alignSet(slot.slot, v, { seconds: v === 'manual' ? a.offset : undefined, channel: v === 'xcorr' || v === 'timesrc' ? a.channel : undefined });
  }
  /** The residuals as a small plot: time across, residual up (ms), a line at zero. */
  const plot = $derived.by(() => {
    const r = a.residuals;
    if (r.length < 2) return null;
    const xs = r.map((p) => p[0]);
    const [x0, x1] = [Math.min(...xs), Math.max(...xs)];
    const worst = Math.max(...r.map((p) => Math.abs(p[1])), 0.001);
    const pts = r.map((p) => [4 + ((p[0] - x0) / Math.max(1e-9, x1 - x0)) * 212, 20 - (p[1] / worst) * 16] as const);
    return { pts, worst };
  });
</script>

<div class="align" data-align={slot.slot} aria-label="Alignment of {slot.name}">
  <label class="arow">
    <span>Alignment</span>
    <select class="sel" data-falign={slot.slot} value={choice} disabled={a.locked} aria-label="Alignment for {slot.name}" onchange={choose}>
      {#each METHODS as [id, label] (id)}<option value={id}>{label}</option>{/each}
      {#each plug as p (p.path)}<option value="plugin:{p.name}">{p.name} (plugin)</option>{/each}
      {#if m === 'plugin' && !plug.some((p) => p.name === a.plugin)}<option value="plugin:{a.plugin ?? ''}">{a.plugin} (not loaded)</option>{/if}
    </select>
  </label>

  {#if m === 'manual'}
    <label class="arow">
      <span>Seconds after file 1 starts</span>
      <input class="sel" type="number" step="0.1" data-foff={slot.slot} disabled={a.locked} value={seconds ?? a.offset} aria-label="Offset in seconds for {slot.name}"
        onchange={(e) => ((seconds = +e.currentTarget.value), actions.alignSet(slot.slot, 'manual', { seconds: +e.currentTarget.value }))} />
    </label>
  {/if}

  {#if m === 'xcorr' || m === 'timesrc' || m === 'events'}
    <label class="arow">
      <span>{m === 'events' ? 'Event name' : 'Channel'}</span>
      <select class="sel" data-fchan={slot.slot} disabled={a.locked} value={a.channel ?? ''} aria-label="Channel to match on"
        onchange={(e) => actions.alignSet(slot.slot, m, { channel: e.currentTarget.value || undefined })}>
        <option value="">Automatic</option>
        {#each shared as n (n)}<option value={n}>{clean(n)}</option>{/each}
      </select>
    </label>
  {/if}

  {#if m === 'plugin'}
    <label class="arow">
      <span>Parameters</span>
      <input class="sel" data-fparams={slot.slot} disabled={a.locked} value={a.params ?? ''} placeholder={'{"channel": "name"}'} spellcheck="false" aria-label="Parameters for {a.plugin}"
        onchange={(e) => actions.alignSet(slot.slot, 'plugin', { plugin: a.plugin, params: e.currentTarget.value.trim() })} />
    </label>
  {/if}

  {#if m === 'sync'}
    <div class="pairs" data-pairs>
      {#each a.pairs as p, i (i)}
        <div class="pair"><span class="num">{p[0].toFixed(2)} s → {p[1].toFixed(2)} s</span><button class="x" aria-label="Remove point {i + 1}" data-tip="Remove this point" onclick={() => actions.alignPairs(slot.slot, a.pairs.filter((_, k) => k !== i))}>×</button></div>
      {/each}
      <button class="linkbtn" data-addpair disabled={a.locked || !ui.cursors} data-tip="Put cursor A on a moment in file 1 and cursor B on the same moment in this file" onclick={() => actions.alignPair(slot.slot)}>Add a point from cursors A and B</button>
    </div>
  {/if}

  {#if a.note}<small class="anote" class:bad={a.quality == null && /^(No|The |Both|Not|A file|Couldn)/.test(a.note)}>{a.note}</small>{/if}
  {#if a.quality != null}
    <div class="fit" data-fit>
      <span class="q" data-quality title="How well the points fit">Fit {pct(a.quality)}</span>
      {#if plot}
        <svg viewBox="0 0 220 40" width="100%" height="40" role="img" aria-label="Residuals, up to ±{plot.worst.toFixed(2)} ms">
          <line x1="4" x2="216" y1="20" y2="20" class="zero" />
          {#each plot.pts as p, i (i)}<circle cx={p[0]} cy={p[1]} r="2" />{/each}
        </svg>
      {/if}
    </div>
  {/if}
  {#if a.preview}
    <div class="keep" data-preview>
      <span>Showing the result.</span>
      <button class="tbtn act" data-keep onclick={() => actions.alignApply(slot.slot)}>Keep</button>
      <button class="tbtn" data-undo onclick={() => actions.alignDiscard(slot.slot)}>Undo</button>
    </div>
  {/if}

  <div class="arow facts">
    <span>Now</span>
    <span class="num" data-offset>{signed(a.offset)}{a.drift ? ` · ${a.drift > 0 ? '+' : '−'}${Math.abs(a.drift).toFixed(1)} ppm` : ''}</span>
  </div>
  <div class="arow">
    <span>Nudge</span>
    <span class="nudge">
      <button class="tbtn" data-nudge="-" disabled={a.locked} aria-label="Earlier by {step} s" onclick={() => actions.alignNudge(slot.slot, -step)}>−</button>
      <input class="sel" type="number" min="0.001" step="0.01" bind:value={step} aria-label="Nudge step in seconds" />
      <span class="u">s</span>
      <button class="tbtn" data-nudge="+" disabled={a.locked} aria-label="Later by {step} s" onclick={() => actions.alignNudge(slot.slot, step)}>+</button>
    </span>
  </div>
  <label class="arow">
    <span>Lock</span>
    <input type="checkbox" data-lock={slot.slot} checked={a.locked} onchange={(e) => actions.alignLock(slot.slot, e.currentTarget.checked)} aria-label="Lock the alignment of {slot.name}" />
  </label>
  <label class="arow">
    <span>Channel prefix</span>
    <input class="sel" type="text" data-prefix={slot.slot} placeholder={slot.path.split(/[\\/]/).pop()?.replace(/\.[^.]*$/, '')} value={slot.name === slot.path.split(/[\\/]/).pop()?.replace(/\.[^.]*$/, '') ? '' : slot.name} aria-label="Channel prefix for {slot.name}"
      onchange={(e) => actions.setPrefix(slot.path, e.currentTarget.value)} />
  </label>
</div>

<style>
  .align {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 2px 8px 6px 30px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 11.5px;
  }
  .arow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--fg-muted);
    min-width: 0;
  }
  .arow > :global(select),
  .arow > :global(input[type='text']),
  .arow > :global(input[type='number']) {
    min-width: 0;
    max-width: 60%;
    font-size: 11.5px;
  }
  .nudge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .nudge input {
    width: 60px;
    font-size: 11.5px;
  }
  .u {
    color: var(--fg-muted);
  }
  .num {
    font-family: var(--font-data);
    color: var(--fg);
  }
  .anote {
    color: var(--fg-muted);
    line-height: 1.35;
  }
  .anote.bad {
    color: var(--warn);
  }
  .fit {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .fit svg {
    flex: 1;
    min-width: 0;
    color: var(--accent);
  }
  .fit .zero {
    stroke: var(--grid);
  }
  .fit circle {
    fill: currentColor;
  }
  .q {
    font-family: var(--font-data);
    color: var(--fg);
  }
  .keep {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg-muted);
  }
  .keep span {
    flex: 1;
  }
  .pairs {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .pair {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .x {
    background: none;
    border: 0;
    color: var(--fg-muted);
    cursor: pointer;
    font-size: 13px;
  }
</style>
