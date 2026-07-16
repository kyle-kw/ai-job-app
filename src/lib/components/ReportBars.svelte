<script lang="ts">
  import type { ReportBucket } from '$lib/types';

  export let rows: ReportBucket[] = [];
  export let limit = 10;
  export let emptyText = '暂无可统计数据';
  export let hrefForRow: ((row: ReportBucket) => string | null) | null = null;

  $: visible = rows.slice(0, limit);
  $: maximum = Math.max(1, ...visible.map((item) => item.count));
</script>

{#if visible.length === 0}
  <div class="grid min-h-36 place-items-center rounded-xl border border-dashed text-sm body-muted" style="border-color: var(--line);">{emptyText}</div>
{:else}
  <div class="space-y-3">
    {#each visible as item}
      {@const href = hrefForRow?.(item) ?? null}
      {#if href}
      <a href={href} class="report-bar-row grid grid-cols-[minmax(110px,180px)_1fr_88px] items-center gap-3 rounded-lg text-xs focus:outline-none focus:ring-2 focus:ring-[var(--brand)] focus:ring-offset-2" aria-label={`查看 ${item.label} 的 ${item.count} 个岗位`}>
        <span class="truncate font-medium" title={item.label}>{item.label}</span>
        <div class="h-2.5 overflow-hidden rounded-full surface-soft" role="img" aria-label={`${item.label}：${item.count} 个岗位，占 ${item.percentage}%`}>
          <div class="h-full rounded-full" style={`width:${Math.max(2, item.count / maximum * 100)}%; background: var(--brand);`}></div>
        </div>
        <span class="text-right tabular-nums body-muted"><strong class="font-semibold text-ink">{item.count}</strong> · {item.percentage.toFixed(1)}%</span>
      </a>
      {:else}
        <div class="grid grid-cols-[minmax(110px,180px)_1fr_88px] items-center gap-3 text-xs">
          <span class="truncate font-medium" title={item.label}>{item.label}</span>
          <div class="h-2.5 overflow-hidden rounded-full surface-soft" role="img" aria-label={`${item.label}：${item.count} 个岗位，占 ${item.percentage}%`}>
            <div class="h-full rounded-full" style={`width:${Math.max(2, item.count / maximum * 100)}%; background: var(--brand);`}></div>
          </div>
          <span class="text-right tabular-nums body-muted"><strong class="font-semibold text-ink">{item.count}</strong> · {item.percentage.toFixed(1)}%</span>
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .report-bar-row { margin: -0.25rem; padding: 0.25rem; transition: background 140ms ease; }
  .report-bar-row:hover { background: var(--brand-faint); }
</style>
