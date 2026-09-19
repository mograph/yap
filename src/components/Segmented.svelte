<script lang="ts">
  let {
    value,
    options,
    onchange,
    size = "md",
    label,
  }: {
    value: string;
    options: { value: string; label: string }[];
    onchange: (v: string) => void;
    size?: "sm" | "md";
    label?: string;
  } = $props();
</script>

<div class="seg {size}" role="radiogroup" aria-label={label}>
  {#each options as o (o.value)}
    <button type="button" role="radio" aria-checked={value === o.value} class:on={value === o.value} onclick={() => onchange(o.value)}>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: 999px;
    background: var(--card-2);
    border: 1px solid var(--line);
  }
  button {
    height: 30px;
    padding: 0 14px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--ink-2);
    font-weight: 560;
    white-space: nowrap;
    transition: background 0.18s, color 0.18s, box-shadow 0.18s;
  }
  button:hover { color: var(--ink); }
  button.on { background: var(--card); color: var(--ink); box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12), inset 0 0 0 1px var(--line); }
  .sm button { height: 26px; padding: 0 11px; font-size: 12.5px; }
</style>
