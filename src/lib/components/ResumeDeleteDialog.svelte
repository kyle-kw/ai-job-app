<script lang="ts">
  import { AlertTriangle, Trash2, X } from 'lucide-svelte';
  import { modalFocus } from '$lib/modal-focus';

  export let open = false;
  export let itemLabel = '';
  export let onCancel: () => void;
  export let onConfirm: () => void;
</script>

{#if open}
  <button
    class="fixed inset-0 z-[90] bg-black/30 backdrop-blur-sm"
    tabindex="-1"
    on:click={onCancel}
    aria-label="关闭简历删除确认"
  ></button>
  <div
    class="fixed left-1/2 top-1/2 z-[91] w-[480px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 panel p-6"
    role="dialog"
    aria-modal="true"
    aria-labelledby="resume-delete-title"
    tabindex="-1"
    use:modalFocus={{ close: onCancel, initialFocus: '[data-cancel-resume-delete]' }}
  >
    <div class="flex items-start justify-between gap-4">
      <div class="flex min-w-0 gap-3">
        <span
          class="grid h-10 w-10 shrink-0 place-items-center rounded-xl text-danger"
          style="background: var(--danger-soft);"><AlertTriangle size={20} /></span
        >
        <div class="min-w-0">
          <p class="eyebrow">简历编辑</p>
          <h3 id="resume-delete-title" class="mt-1 text-xl font-semibold">确认删除简历内容</h3>
          <p class="mt-2 break-words text-sm leading-6 body-muted">
            将删除 <strong class="text-ink">{itemLabel}</strong>。
          </p>
        </div>
      </div>
      <button class="btn-icon shrink-0" aria-label="关闭" on:click={onCancel}
        ><X size={17} /></button
      >
    </div>
    <div
      class="mt-5 rounded-xl border px-4 py-3 text-xs leading-5 body-muted"
      style="border-color: color-mix(in srgb, var(--danger) 35%, var(--line)); background: var(--danger-soft);"
    >
      删除后无法撤销。点击页面顶部的“保存修改”后，这项删除才会写入主简历。
    </div>
    <div class="mt-6 flex justify-end gap-2">
      <button class="btn" data-cancel-resume-delete on:click={onCancel}>取消</button>
      <button class="btn-danger" data-confirm-resume-delete on:click={onConfirm}
        ><Trash2 size={15} />确认删除</button
      >
    </div>
  </div>
{/if}
