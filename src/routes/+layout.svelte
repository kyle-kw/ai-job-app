<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Topbar from '$lib/components/Topbar.svelte';
  import TaskDrawer from '$lib/components/TaskDrawer.svelte';
  import { appError, clearAppError, initialize, refresh } from '$lib/stores/app';

  let taskDrawerOpen = false;

  function applySystemTheme(media: MediaQueryList) {
    document.documentElement.dataset.theme = media.matches ? 'dark' : 'light';
  }

  onMount(() => {
    document.documentElement.lang = 'zh-CN';
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const syncTheme = () => applySystemTheme(media);
    syncTheme();
    media.addEventListener('change', syncTheme);
    void initialize();
    return () => media.removeEventListener('change', syncTheme);
  });
</script>

<svelte:head>
  <title>求职舱 · 本地 AI 求职助手</title>
  <meta name="description" content="数据保存在本机的个人 AI 求职助手" />
</svelte:head>

<div class="app-shell">
  <Sidebar />
  <div class="flex min-w-0 flex-1 flex-col">
    <Topbar onTasks={() => taskDrawerOpen = true} />
    {#if $appError}
      <div class="mx-5 mt-4 flex items-center justify-between gap-4 rounded-xl border px-4 py-3 text-sm text-danger" style="border-color: var(--danger); background: var(--danger-soft);" role="alert">
        <span>应用数据加载失败：{$appError}</span>
        <span class="flex shrink-0 gap-2"><button class="btn" type="button" on:click={() => void refresh()}>重试</button><button class="btn-ghost" type="button" on:click={clearAppError}>关闭</button></span>
      </div>
    {/if}
    <main class="page-shell scrollbar-thin"><slot /></main>
  </div>
  <TaskDrawer bind:open={taskDrawerOpen} />
</div>
