<script lang="ts">
  import { Activity, Bell, PanelRightOpen } from 'lucide-svelte';
  import { page } from '$app/stores';
  import { runningTasks } from '$lib/stores/app';

  export let onTasks: () => void;
  const titles: Record<string, { title: string; subtitle: string }> = {
    '/': { title: '工作台', subtitle: '今天，从最值得做的下一步开始' },
    '/jobs': { title: '岗位库', subtitle: '抓取、筛选并理解每一个机会' },
    '/resume': { title: '我的简历', subtitle: '维护一份可信、可复用的职业事实库' },
    '/settings': { title: '设置', subtitle: '连接模型，并掌控数据与高级能力' }
  };
  $: current = titles[$page.url.pathname] ?? titles['/'];
</script>

<header class="sticky top-0 z-20 flex h-[74px] items-center justify-between border-b px-7 backdrop-blur-xl" style="border-color: var(--line); background: color-mix(in srgb, var(--canvas) 88%, transparent);">
  <div>
    <h1 class="text-[19px] font-semibold tracking-[-0.025em]">{current.title}</h1>
    <p class="mt-0.5 text-xs body-muted">{current.subtitle}</p>
  </div>
  <div class="flex items-center gap-2">
    <button class="task-button flex h-10 items-center gap-2 rounded-xl border px-3 text-sm font-medium transition" on:click={onTasks} aria-label="打开任务中心">
      {#if $runningTasks.length > 0}
        <Activity size={17} class="animate-pulse" />
        <span>{$runningTasks.length} 个任务运行中</span>
        <span class="h-2 w-2 rounded-full" style="background: var(--brand);"></span>
      {:else}
        <PanelRightOpen size={17} />
        <span>任务中心</span>
      {/if}
    </button>
    <button class="btn-icon" aria-label="通知"><Bell size={17} /></button>
    <div class="ml-1 grid h-9 w-9 place-items-center rounded-full text-xs font-semibold" style="background: var(--brand-soft); color: var(--brand);">L</div>
  </div>
</header>

<style>
  .task-button { border-color: var(--line); background: var(--panel); }
  .task-button:hover { border-color: var(--brand); color: var(--brand); }
</style>
