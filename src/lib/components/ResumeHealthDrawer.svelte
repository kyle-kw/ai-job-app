<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { AlertCircle, CheckCircle2, Lightbulb, Sparkles, TriangleAlert, X } from 'lucide-svelte';
  import { modalFocus } from '$lib/modal-focus';
  import type { ResumeHealthIssue, ResumeHealthReport } from '$lib/types';

  export let open = false;
  export let report: ResumeHealthReport;
  export let aiReady = false;

  const dispatch = createEventDispatcher<{
    select: { issue: ResumeHealthIssue };
    ai: { issues: ResumeHealthIssue[] };
  }>();

  const labels = { error: '严重', warning: '警告', suggestion: '建议' } as const;
  const groups: Array<ResumeHealthIssue['severity']> = ['error', 'warning', 'suggestion'];
  let selectedIds = new Set<string>();
  let reportKey = '';
  $: total = report.issues.length;
  $: {
    const key = report.issues.map((item) => item.id).join('|');
    if (key !== reportKey) {
      selectedIds = new Set(report.issues.map((item) => item.id));
      reportKey = key;
    }
  }

  function close() {
    open = false;
  }
  function toggleIssue(id: string, checked: boolean) {
    const next = new Set(selectedIds);
    if (checked) next.add(id);
    else next.delete(id);
    selectedIds = next;
  }
</script>

{#if open}
  <button
    class="fixed inset-0 z-[70] bg-black/25 backdrop-blur-[1px]"
    tabindex="-1"
    aria-label="关闭简历体检"
    on:click={close}
  ></button>
  <div
    class="fixed bottom-0 right-0 top-0 z-[80] flex w-[min(520px,calc(100vw-28px))] flex-col border-l bg-panel shadow-2xl"
    style="border-color: var(--line); animation: slide-in .22s ease-out;"
    role="dialog"
    aria-modal="true"
    aria-labelledby="resume-health-title"
    tabindex="-1"
    use:modalFocus={{ close }}
  >
    <header
      class="flex h-[74px] shrink-0 items-center justify-between border-b px-6"
      style="border-color: var(--line);"
    >
      <div>
        <h2 id="resume-health-title" class="text-base font-semibold">简历体检</h2>
        <p class="mt-0.5 text-xs body-muted">完全在本地运行，不会发送简历内容。</p>
      </div>
      <button class="btn-icon" aria-label="关闭" on:click={close}><X size={18} /></button>
    </header>

    <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6">
      {#if total === 0}
        <div class="grid min-h-72 place-items-center text-center">
          <div>
            <CheckCircle2 size={32} class="mx-auto mb-3 text-brand" />
            <p class="font-semibold">没有发现明显问题</p>
            <p class="mt-1 text-xs body-muted">仍建议针对目标岗位检查内容相关性。</p>
          </div>
        </div>
      {:else}
        <div class="mb-6 grid grid-cols-3 gap-3">
          <div class="rounded-xl p-3 text-center" style="background: var(--danger-soft);">
            <strong class="block text-lg text-danger">{report.errorCount}</strong><span
              class="text-xs body-muted">严重</span
            >
          </div>
          <div class="rounded-xl p-3 text-center" style="background: var(--warning-soft);">
            <strong class="block text-lg text-warning">{report.warningCount}</strong><span
              class="text-xs body-muted">警告</span
            >
          </div>
          <div class="rounded-xl p-3 text-center" style="background: var(--brand-faint);">
            <strong class="block text-lg text-brand">{report.suggestionCount}</strong><span
              class="text-xs body-muted">建议</span
            >
          </div>
        </div>
        {#each groups as severity}
          {@const items = report.issues.filter((item) => item.severity === severity)}
          {#if items.length}
            <section class="mb-6">
              <h3 class="mb-2 flex items-center gap-2 text-sm font-semibold">
                {#if severity === 'error'}<AlertCircle
                    size={15}
                    class="text-danger"
                  />{:else if severity === 'warning'}<TriangleAlert
                    size={15}
                    class="text-warning"
                  />{:else}<Lightbulb size={15} class="text-brand" />{/if}
                {labels[severity]} · {items.length}
              </h3>
              <div class="space-y-2">
                {#each items as item}
                  <div
                    class="flex items-start gap-3 rounded-xl border p-3 transition hover:-translate-y-0.5 hover:shadow-sm"
                    style="border-color: var(--line); background: var(--panel-soft);"
                  >
                    <input
                      class="mt-0.5 h-4 w-4 accent-[var(--brand)]"
                      type="checkbox"
                      checked={selectedIds.has(item.id)}
                      aria-label={`选择体检问题：${item.label}`}
                      on:change={(event) => toggleIssue(item.id, event.currentTarget.checked)}
                    />
                    <button
                      class="min-w-0 flex-1 text-left"
                      on:click={() => dispatch('select', { issue: item })}
                      ><span class="block text-xs font-semibold">{item.label}</span><span
                        class="mt-1 block text-xs leading-5 body-muted">{item.message}</span
                      ></button
                    >
                  </div>
                {/each}
              </div>
            </section>
          {/if}
        {/each}
      {/if}
    </div>

    {#if total}
      <footer
        class="flex shrink-0 items-center justify-between gap-4 border-t px-6 py-4"
        style="border-color: var(--line);"
      >
        <p class="text-[11px] body-muted">已选择 {selectedIds.size} 项；AI 只会生成待审核修改。</p>
        <button
          class="btn-primary shrink-0"
          disabled={!aiReady || selectedIds.size === 0}
          title={aiReady ? '' : '请先配置并验证默认模型'}
          on:click={() =>
            dispatch('ai', { issues: report.issues.filter((item) => selectedIds.has(item.id)) })}
          ><Sparkles size={15} />请 AI 优化</button
        >
      </footer>
    {/if}
  </div>
{/if}
