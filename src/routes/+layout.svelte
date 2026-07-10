<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Topbar from '$lib/components/Topbar.svelte';
  import TaskDrawer from '$lib/components/TaskDrawer.svelte';
  import { initialize, snapshot } from '$lib/stores/app';

  let taskDrawerOpen = false;

  function applyTheme(theme: 'light' | 'dark' | 'system') {
    const resolved = theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : theme === 'system' ? 'light' : theme;
    document.documentElement.dataset.theme = resolved;
  }

  onMount(() => {
    void initialize();
  });

  $: if (typeof document !== 'undefined') applyTheme($snapshot.settings.theme);
</script>

<svelte:head>
  <title>求职舱 · AI 求职助手</title>
  <meta name="description" content="本地优先的 AI 求职助手" />
</svelte:head>

<div class="app-shell">
  <Sidebar />
  <div class="flex min-w-0 flex-1 flex-col">
    <Topbar onTasks={() => taskDrawerOpen = true} />
    <main class="page-shell scrollbar-thin"><slot /></main>
  </div>
  <TaskDrawer bind:open={taskDrawerOpen} />
</div>
