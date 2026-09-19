<script lang="ts">
  import { niceTicks } from "../lib/util";

  type Row = { label: string; a: number; b: number };
  let { rows, names }: { rows: Row[]; names: [string, string] } = $props();

  const LABEL = 136;
  const END = 44;
  const ROW = 36;
  const BAR = 18;
  const GAP = 2;
  const AXIS = 24;

  let width = $state(600);
  let hover = $state<number | null>(null);

  const ticks = $derived(niceTicks(Math.max(1, ...rows.map((r) => r.a + r.b)), 4));
  const max = $derived(ticks[ticks.length - 1]);
  const plot = $derived(Math.max(80, width - LABEL - END));
  const height = $derived(rows.length * ROW + AXIS);
  const sx = (v: number) => (v / max) * plot;

  /** Horizontal bar, square at its left, optionally rounded (4px) at its data end. */
  function bar(x: number, y: number, w: number, h: number, rounded: boolean) {
    if (w <= 0.5) return "";
    const r = rounded ? Math.min(4, w, h / 2) : 0;
    return `M${x},${y}h${w - r}a${r},${r} 0 0 1 ${r},${r}v${h - 2 * r}a${r},${r} 0 0 1 ${-r},${r}h${-(w - r)}z`;
  }
</script>

<div class="viz" bind:clientWidth={width}>
  <div class="legend">
    <span><i style:background="var(--series-1)"></i>{names[0]}</span>
    <span><i style:background="var(--series-2)"></i>{names[1]}</span>
  </div>

  <div class="plot">
    <svg {width} {height} role="img" aria-label="{names[0]} and {names[1]} by kind of change">
      {#each ticks as t (t)}
        <line class="grid" x1={LABEL + sx(t)} x2={LABEL + sx(t)} y1={0} y2={rows.length * ROW} />
        <text class="tick" x={LABEL + sx(t)} y={height - 6} text-anchor="middle">{t}</text>
      {/each}
      <line class="base" x1={LABEL} x2={LABEL} y1={0} y2={rows.length * ROW} />

      {#each rows as row, i (row.label)}
        {@const y = i * ROW + (ROW - BAR) / 2}
        {@const wa = sx(row.a)}
        {@const wb = sx(row.b)}
        {@const both = row.a > 0 && row.b > 0}
        <g class="row" class:dim={hover !== null && hover !== i}>
          <text class="cat" x={LABEL - 12} y={y + BAR / 2} text-anchor="end" dominant-baseline="central">{row.label}</text>
          <path d={bar(LABEL, y, both ? wa - GAP : wa, BAR, row.b === 0)} fill="var(--series-1)" />
          <path d={bar(LABEL + wa, y, wb, BAR, true)} fill="var(--series-2)" />
          <text class="val" x={LABEL + wa + wb + 8} y={y + BAR / 2} dominant-baseline="central">{row.a + row.b}</text>
          <rect
            class="hit"
            x={0}
            y={i * ROW}
            width={LABEL + plot + END}
            height={ROW}
            tabindex="0"
            role="button"
            aria-label="{row.label}: {row.a} {names[0].toLowerCase()}, {row.b} {names[1].toLowerCase()}"
            onpointerenter={() => (hover = i)}
            onpointerleave={() => (hover = null)}
            onfocus={() => (hover = i)}
            onblur={() => (hover = null)}
          />
        </g>
      {/each}
    </svg>

    {#if hover !== null && rows[hover]}
      {@const row = rows[hover]}
      <!-- Above the bar, except the top row, where above would cover the card's header. -->
      <div
        class="tip"
        class:below={hover === 0}
        style:left="{Math.min(Math.max(LABEL + sx(row.a + row.b) / 2, LABEL + 70), width - 80)}px"
        style:top="{hover === 0 ? ROW - (ROW - BAR) / 2 + 6 : hover * ROW + (ROW - BAR) / 2 - 6}px"
      >
        <div class="tip-title">{row.label}</div>
        <div class="tip-row"><i style:background="var(--series-1)"></i><b>{row.a}</b> {names[0].toLowerCase()}</div>
        <div class="tip-row"><i style:background="var(--series-2)"></i><b>{row.b}</b> {names[1].toLowerCase()}</div>
      </div>
    {/if}
  </div>
</div>

<style>
  .viz { width: 100%; font-family: system-ui, -apple-system, "Segoe UI", sans-serif; }
  .legend { display: flex; gap: 16px; font-size: 12.5px; color: var(--ink-2); margin-bottom: 10px; }
  .legend span { display: inline-flex; align-items: center; gap: 6px; }
  .legend i { width: 10px; height: 10px; border-radius: 2px; }
  .plot { position: relative; }
  svg { display: block; overflow: visible; }
  .grid { stroke: var(--grid); stroke-width: 1; }
  .base { stroke: var(--axis); stroke-width: 1; }
  .tick { fill: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
  .cat { fill: var(--ink-2); font-size: 12.5px; }
  .val { fill: var(--ink); font-size: 12px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .row { transition: opacity 0.15s; }
  .row.dim { opacity: 0.45; }
  .hit { fill: transparent; outline: none; cursor: default; }
  .tip {
    position: absolute;
    transform: translate(-50%, -100%);
    pointer-events: none;
    background: var(--card);
    border: 1px solid var(--line-2);
    border-radius: 10px;
    padding: 8px 11px;
    box-shadow: var(--shadow);
    font-size: 12.5px;
    color: var(--ink-2);
    white-space: nowrap;
    z-index: 2;
  }
  .tip.below { transform: translate(-50%, 0); }
  .tip-title { color: var(--ink-3); font-size: 11.5px; margin-bottom: 3px; }
  .tip-row { display: flex; align-items: center; gap: 7px; }
  .tip-row i { width: 12px; height: 2px; border-radius: 1px; }
  .tip-row b { color: var(--ink); font-variant-numeric: tabular-nums; }
</style>
