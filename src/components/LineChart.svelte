<script lang="ts">
  type Point = { label: string; value: number | null };
  let { points, max = 100, unit = "%", name }: { points: Point[]; max?: number; unit?: string; name: string } = $props();

  const H = 190;
  const L = 40;
  const R = 18;
  const T = 12;
  const B = 28;

  let width = $state(600);
  let hover = $state<number | null>(null);

  const plotW = $derived(Math.max(80, width - L - R));
  const plotH = H - T - B;
  const sx = (i: number) => L + (points.length <= 1 ? plotW / 2 : (i / (points.length - 1)) * plotW);
  const sy = (v: number) => T + plotH - (v / max) * plotH;
  const data = $derived(points.map((p, i) => ({ ...p, i })).filter((p): p is { label: string; value: number; i: number } => p.value !== null));
  const line = $derived(data.map((p, k) => `${k ? "L" : "M"}${sx(p.i)},${sy(p.value)}`).join(""));
  const area = $derived(data.length > 1 ? `${line}L${sx(data[data.length - 1].i)},${sy(0)}L${sx(data[0].i)},${sy(0)}Z` : "");
  const ticks = $derived([0, max / 2, max]);
  const xLabels = $derived(points.length > 2 ? [0, Math.floor((points.length - 1) / 2), points.length - 1] : points.map((_, i) => i));

  function onmove(e: PointerEvent) {
    if (!data.length) return;
    const rect = (e.currentTarget as SVGRectElement).getBoundingClientRect();
    const x = e.clientX - rect.left + L;
    let best = data[0];
    for (const p of data) if (Math.abs(sx(p.i) - x) < Math.abs(sx(best.i) - x)) best = p;
    hover = best.i;
  }
  const hovered = $derived(hover === null ? null : data.find((p) => p.i === hover) ?? null);
</script>

<div class="viz" bind:clientWidth={width}>
  <div class="plot">
    <svg {width} height={H} role="img" aria-label={name}>
      {#each ticks as t (t)}
        <line class="grid" x1={L} x2={L + plotW} y1={sy(t)} y2={sy(t)} />
        <text class="tick" x={L - 8} y={sy(t)} text-anchor="end" dominant-baseline="central">{t}{unit}</text>
      {/each}
      {#each xLabels as i (i)}
        <text class="tick" x={sx(i)} y={H - 8} text-anchor={i === 0 ? "start" : i === points.length - 1 ? "end" : "middle"}>{points[i]?.label}</text>
      {/each}

      {#if area}<path d={area} class="area" />{/if}
      {#if data.length > 1}<path d={line} class="line" />{/if}

      {#if hovered}
        <line class="cross" x1={sx(hovered.i)} x2={sx(hovered.i)} y1={T} y2={T + plotH} />
      {/if}
      {#each data as p (p.i)}
        <circle cx={sx(p.i)} cy={sy(p.value)} r={hovered?.i === p.i ? 5 : 4} class="dot" />
      {/each}

      <rect
        class="hit"
        x={L}
        y={T}
        width={plotW}
        height={plotH}
        role="presentation"
        onpointermove={onmove}
        onpointerleave={() => (hover = null)}
      />
    </svg>

    {#if hovered}
      <div class="tip" style:left="{Math.min(Math.max(sx(hovered.i), 70), width - 70)}px" style:top="{sy(hovered.value) - 12}px">
        <div class="tip-title">{hovered.label}</div>
        <div class="tip-row"><i></i><b>{Math.round(hovered.value)}{unit}</b> {name.toLowerCase()}</div>
      </div>
    {/if}
  </div>
</div>

<style>
  .viz { width: 100%; font-family: system-ui, -apple-system, "Segoe UI", sans-serif; }
  .plot { position: relative; }
  svg { display: block; overflow: visible; }
  .grid { stroke: var(--grid); stroke-width: 1; }
  .tick { fill: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
  .area { fill: var(--series-1); opacity: 0.1; }
  .line { fill: none; stroke: var(--series-1); stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
  .dot { fill: var(--series-1); stroke: var(--surface-1); stroke-width: 2; transition: r 0.12s; }
  .cross { stroke: var(--axis); stroke-width: 1; }
  .hit { fill: transparent; }
  .tip {
    position: absolute;
    transform: translate(-50%, -100%);
    pointer-events: none;
    background: var(--card);
    border: 1px solid var(--line-2);
    border-radius: 10px;
    padding: 7px 11px;
    box-shadow: var(--shadow);
    font-size: 12.5px;
    color: var(--ink-2);
    white-space: nowrap;
  }
  .tip-title { color: var(--ink-3); font-size: 11.5px; margin-bottom: 2px; }
  .tip-row { display: flex; align-items: center; gap: 7px; }
  .tip-row i { width: 12px; height: 2px; border-radius: 1px; background: var(--series-1); }
  .tip-row b { color: var(--ink); }
</style>
