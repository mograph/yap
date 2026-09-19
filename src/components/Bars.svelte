<script lang="ts">
  type Row = { label: string; value: number };
  let { rows, name }: { rows: Row[]; name: string } = $props();

  const LABEL = 110;
  const END = 40;
  const ROW = 32;
  const BAR = 16;

  let width = $state(500);
  let hover = $state<number | null>(null);

  const max = $derived(Math.max(1, ...rows.map((r) => r.value)));
  const plot = $derived(Math.max(60, width - LABEL - END));
  const sx = (v: number) => (v / max) * plot;

  function bar(x: number, y: number, w: number, h: number) {
    if (w <= 0.5) return "";
    const r = Math.min(4, w, h / 2);
    return `M${x},${y}h${w - r}a${r},${r} 0 0 1 ${r},${r}v${h - 2 * r}a${r},${r} 0 0 1 ${-r},${r}h${-(w - r)}z`;
  }
</script>

<div class="viz" bind:clientWidth={width}>
  <svg {width} height={rows.length * ROW} role="img" aria-label={name}>
    <line class="base" x1={LABEL} x2={LABEL} y1={0} y2={rows.length * ROW} />
    {#each rows as row, i (row.label)}
      {@const y = i * ROW + (ROW - BAR) / 2}
      <g class:dim={hover !== null && hover !== i}>
        <text class="cat" x={LABEL - 12} y={y + BAR / 2} text-anchor="end" dominant-baseline="central">“{row.label}”</text>
        <path d={bar(LABEL, y, sx(row.value), BAR)} fill="var(--series-1)" />
        <text class="val" x={LABEL + sx(row.value) + 8} y={y + BAR / 2} dominant-baseline="central">{row.value}×</text>
        <rect
          class="hit"
          x={0}
          y={i * ROW}
          width={LABEL + plot + END}
          height={ROW}
          tabindex="0"
          role="button"
          aria-label="{row.label}: {row.value} times"
          onpointerenter={() => (hover = i)}
          onpointerleave={() => (hover = null)}
          onfocus={() => (hover = i)}
          onblur={() => (hover = null)}
        />
      </g>
    {/each}
  </svg>
</div>

<style>
  .viz { width: 100%; font-family: system-ui, -apple-system, "Segoe UI", sans-serif; }
  svg { display: block; overflow: visible; }
  .base { stroke: var(--axis); stroke-width: 1; }
  .cat { fill: var(--ink-2); font-size: 12.5px; }
  .val { fill: var(--ink); font-size: 12px; font-weight: 600; font-variant-numeric: tabular-nums; }
  g { transition: opacity 0.15s; }
  g.dim { opacity: 0.45; }
  .hit { fill: transparent; outline: none; }
</style>
