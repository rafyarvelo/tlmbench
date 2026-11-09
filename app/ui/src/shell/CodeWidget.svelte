<!-- A code widget: build(el, info) once and update(el, x) per frame, in a sandboxed iframe (scripts only: no same-origin
     access, no network, no files). Values arrive by postMessage at most ten times a second. Until a shared folder's code
     is trusted it is a plain value card with the question. -->
<script lang="ts">
  import { onDestroy } from 'svelte';

  import { gNum, type Ctx } from '../gauges';
  import { settings } from '../settings.svelte';
  import { trust } from '../customwidgets.svelte';
  import { type CustomWidget, codeTrusted, widgetHostUrl } from '../widgets';

  let { spec, x, title }: { spec: CustomWidget; x: Ctx; title: string } = $props();

  const trusted = $derived(codeTrusted(spec, settings.trustedWidgetSources));
  let frame = $state<HTMLIFrameElement>();
  let ready = false;
  let pending: unknown = null;
  let sentAt = 0;
  let timer = 0;
  onDestroy(() => clearTimeout(timer));

  const tok = (n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
  const color = () => String(x.col).replace(/^var\((--[\w-]+)\)$/, (m, v: string) => tok(v) || m);

  function send(m: unknown) {
    sentAt = performance.now();
    frame?.contentWindow?.postMessage(m, '*');
    pending = null;
  }
  function onload() {
    frame?.contentWindow?.postMessage({ code: spec.code, info: { name: x.name, unit: x.unit, min: gNum(x.lo, x.dec), max: gNum(x.hi, x.dec), fg: tok('--fg') } }, '*');
  }
  function onmessage(e: MessageEvent) {
    if (!e.data?.tlmReady || e.source !== frame?.contentWindow) return;
    ready = true;
    if (pending) send(pending);
  }
  $effect(() => {
    if (!trusted) return;
    const msg = { v: x.ok ? x.v : null, txt: x.txt, pct: x.ok ? +(x.p * 100).toFixed(1) : 0, color: color(), level: x.lvl || 'none', unit: x.unit };
    pending = msg;
    if (!ready) return;
    const wait = 100 - (performance.now() - sentAt);
    if (wait <= 0) send(msg);
    else if (!timer)
      timer = window.setTimeout(() => {
        timer = 0;
        if (pending) send(pending);
      }, wait);
  });
</script>

<svelte:window {onmessage} />

{#if trusted}
  <iframe bind:this={frame} class="gcodefr" sandbox="allow-scripts" title="{spec.name} for {title}" src={widgetHostUrl()} {onload}></iframe>
{:else}
  <div class="gcode off"><b>{x.txt}</b> <span class="muted">{x.unit}</span><button class="tbtn sm" data-trustw={spec.id} data-tip="This widget runs code from {spec.shared}. Choose whether to run it." onclick={(e) => (e.stopPropagation(), (trust.id = spec.id))}>Code widget · Run?</button></div>
{/if}

<style>
  .gcodefr {
    display: block;
    width: 100%;
    height: 96px;
    border: 0;
    background: transparent;
    color-scheme: normal;
  }
  .gcode.off {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 6px;
    padding: 14px 4px;
  }
  .gcode.off b {
    font: 600 20px var(--font-data);
  }
  .gcode.off button {
    flex-basis: 100%;
    justify-content: center;
    margin-top: 6px;
  }
  .muted {
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
