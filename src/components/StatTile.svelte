<script lang="ts">
  let { label, value, sub }: { label: string; value: string; sub?: string } = $props();
</script>

<div class="tile card">
  <span class="label">{label}</span>
  <span class="value">{value}</span>
  {#if sub}<span class="sub">{sub}</span>{/if}
</div>

<style>
  .tile { padding: 16px 18px; display: grid; gap: 2px; align-content: start; }
  .label { font-size: 12.5px; font-weight: 560; color: var(--ink-2); margin: 0; }
  .value { font: 650 28px/1.15 var(--font); letter-spacing: -0.02em; color: var(--ink); }
  .sub { font-size: 12px; color: var(--ink-3); }
</style>
