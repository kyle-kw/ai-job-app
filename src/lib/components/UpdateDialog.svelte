<script lang="ts">
  import { Download, RefreshCw } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import { runningTasks } from '$lib/stores/app';
  import type { AppUpdateInfo, UpdateEvent } from '$lib/types';

  export let update: AppUpdateInfo;
  export let onLater: () => void;

  let installing = false;
  let downloaded = 0;
  let total: number | null = update.downloadSize ?? null;
  let errorMessage = '';

  $: busy = $runningTasks.length > 0;
  $: progress = total && total > 0 ? Math.min(100, Math.round(downloaded / total * 100)) : 0;

  const sizeLabel = (bytes?: number | null) => bytes ? `${(bytes / 1024 / 1024).toFixed(1)} MiB` : '大小未知';

  async function install() {
    if (busy || installing) return;
    installing = true;
    errorMessage = '';
    try {
      await backend.downloadAndInstallUpdate((event: UpdateEvent) => {
        downloaded = event.downloaded || downloaded;
        total = event.total ?? total;
      });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
      installing = false;
    }
  }
</script>

<div class="fixed inset-0 z-[90] grid place-items-center bg-black/45 p-4" role="dialog" aria-modal="true" aria-labelledby="update-title">
  <section class="panel w-full max-w-[560px] overflow-hidden shadow-2xl">
    <header class="border-b px-6 py-5" style="border-color: var(--line);">
      <div class="flex items-start gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"><Download size={19} /></span><div><p class="eyebrow">UPDATE AVAILABLE</p><h2 id="update-title" class="mt-1 text-lg font-semibold">求职舱 {update.version}</h2><p class="mt-1 text-xs body-muted">当前 {update.currentVersion} · 完整包 {sizeLabel(update.downloadSize)}</p></div></div>
    </header>
    <div class="px-6 py-5">
      <h3 class="text-sm font-semibold">更新说明</h3>
      <div class="mt-2 max-h-48 overflow-y-auto whitespace-pre-wrap rounded-xl border p-4 text-sm leading-6 body-muted" style="border-color: var(--line); background: var(--panel-soft);">{update.notes || '本版本未提供更新说明。'}</div>
      {#if installing || downloaded > 0}
        <div class="mt-4"><div class="mb-1.5 flex justify-between text-xs body-muted"><span>正在下载并验证签名</span><span>{total ? `${progress}%` : sizeLabel(downloaded)}</span></div><div class="h-2 overflow-hidden rounded-full bg-[var(--line)]"><div class="h-full bg-brand transition-all" style={`width:${total ? progress : 35}%`}></div></div></div>
      {/if}
      {#if busy}<p class="mt-4 rounded-xl border p-3 text-xs text-warning" style="border-color: var(--warning); background: var(--warning-soft);">当前有任务正在运行。任务结束后更新，避免中断抓取或 AI 分析。</p>{/if}
      {#if errorMessage}<div class="mt-4 rounded-xl border p-3 text-xs text-danger" style="border-color: var(--danger); background: var(--danger-soft);">{errorMessage}<br /><a class="underline" href={`https://github.com/kyle-kw/ai-job-app/releases/tag/v${update.version}`} target="_blank" rel="noreferrer">前往 GitHub Release 手动下载</a></div>{/if}
    </div>
    <footer class="flex justify-end gap-2 border-t px-6 py-4" style="border-color: var(--line);"><button class="btn" type="button" on:click={onLater} disabled={installing}>稍后</button><button class="btn-primary" type="button" on:click={install} disabled={busy || installing}><RefreshCw size={15} class={installing ? 'animate-spin' : ''} />{busy ? '任务结束后更新' : installing ? '正在更新…' : '下载并安装'}</button></footer>
  </section>
</div>
