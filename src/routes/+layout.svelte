<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Topbar from '$lib/components/Topbar.svelte';
  import TaskDrawer from '$lib/components/TaskDrawer.svelte';
  import PrivacyGate from '$lib/components/PrivacyGate.svelte';
  import UpdateDialog from '$lib/components/UpdateDialog.svelte';
  import { backend } from '$lib/services/backend';
  import { appError, initialize, loading, refresh, saveSettings, snapshot } from '$lib/stores/app';
  import { availableUpdate, checkForUpdate } from '$lib/stores/distribution';

  let taskDrawerOpen = false;
  let acceptingPrivacy = false;
  let autoCheckStartedFor = '';
  const privacyVersion = '2026-07-14';

  $: acknowledgedVersion = $snapshot.settings.privacyAcknowledgedVersion ?? '';
  $: if (!$loading && !$appError && acknowledgedVersion === privacyVersion && autoCheckStartedFor !== acknowledgedVersion) {
    autoCheckStartedFor = acknowledgedVersion;
    void checkForUpdate(false);
  }

  function applySystemTheme(media: MediaQueryList) {
    document.documentElement.dataset.theme = media.matches ? 'dark' : 'light';
  }

  async function acceptPrivacy() {
    acceptingPrivacy = true;
    try {
      await saveSettings({ ...$snapshot.settings, privacyAcknowledgedVersion: privacyVersion });
    } finally {
      acceptingPrivacy = false;
    }
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
  <meta name="description" content="数据保存在本地的个人 AI 求职助手" />
</svelte:head>

{#if $loading}
  <div class="fixed inset-0 grid place-items-center bg-[var(--app-bg)]"><p class="text-sm body-muted">正在安全加载本地数据…</p></div>
{:else if $appError}
  <div class="fixed inset-0 grid place-items-center bg-[var(--app-bg)] p-6">
    <div class="panel max-w-xl p-6 text-sm">
      <h1 class="text-lg font-semibold">应用无法安全启动</h1>
      <p class="mt-3 leading-6 text-danger">{$appError}</p>
      <p class="mt-3 text-xs body-muted">为避免创建空数据库掩盖迁移问题，应用已停止继续加载。原数据不会被删除。</p>
      <div class="mt-5 flex justify-end"><button class="btn-primary" type="button" on:click={() => void refresh()}>重试</button></div>
    </div>
  </div>
{:else if acknowledgedVersion !== privacyVersion}
  <PrivacyGate accepting={acceptingPrivacy} onAccept={() => void acceptPrivacy()} onExit={() => void backend.exitApp()} />
{:else}
  <div class="app-shell">
    <Sidebar />
    <div class="flex min-w-0 flex-1 flex-col">
      <Topbar onTasks={() => taskDrawerOpen = true} />
      <main class="page-shell scrollbar-thin"><slot /></main>
    </div>
    <TaskDrawer bind:open={taskDrawerOpen} />
  </div>
  {#if $availableUpdate}
    <UpdateDialog update={$availableUpdate} onLater={() => availableUpdate.set(null)} />
  {/if}
{/if}
