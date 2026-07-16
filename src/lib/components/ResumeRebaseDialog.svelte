<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { LoaderCircle, RefreshCw, X } from 'lucide-svelte';
  import { modalFocus } from '$lib/modal-focus';
  import type { ResumeRebasePreview, ResumeRebaseResolution } from '$lib/types';

  export let open = false;
  export let preview: ResumeRebasePreview | null = null;
  export let applying = false;

  const dispatch = createEventDispatcher<{ apply: { resolutions: ResumeRebaseResolution[] } }>();
  let choices: Record<string, 'variant' | 'master'> = {};
  let lastPreviewKey = '';

  $: if (preview) {
    const key = `${preview.variantId}:${preview.variantVersion}:${preview.masterVersion}`;
    if (key !== lastPreviewKey) {
      choices = Object.fromEntries(preview.conflicts.map((item) => [item.path, 'variant']));
      lastPreviewKey = key;
    }
  }

  function close() { if (!applying) open = false; }
  function format(value: unknown) {
    if (typeof value === 'string') return value || '（空）';
    if (Array.isArray(value)) return `${value.length} 条记录`;
    return JSON.stringify(value, null, 2);
  }
</script>

{#if open}
  <button class="fixed inset-0 z-[80] bg-black/35 backdrop-blur-sm" tabindex="-1" aria-label="关闭同步主简历" on:click={close}></button>
  <div class="fixed left-1/2 top-1/2 z-[81] flex max-h-[86vh] w-[min(820px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 flex-col panel" role="dialog" aria-modal="true" aria-labelledby="resume-rebase-title" tabindex="-1" use:modalFocus={{ close, canClose: !applying }}>
    <header class="flex items-start justify-between gap-4 border-b p-6" style="border-color: var(--line);"><div><p class="eyebrow">SYNC MASTER</p><h2 id="resume-rebase-title" class="mt-1 text-xl font-semibold">同步主简历更新</h2><p class="mt-1 text-xs body-muted">未修改字段自动同步；双方都修改的章节由你决定。</p></div><button class="btn-icon" disabled={applying} aria-label="关闭" on:click={close}><X size={18} /></button></header>
    <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6">
      {#if !preview}
        <div class="flex min-h-48 items-center justify-center gap-2 text-sm body-muted"><LoaderCircle size={16} class="animate-spin" />正在比较版本…</div>
      {:else}
        {#if preview.autoChanges.length}
          <section class="mb-6"><h3 class="text-sm font-semibold">将自动同步 · {preview.autoChanges.length}</h3><div class="mt-2 flex flex-wrap gap-2">{#each preview.autoChanges as change}<span class="chip-brand">{change.label}</span>{/each}</div></section>
        {/if}
        {#if preview.conflicts.length}
          <section><h3 class="text-sm font-semibold">需要选择 · {preview.conflicts.length}</h3><div class="mt-3 space-y-4">{#each preview.conflicts as conflict}<article class="rounded-xl border p-4" style="border-color: var(--line);"><p class="text-sm font-semibold">{conflict.label}</p><div class="mt-3 grid grid-cols-2 gap-3"><label class:selected-choice={choices[conflict.path] === 'variant'} class="cursor-pointer rounded-xl border p-3" style="border-color: var(--line);"><span class="flex items-center gap-2 text-xs font-semibold"><input type="radio" name={conflict.path} value="variant" bind:group={choices[conflict.path]} />保留岗位版本</span><span class="mt-2 block max-h-24 overflow-auto whitespace-pre-wrap text-[11px] body-muted">{format(conflict.variant)}</span></label><label class:selected-choice={choices[conflict.path] === 'master'} class="cursor-pointer rounded-xl border p-3" style="border-color: var(--line);"><span class="flex items-center gap-2 text-xs font-semibold"><input type="radio" name={conflict.path} value="master" bind:group={choices[conflict.path]} />采用主简历 v{preview.masterVersion}</span><span class="mt-2 block max-h-24 overflow-auto whitespace-pre-wrap text-[11px] body-muted">{format(conflict.master)}</span></label></div></article>{/each}</div></section>
        {:else if preview.autoChanges.length === 0}
          <div class="grid min-h-44 place-items-center text-center"><div><p class="font-semibold">内容已经同步</p><p class="mt-1 text-xs body-muted">只会刷新事实、偏好与基线版本。</p></div></div>
        {/if}
      {/if}
    </div>
    <footer class="flex justify-end gap-2 border-t p-5" style="border-color: var(--line);"><button class="btn" disabled={applying} on:click={close}>取消</button><button class="btn-primary" disabled={!preview || applying} on:click={() => preview && dispatch('apply', { resolutions: preview.conflicts.map((item) => ({ path: item.path, choice: choices[item.path] ?? 'variant' })) })}>{#if applying}<LoaderCircle size={15} class="animate-spin" />正在同步…{:else}<RefreshCw size={15} />同步并创建版本{/if}</button></footer>
  </div>
{/if}

<style>
  .selected-choice { border-color: var(--brand) !important; background: var(--brand-faint); }
</style>
