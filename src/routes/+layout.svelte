<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Topbar from '$lib/components/Topbar.svelte';
  import TaskDrawer from '$lib/components/TaskDrawer.svelte';
  import { initialize } from '$lib/stores/app';
  import { locale } from '$lib/i18n';

  let taskDrawerOpen = false;

  function applySystemTheme(media: MediaQueryList) {
    document.documentElement.dataset.theme = media.matches ? 'dark' : 'light';
  }

  onMount(() => {
    locale.set('zh-CN');
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
    <main class="page-shell scrollbar-thin"><slot /></main>
  </div>
  <TaskDrawer bind:open={taskDrawerOpen} />
</div>
