<script lang="ts">
  import { CheckCircle2, Circle, LockKeyhole, RefreshCw, ShieldCheck, UserRoundCheck, XCircle } from 'lucide-svelte';
  import { setupBoss, snapshot } from '$lib/stores/app';

  export let eyebrow = '步骤 1';
  export let title = '登录 BOSS 直聘';

  let busy = false;
  let feedback = '';
  const stateLabel = { needs_setup: '待配置', running: '配置中', ready: '已完成', failed: '配置失败' } as const;
  const formatTime = (value?: string | null) => value ? new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false
  }).format(new Date(value)) : '';

  $: task = $snapshot.tasks.find((item) => item.kind === 'boss-login');
  $: state = $snapshot.configuration?.boss?.state
    ?? (task?.state === 'queued' || task?.state === 'running' ? 'running' : task?.state === 'failed' ? 'failed' : $snapshot.readiness.boss ? 'ready' : 'needs_setup');
  $: message = $snapshot.configuration?.boss?.message
    ?? (state === 'running' ? task?.message : state === 'failed' ? task?.recoverableError || task?.message : '');

  async function configure(resetProfile: boolean) {
    busy = true;
    feedback = '';
    try {
      await setupBoss({ resetProfile });
      feedback = '专用 Chrome 已启动。请在浏览器中完成登录，验证结束后浏览器会自动关闭。';
    } catch (error) {
      feedback = error instanceof Error ? error.message : String(error);
    } finally {
      busy = false;
    }
  }
</script>

<article class="panel overflow-hidden">
  <div class="flex items-start justify-between border-b px-6 py-5" style="border-color: var(--line);">
    <div class="flex gap-3"><span class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"><UserRoundCheck size={19} /></span><div><p class="eyebrow">{eyebrow}</p><h3 class="section-title mt-1">{title}</h3></div></div>
    <span class:chip-brand={state === 'ready'} class="chip">{#if state === 'ready'}<CheckCircle2 size={13} />{:else if state === 'running'}<Circle size={12} class="animate-spin" />{:else if state === 'failed'}<XCircle size={13} />{/if}{stateLabel[state]}</span>
  </div>
  <div class="p-6">
    <p class="text-sm leading-6 body-muted">应用会打开一个独立的 Chrome Profile。登录验证成功或失败后，只会关闭这个专用浏览器，不会影响你的日常 Chrome。</p>
    <div class="mt-5 rounded-xl border p-4" style={`border-color:${state === 'failed' ? 'var(--danger)' : 'var(--line)'}; background:${state === 'failed' ? 'var(--danger-soft)' : 'var(--panel-soft)'}`}>
      {#if state === 'ready'}
        <div class="flex items-start gap-3"><ShieldCheck size={18} class="mt-0.5 shrink-0 text-success" /><div><p class="text-sm font-semibold">BOSS 登录配置已完成</p><p class="mt-1 text-xs leading-5 body-muted">每次抓取前仍会重新检查真实登录状态。</p></div></div>
      {:else if state === 'running'}
        <div class="flex items-start gap-3"><RefreshCw size={18} class="mt-0.5 shrink-0 animate-spin text-brand" /><div><p class="text-sm font-semibold">正在等待登录验证</p><p class="mt-1 text-xs leading-5 body-muted">{message || '请在打开的 Chrome 中完成登录。'}</p></div></div>
      {:else if state === 'failed'}
        <div class="flex items-start gap-3"><XCircle size={18} class="mt-0.5 shrink-0 text-danger" /><div><p class="text-sm font-semibold">本次配置未完成</p><p class="mt-1 text-xs leading-5 body-muted">{message || '请重试；如果问题持续出现，可重新创建专用 Profile。'}</p></div></div>
      {:else}
        <div class="flex items-start gap-3"><LockKeyhole size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">需要你手动开始</p><p class="mt-1 text-xs leading-5 body-muted">应用不会在首次打开时自动弹出浏览器。</p></div></div>
      {/if}
    </div>
    {#if $snapshot.configuration?.boss?.lastAttemptAt}<p class="mt-3 text-[11px] body-muted">上次验证：{formatTime($snapshot.configuration.boss.lastAttemptAt)}</p>{/if}
    {#if feedback}<p class="mt-3 text-xs leading-5 body-muted">{feedback}</p>{/if}
    <button class="btn-primary mt-5 w-full" disabled={busy || state === 'running'} on:click={() => configure(state === 'ready' || state === 'failed')}>
      {#if busy || state === 'running'}<RefreshCw size={15} class="animate-spin" />等待配置完成{:else if state === 'ready'}<RefreshCw size={15} />重新配置 BOSS{:else}<UserRoundCheck size={15} />打开 Chrome 并登录{/if}
    </button>
  </div>
</article>
