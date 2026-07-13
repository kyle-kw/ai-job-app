<script lang="ts">
  import { CheckCircle2, ChevronDown, ChevronRight, CircleDashed, Clock3, TerminalSquare, X, XCircle } from 'lucide-svelte';
  import { completedTasks, runningTasks, snapshot } from '$lib/stores/app';
  import AdvancedLog from './AdvancedLog.svelte';
  import { modalFocus } from '$lib/modal-focus';
  import type { TaskRun } from '$lib/types';

  export let open = false;
  let expanded: string | null = null;

  const iconFor = (task: TaskRun) => task.state === 'completed' ? CheckCircle2 : task.state === 'failed' ? XCircle : task.state === 'running' ? CircleDashed : Clock3;
  const labelFor = (task: TaskRun) => ({ scrape: '岗位抓取', 'job-detail-extraction': '岗位详情提取', 'resume-import': '简历解析', fit: '匹配分析', tailor: '专岗优化', render: 'PDF 渲染', 'provider-test': '模型测试', 'boss-login': 'BOSS 登录' }[task.kind]);
</script>

{#if open}
  <button class="fixed inset-0 z-40 bg-black/20 backdrop-blur-[1px]" tabindex="-1" on:click={() => open = false} aria-label="关闭任务中心"></button>
  <div class="fixed bottom-0 right-0 top-0 z-50 flex w-[430px] flex-col border-l bg-panel shadow-2xl" style="border-color: var(--line); animation: slide-in .22s ease-out;" role="dialog" aria-modal="true" aria-labelledby="task-drawer-title" tabindex="-1" use:modalFocus={{ close: () => open = false }}>
    <div class="flex h-[74px] items-center justify-between border-b px-5" style="border-color: var(--line);">
      <div>
        <h2 id="task-drawer-title" class="text-base font-semibold">任务中心</h2>
        <p class="mt-0.5 text-xs body-muted">切换页面不会中断；岗位抓取期间请勿关闭应用</p>
      </div>
      <button class="btn-icon" on:click={() => open = false} aria-label="关闭"><X size={18} /></button>
    </div>

    <div class="scrollbar-thin flex-1 overflow-y-auto px-5 py-5">
      {#if $runningTasks.length === 0 && $completedTasks.length === 0}
        <div class="grid min-h-72 place-items-center text-center">
          <div>
            <span class="mx-auto mb-3 grid h-12 w-12 place-items-center rounded-2xl surface-soft"><Clock3 size={21} class="body-muted" /></span>
            <p class="text-sm font-semibold">还没有任务</p>
            <p class="mt-1 text-xs body-muted">抓取岗位或导入简历后，进度会出现在这里。</p>
          </div>
        </div>
      {:else}
        {#if $runningTasks.length > 0}
          <p class="eyebrow mb-3">正在进行</p>
          <div class="space-y-3">
            {#each $runningTasks as task}
              <article class="panel-flat overflow-hidden p-4">
                <div class="flex items-start gap-3">
                  <span class="mt-0.5 text-brand"><svelte:component this={iconFor(task)} size={18} class={task.state === 'running' ? 'animate-spin' : ''} /></span>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center justify-between gap-3"><p class="truncate text-sm font-semibold">{task.title}</p><span class="text-xs font-semibold text-brand">{task.progress}%</span></div>
                    <p class="mt-1 text-xs body-muted">{task.message}</p>
                    {#if task.kind === 'scrape'}<p class="mt-2 text-[11px] font-medium text-warning">请勿关闭应用，切换页面不会中断抓取。</p>{/if}
                    <div class="mt-3 h-1.5 overflow-hidden rounded-full surface-soft"><div class="h-full rounded-full transition-all duration-500" style={`width:${task.progress}%; background: var(--brand);`}></div></div>
                  </div>
                </div>
              </article>
            {/each}
          </div>
        {/if}

        {#if $completedTasks.length > 0}
          <p class="eyebrow mb-3 mt-7">最近完成</p>
          <div class="space-y-2">
            {#each $completedTasks.slice(0, 8) as task}
              <article class="overflow-hidden rounded-xl border" style="border-color: var(--line);">
                <button class="flex w-full items-center gap-3 px-3.5 py-3 text-left hover:surface-soft" on:click={() => expanded = expanded === task.id ? null : task.id}>
                  <span style={`color:${task.state === 'failed' ? 'var(--danger)' : 'var(--success)'}`}><svelte:component this={iconFor(task)} size={17} /></span>
                  <div class="min-w-0 flex-1"><p class="truncate text-sm font-medium">{task.title}</p><p class="mt-0.5 text-[11px] body-muted">{labelFor(task)} · {task.message}</p></div>
                  {#if expanded === task.id}<ChevronDown size={15} class="body-muted" />{:else}<ChevronRight size={15} class="body-muted" />{/if}
                </button>
                {#if expanded === task.id}
                  <div class="border-t p-3" style="border-color: var(--line);">
                    {#if $snapshot.settings.advancedMode}
                      <div class="mb-2 flex items-center gap-2 text-xs font-semibold"><TerminalSquare size={14} />运行日志</div>
                      <AdvancedLog logs={task.logs} />
                    {:else}
                      <p class="text-xs body-muted">高级模式开启后可查看原始运行日志。</p>
                    {/if}
                  </div>
                {/if}
              </article>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}
