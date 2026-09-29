<script lang="ts">
  import { KINDS, KIND_INFO } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { compact, minutes, summarize } from "../lib/util";
  import Bars from "../components/Bars.svelte";
  import ChartCard from "../components/ChartCard.svelte";
  import LineChart from "../components/LineChart.svelte";
  import Segmented from "../components/Segmented.svelte";
  import StackedBars from "../components/StackedBars.svelte";
  import StatTile from "../components/StatTile.svelte";

  const snap = app.snap!;
  let range = $state("30");

  const list = $derived(
    snap.history.filter((d) => range === "all" || Date.now() - Date.parse(d.createdAt) < Number(range) * 864e5),
  );
  const totals = $derived(summarize(list));

  const byKind = $derived.by(() => {
    const rows = KINDS.map((k) => ({ label: KIND_INFO[k].label, a: 0, b: 0 }));
    for (const d of list)
      for (const e of d.edits) {
        const i = KINDS.indexOf(e.kind);
        if (i < 0) continue;
        if (e.applied) rows[i].a++;
        else rows[i].b++;
      }
    return rows.filter((r) => r.a + r.b > 0).sort((x, y) => y.a + y.b - (x.a + x.b));
  });

  // A day per point while the range is short, then weeks and months. Without this the chart
  // stopped at 90 points, so an imported year of history counted in the totals but vanished
  // off the timeline.
  const days = $derived.by(() => {
    const oldest = list.length ? Date.parse(list[list.length - 1].createdAt) : Date.now();
    const span = range === "all" ? Math.max(7, Math.ceil((Date.now() - oldest) / 864e5) + 1) : Number(range);
    const step = span <= 45 ? 1 : span <= 400 ? 7 : 30;
    const start = new Date();
    start.setHours(0, 0, 0, 0);
    start.setDate(start.getDate() - (span - 1));

    const out: { at: Date; sum: number; n: number }[] = [];
    for (let i = 0; i < Math.ceil(span / step); i++) {
      const at = new Date(start);
      at.setDate(at.getDate() + i * step);
      out.push({ at, sum: 0, n: 0 });
    }
    for (const d of list) {
      const i = Math.floor((Date.parse(d.createdAt) - start.getTime()) / (step * 864e5));
      if (i >= 0 && i < out.length) {
        out[i].sum += d.keptPct;
        out[i].n++;
      }
    }
    const fmt: Intl.DateTimeFormatOptions =
      step >= 30 ? { month: "short", year: "2-digit" } : { month: "short", day: "numeric" };
    return out.map((o) => ({ label: o.at.toLocaleDateString(undefined, fmt), value: o.n ? o.sum / o.n : null }));
  });

  const fillers = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const d of list)
      for (const e of d.edits) {
        if (e.kind !== "filler") continue;
        const w = e.original.toLowerCase().replace(/[^\p{L}\p{N}' ]/gu, "").trim();
        if (w) counts.set(w, (counts.get(w) ?? 0) + 1);
      }
    return [...counts].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([label, value]) => ({ label, value }));
  });
</script>

<div class="page-head">
  <h1>Insights</h1>
  <p>What Yap changes, what it only suggests, and how much of you makes it through.</p>
</div>

<div class="filters">
  <Segmented
    label="Date range"
    value={range}
    options={[
      { value: "7", label: "Last 7 days" },
      { value: "30", label: "Last 30 days" },
      { value: "all", label: "All time" },
    ]}
    onchange={(v) => (range = v)}
  />
</div>

{#if !list.length}
  <div class="card empty">
    <h3>No data for this range yet</h3>
    <p class="muted">Dictate a few things and this fills up with what Yap changed, what it held back, and how much still sounds like you.</p>
  </div>
{:else}
  <div class="tiles">
    <StatTile label="Dictations" value={compact(totals.count)} />
    <StatTile label="Words" value={compact(totals.words)} />
    <StatTile label="Time saved" value={minutes(totals.savedMin)} sub="vs typing at 40 wpm" />
    <StatTile label="Still sounds like you" value="{Math.round(totals.kept)}%" sub="average words kept" />
  </div>

  <div class="charts">
    <ChartCard
      title="What Yap changed vs. what it could have"
      sub="Each bar is every edit Yap found. Blue ones it made; orange ones it only suggested because your rules said so."
    >
      {#snippet chart()}
        {#if byKind.length}
          <StackedBars rows={byKind} names={["Changed", "Suggested"]} />
        {:else}
          <p class="muted small">No edits yet. Yap left everything exactly as you said it.</p>
        {/if}
      {/snippet}
      {#snippet table()}
        <table>
          <thead><tr><th>Kind</th><th class="num">Changed</th><th class="num">Suggested</th><th class="num">Total</th></tr></thead>
          <tbody>
            {#each byKind as r (r.label)}
              <tr><td>{r.label}</td><td class="num">{r.a}</td><td class="num">{r.b}</td><td class="num">{r.a + r.b}</td></tr>
            {/each}
          </tbody>
        </table>
      {/snippet}
    </ChartCard>

    <ChartCard title="How much still sounds like you" sub="Daily average share of your own words (ignoring um and uh) that made it into the final text.">
      {#snippet chart()}
        <LineChart points={days} name="Words kept" />
      {/snippet}
      {#snippet table()}
        <table>
          <thead><tr><th>Day</th><th class="num">Words kept</th></tr></thead>
          <tbody>
            {#each days.filter((d) => d.value !== null) as d (d.label)}
              <tr><td>{d.label}</td><td class="num">{Math.round(d.value ?? 0)}%</td></tr>
            {/each}
          </tbody>
        </table>
      {/snippet}
    </ChartCard>

    <ChartCard title="Your go-to fillers" sub="The padding words Yap caught most. Totally normal, everyone has theirs.">
      {#snippet chart()}
        {#if fillers.length}
          <Bars rows={fillers} name="Filler words" />
        {:else}
          <p class="muted small">No fillers caught yet. Smooth talker.</p>
        {/if}
      {/snippet}
      {#snippet table()}
        <table>
          <thead><tr><th>Filler</th><th class="num">Times</th></tr></thead>
          <tbody>
            {#each fillers as f (f.label)}<tr><td>{f.label}</td><td class="num">{f.value}</td></tr>{/each}
          </tbody>
        </table>
      {/snippet}
    </ChartCard>
  </div>
{/if}

<style>
  .filters { display: flex; gap: 10px; margin-bottom: 18px; }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  .charts { display: grid; gap: 14px; margin-top: 14px; }
  .empty { padding: 36px; text-align: center; display: grid; gap: 6px; justify-items: center; }
  .empty p { max-width: 52ch; }
  @media (max-width: 820px) {
    .tiles { grid-template-columns: repeat(2, 1fr); }
  }
</style>
