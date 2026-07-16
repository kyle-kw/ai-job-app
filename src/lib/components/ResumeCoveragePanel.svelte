<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { CheckCircle2, CircleHelp, LoaderCircle, SearchCheck, Sparkles, TriangleAlert, WandSparkles } from 'lucide-svelte';
  import type { ResumeCoverageItem, ResumeCoverageReport } from '$lib/types';

  export let report: ResumeCoverageReport | null = null;
  export let aiReady = false;
  export let analyzing = false;

  const dispatch = createEventDispatcher<{ analyze: void; select: { item: ResumeCoverageItem }; optimize: { item: ResumeCoverageItem } }>();
  const statusLabel = { covered: '已覆盖', strengthenable: '可强化', gap: '真实缺口', unknown: '待判断' } as const;
  const statusClass = { covered: 'text-brand', strengthenable: 'text-[#005cb8]', gap: 'text-danger', unknown: 'body-muted' } as const;
</script>

{#if !report}
  <div class="flex min-h-64 items-center justify-center gap-2 text-sm body-muted"><LoaderCircle size={16} class="animate-spin" />正在整理岗位要求…</div>
{:else if report.items.length === 0}
  <div class="grid min-h-64 place-items-center text-center"><div><SearchCheck size={28} class="mx-auto mb-3 body-muted" /><p class="font-semibold">岗位要求信息不足</p><p class="mt-1 text-xs body-muted">请先在岗位库提取完整岗位详情。</p></div></div>
{:else}
  <div class="animate-lift">
    <div class="grid grid-cols-4 gap-2">
      <div class="rounded-xl p-3 text-center" style="background: var(--brand-faint);"><strong class="block text-lg text-brand">{report.coveredCount}</strong><span class="text-[11px] body-muted">已覆盖</span></div>
      <div class="rounded-xl p-3 text-center" style="background: #eaf3fb;"><strong class="block text-lg text-[#005cb8]">{report.strengthenableCount}</strong><span class="text-[11px] body-muted">可强化</span></div>
      <div class="rounded-xl p-3 text-center" style="background: var(--danger-soft);"><strong class="block text-lg text-danger">{report.gapCount}</strong><span class="text-[11px] body-muted">真实缺口</span></div>
      <div class="rounded-xl p-3 text-center" style="background: var(--panel-soft);"><strong class="block text-lg">{report.unknownCount}</strong><span class="text-[11px] body-muted">待判断</span></div>
    </div>
    <div class="mt-4 flex items-center justify-between rounded-xl border p-3" style="border-color: var(--line);"><div><p class="text-xs font-semibold">{report.source === 'ai' ? 'AI 语义分析结果' : '本地精确匹配'}</p><p class="mt-0.5 text-[11px] body-muted">AI 仅在主动点击后调用，并且必须引用简历或事实证据。</p></div><button class="btn-primary" disabled={!aiReady || analyzing} title={aiReady ? '' : '请先配置并验证默认模型'} on:click={() => dispatch('analyze')}>{#if analyzing}<LoaderCircle size={14} class="animate-spin" />分析中…{:else}<Sparkles size={14} />AI 语义分析{/if}</button></div>
    <div class="mt-4 space-y-2">
      {#each report.items as item}
        <article class="rounded-xl border p-3" style="border-color: var(--line); background: var(--panel-soft);">
          <div class="flex items-start gap-3">
            <span class={`mt-0.5 ${statusClass[item.status]}`}>{#if item.status === 'covered'}<CheckCircle2 size={15} />{:else if item.status === 'gap'}<TriangleAlert size={15} />{:else}<CircleHelp size={15} />{/if}</span>
            <button class="min-w-0 flex-1 text-left" on:click={() => dispatch('select', { item })}><span class="block text-xs font-semibold leading-5">{item.label}</span><span class="mt-1 block text-[11px] leading-5 body-muted">{statusLabel[item.status]} · {item.rationale}</span></button>
            {#if item.status === 'strengthenable'}<button class="btn-ghost h-8 shrink-0 text-xs text-brand" on:click={() => dispatch('optimize', { item })}><WandSparkles size={13} />写入简历</button>{/if}
          </div>
        </article>
      {/each}
    </div>
  </div>
{/if}
