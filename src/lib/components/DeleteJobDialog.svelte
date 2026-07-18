<script lang="ts">
  import { AlertTriangle, Trash2, X } from 'lucide-svelte';
  import { modalFocus } from '$lib/modal-focus';

  export let open = false;
  export let mode: 'single' | 'bulk' = 'single';
  export let jobTitle = '';
  export let company = '';
  export let count = 0;
  export let busy = false;
  export let onCancel: () => void;
  export let onConfirm: () => void;

  const close = () => {
    if (!busy) onCancel();
  };
</script>

{#if open}
  <button
    class="fixed inset-0 z-40 bg-black/30 backdrop-blur-sm"
    tabindex="-1"
    on:click={close}
    aria-label="关闭删除确认"
  ></button>
  <div
    class="fixed left-1/2 top-1/2 z-50 w-[460px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 panel p-6"
    role="dialog"
    aria-modal="true"
    aria-labelledby="delete-job-title"
    tabindex="-1"
    use:modalFocus={{ close, canClose: !busy, initialFocus: '[data-cancel-delete]' }}
  >
    <div class="flex items-start justify-between gap-4">
      <div class="flex min-w-0 gap-3">
        <span
          class="grid h-10 w-10 shrink-0 place-items-center rounded-xl text-danger"
          style="background: var(--danger-soft);"><AlertTriangle size={20} /></span
        >
        <div class="min-w-0">
          <h3 id="delete-job-title" class="text-xl font-semibold">
            {mode === 'single' ? '确认删除岗位' : '确认批量删除'}
          </h3>
          {#if mode === 'single'}
            <p class="mt-2 break-words text-sm leading-6 body-muted">
              将永久删除“<strong class="text-ink">{jobTitle} · {company}</strong>”。
            </p>
          {:else}
            <p class="mt-2 text-sm leading-6 body-muted">
              将永久删除当前筛选下的 <strong class="text-danger">{count}</strong> 个无原始详情岗位。
            </p>
          {/if}
        </div>
      </div>
      <button class="btn-icon shrink-0" aria-label="关闭" disabled={busy} on:click={close}
        ><X size={17} /></button
      >
    </div>
    <div
      class="mt-5 rounded-xl border px-4 py-3 text-xs leading-5 body-muted"
      style="border-color: color-mix(in srgb, var(--danger) 35%, var(--line)); background: var(--danger-soft);"
    >
      删除后无法撤销，相关的匹配结果、招呼语和岗位详情也会一并移除。
    </div>
    <div class="mt-6 flex justify-end gap-2">
      <button class="btn" data-cancel-delete disabled={busy} on:click={close}>取消</button>
      <button class="btn-danger" disabled={busy} on:click={onConfirm}
        ><Trash2 size={15} />{busy
          ? '正在删除…'
          : mode === 'single'
            ? '确认删除'
            : `确认删除 ${count} 个岗位`}</button
      >
    </div>
  </div>
{/if}
