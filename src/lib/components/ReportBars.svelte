<script lang="ts">
  import type { ReportBucket } from '$lib/types';

  export let rows: ReportBucket[] = [];
  export let limit = 10;
  export let emptyText = '暂无可统计数据';

  $: visible = rows.slice(0, limit);
  $: maximum = Math.max(1, ...visible.map((item) => item.count));
</script>

{#if visible.length === 0}
  <div class="grid min-h-36 place-items-center rounded-xl border border-dashed text-sm body-muted" style="border-color: var(--line);">{emptyText}</div>
{:else}
  <div class="space-y-3">
    {#each visible as item}
      <div class="grid grid-cols-[minmax(110px,180px)_1fr_88px] items-center gap-3 text-xs">
        <span class="truncate font-medium" title={item.label}>{item.label}</span>
        <div class="h-2.5 overflow-hidden rounded-full surface-soft" role="img" aria-label={`${item.label}：${item.count} 个岗位，占 ${item.percentage}%`}>
          <div class="h-full rounded-full" style={`width:${Math.max(2, item.count / maximum * 100)}%; background: var(--brand);`}></div>
        </div>
        <span class="text-right tabular-nums body-muted"><strong class="font-semibold text-ink">{item.count}</strong> · {item.percentage.toFixed(1)}%</span>
      </div>
    {/each}
  </div>
{/if}
