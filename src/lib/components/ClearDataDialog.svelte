<script lang="ts">
  import { AlertTriangle, Trash2, X } from 'lucide-svelte';
  import { modalFocus } from '$lib/modal-focus';
  import type { ClearDataScope } from '$lib/types';

  export let open = false;
  export let scope: ClearDataScope = 'modelKeys';
  export let busy = false;
  export let error = '';
  export let onCancel: () => void;
  export let onConfirm: () => void;

  const configurations: Record<
    ClearDataScope,
    {
      title: string;
      description: string;
      warning: string;
      confirmLabel: string;
    }
  > = {
    modelKeys: {
      title: '确认清除模型密钥',
      description: '将删除新旧系统钥匙串中的模型 API Key，并清除模型配置中的密钥引用。',
      warning: '清除后，所有模型都会标记为未验证；再次使用 AI 功能前需要重新填写并验证密钥。',
      confirmLabel: '确认清除'
    },
    bossProfile: {
      title: '确认清除 BOSS 登录数据',
      description: '将关闭 BOSS 专用 Chrome，并删除它的 Profile、抓取结果和本地登录状态。',
      warning: '清除后无法恢复当前登录状态；再次使用 BOSS 功能时需要重新登录和配置。',
      confirmLabel: '确认清除'
    },
    legacyData: {
      title: '确认删除旧版遗留数据',
      description: '将删除旧标识目录和旧钥匙串中的模型密钥；当前版本的数据不会被删除。',
      warning: '删除后无法再用这份旧版数据回退，请确认当前版本中的数据已经完整可用。',
      confirmLabel: '确认删除'
    },
    all: {
      title: '确认清除全部应用数据',
      description: '将清除模型密钥、BOSS Profile、数据库、自动备份、日志和导入临时文件。',
      warning: '此操作无法撤销。自行导出的 PDF、报告和 .aijobbackup 不受影响；完成后应用需要重启。',
      confirmLabel: '确认全部清除'
    }
  };

  $: configuration = configurations[scope];
  const close = () => {
    if (!busy) onCancel();
  };
</script>

{#if open}
  <button
    class="fixed inset-0 z-[90] bg-black/30 backdrop-blur-sm"
    tabindex="-1"
    on:click={close}
    aria-label="关闭清除数据确认"
  ></button>
  <div
    class="fixed left-1/2 top-1/2 z-[91] w-[500px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 panel p-6"
    role="dialog"
    aria-modal="true"
    aria-labelledby="clear-data-title"
    tabindex="-1"
    use:modalFocus={{ close, canClose: !busy, initialFocus: '[data-cancel-clear]' }}
  >
    <div class="flex items-start justify-between gap-4">
      <div class="flex min-w-0 gap-3">
        <span
          class="grid h-10 w-10 shrink-0 place-items-center rounded-xl text-danger"
          style="background: var(--danger-soft);"><AlertTriangle size={20} /></span
        >
        <div class="min-w-0">
          <p class="eyebrow">数据生命周期</p>
          <h3 id="clear-data-title" class="mt-1 text-xl font-semibold">{configuration.title}</h3>
          <p class="mt-2 text-sm leading-6 body-muted">{configuration.description}</p>
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
      {configuration.warning}
    </div>
    {#if error}
      <div
        class="mt-3 rounded-xl border px-4 py-3 text-xs leading-5 text-danger"
        style="border-color: var(--danger); background: var(--danger-soft);"
        role="alert"
      >
        {error}
      </div>
    {/if}
    <div class="mt-6 flex justify-end gap-2">
      <button class="btn" data-cancel-clear disabled={busy} on:click={close}>取消</button>
      <button class="btn-danger" disabled={busy} on:click={onConfirm}
        ><Trash2 size={15} />{busy ? '正在清除…' : configuration.confirmLabel}</button
      >
    </div>
  </div>
{/if}
