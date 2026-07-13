<script lang="ts">
  import { ArrowRight, Bot, CheckCircle2, Circle, FileText, KeyRound, LockKeyhole, RefreshCw, Rocket, ShieldCheck, Sparkles, UserRoundCheck, XCircle } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import { loading, refresh, setupBoss, snapshot } from '$lib/stores/app';
  import type { AiProviderConfig, BootstrapSnapshot, ProviderTestResult } from '$lib/types';

  type SetupState = 'needs_setup' | 'running' | 'ready' | 'failed';
  type SetupStatus = { state: SetupState; message?: string | null; lastAttemptAt?: string | null };
  type ConfiguredSnapshot = BootstrapSnapshot & { configuration?: { boss?: SetupStatus; llm?: SetupStatus } };

  let bossBusy = false;
  let bossFeedback = '';
  let llmDraft: AiProviderConfig | null = null;
  let llmDraftId = '';
  let apiKey = '';
  let testingLlm = false;
  let llmResult: ProviderTestResult | null = null;
  let llmError = '';

  const setupBossWithOptions = setupBoss as unknown as (options: { resetProfile: boolean }) => Promise<void>;
  const stateLabel: Record<SetupState, string> = { needs_setup: '待配置', running: '配置中', ready: '已完成', failed: '配置失败' };
  const formatTime = (value?: string | null) => value ? new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false
  }).format(new Date(value)) : '';

  $: configuration = ($snapshot as ConfiguredSnapshot).configuration;
  $: bossTask = $snapshot.tasks.find((task) => task.kind === 'boss-login');
  $: bossState = configuration?.boss?.state ?? (bossTask?.state === 'queued' || bossTask?.state === 'running' ? 'running' : bossTask?.state === 'failed' ? 'failed' : $snapshot.readiness.boss ? 'ready' : 'needs_setup');
  $: bossMessage = configuration?.boss?.message ?? (bossState === 'running' ? bossTask?.message : bossState === 'failed' ? bossTask?.recoverableError || bossTask?.message : '');
  $: llmState = configuration?.llm?.state ?? ($snapshot.readiness.ai ? 'ready' : 'needs_setup');
  $: availableProviders = $snapshot.providers.filter((provider) => (provider.kind as string) !== 'openrouter');
  $: defaultProvider = availableProviders.find((provider) => provider.isDefault) ?? availableProviders.find((provider) => provider.kind === 'xiaomi') ?? availableProviders[0];
  $: if (defaultProvider && llmDraftId !== defaultProvider.id) {
    llmDraft = structuredClone(defaultProvider);
    llmDraftId = defaultProvider.id;
    apiKey = '';
    llmResult = null;
    llmError = '';
  }

  async function configureBoss(resetProfile: boolean) {
    bossBusy = true;
    bossFeedback = '';
    try {
      await setupBossWithOptions({ resetProfile });
      bossFeedback = '专用 Chrome 已启动。请在浏览器中完成登录，验证结束后浏览器会自动关闭。';
    } catch (error) {
      bossFeedback = error instanceof Error ? error.message : String(error);
    } finally {
      bossBusy = false;
    }
  }

  async function testAndSaveLlm() {
    if (!llmDraft) return;
    testingLlm = true;
    llmResult = null;
    llmError = '';
    try {
      const saved = await backend.saveProvider({ ...llmDraft, apiKey: apiKey || undefined });
      llmResult = saved.testResult;
      if (saved.testResult.ok) {
        await refresh();
        const verified = $snapshot.providers.find((provider) => provider.id === llmDraft?.id);
        if (verified) llmDraft = structuredClone(verified);
        apiKey = '';
      }
    } catch (error) {
      llmError = error instanceof Error ? error.message : String(error);
    } finally {
      testingLlm = false;
    }
  }
</script>

<div class="page-content max-w-[1180px]">
  {#if $loading}
    <div class="space-y-5"><div class="skeleton h-40 rounded-2xl"></div><div class="grid grid-cols-2 gap-5"><div class="skeleton h-96 rounded-2xl"></div><div class="skeleton h-96 rounded-2xl"></div></div></div>
  {:else}
    <section class="relative overflow-hidden rounded-[24px] border px-7 py-7 shadow-panel" style="border-color: var(--line); background: linear-gradient(115deg, var(--panel) 0%, var(--brand-faint) 100%);">
      <div class="dot-grid pointer-events-none absolute inset-y-0 right-0 w-[42%] opacity-40"></div>
      <div class="relative flex items-end justify-between gap-8">
        <div class="max-w-[720px]">
          <div class="mb-3 inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-semibold" style="background: var(--brand-soft); color: var(--brand);"><Sparkles size={13} />首次使用向导</div>
          <h2 class="text-[29px] font-semibold tracking-[-0.04em]">先完成两项必要配置</h2>
          <p class="mt-2 text-sm leading-6 body-muted">连接 BOSS 专用浏览器并验证默认模型后，就可以开始抓取和分析岗位。配置未完成时，其他页面仍然可以正常查看。</p>
        </div>
        <span class="hidden h-12 w-12 shrink-0 place-items-center rounded-2xl bg-panel text-brand shadow-sm lg:grid"><Rocket size={22} /></span>
      </div>
    </section>

    <section class="setup-grid mt-6 grid grid-cols-2 gap-5">
      <article class="panel overflow-hidden">
        <div class="flex items-start justify-between border-b px-6 py-5" style="border-color: var(--line);">
          <div class="flex gap-3"><span class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"><UserRoundCheck size={19} /></span><div><p class="eyebrow">步骤 1</p><h3 class="section-title mt-1">登录 BOSS 直聘</h3></div></div>
          <span class:chip-brand={bossState === 'ready'} class="chip">{#if bossState === 'ready'}<CheckCircle2 size={13} />{:else if bossState === 'running'}<Circle size={12} class="animate-spin" />{:else if bossState === 'failed'}<XCircle size={13} />{/if}{stateLabel[bossState]}</span>
        </div>
        <div class="p-6">
          <p class="text-sm leading-6 body-muted">应用会打开一个独立的 Chrome Profile。登录验证成功或失败后，只会关闭这个专用浏览器，不会影响你的日常 Chrome。</p>
          <div class="mt-5 rounded-xl border p-4" style={`border-color:${bossState === 'failed' ? 'var(--danger)' : 'var(--line)'}; background:${bossState === 'failed' ? 'var(--danger-soft)' : 'var(--panel-soft)'}`}>
            {#if bossState === 'ready'}
              <div class="flex items-start gap-3"><ShieldCheck size={18} class="mt-0.5 shrink-0 text-success" /><div><p class="text-sm font-semibold">BOSS 登录配置已完成</p><p class="mt-1 text-xs leading-5 body-muted">每次抓取前仍会重新检查真实登录状态。</p></div></div>
            {:else if bossState === 'running'}
              <div class="flex items-start gap-3"><RefreshCw size={18} class="mt-0.5 shrink-0 animate-spin text-brand" /><div><p class="text-sm font-semibold">正在等待登录验证</p><p class="mt-1 text-xs leading-5 body-muted">{bossMessage || '请在打开的 Chrome 中完成登录。'}</p></div></div>
            {:else if bossState === 'failed'}
              <div class="flex items-start gap-3"><XCircle size={18} class="mt-0.5 shrink-0 text-danger" /><div><p class="text-sm font-semibold">本次配置未完成</p><p class="mt-1 text-xs leading-5 body-muted">{bossMessage || '请重试；如果问题持续出现，可重新创建专用 Profile。'}</p></div></div>
            {:else}
              <div class="flex items-start gap-3"><LockKeyhole size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">需要你手动开始</p><p class="mt-1 text-xs leading-5 body-muted">应用不会在首次打开时自动弹出浏览器。</p></div></div>
            {/if}
          </div>
          {#if configuration?.boss?.lastAttemptAt}<p class="mt-3 text-[11px] body-muted">上次验证：{formatTime(configuration.boss.lastAttemptAt)}</p>{/if}
          {#if bossFeedback}<p class="mt-3 text-xs leading-5 body-muted">{bossFeedback}</p>{/if}
          <button class="btn-primary mt-5 w-full" disabled={bossBusy || bossState === 'running'} on:click={() => configureBoss(bossState === 'ready' || bossState === 'failed')}>
            {#if bossBusy || bossState === 'running'}<RefreshCw size={15} class="animate-spin" />等待配置完成{:else if bossState === 'ready'}<RefreshCw size={15} />重新配置 BOSS{:else}<UserRoundCheck size={15} />打开 Chrome 并登录{/if}
          </button>
        </div>
      </article>

      <article class="panel overflow-hidden">
        <div class="flex items-start justify-between border-b px-6 py-5" style="border-color: var(--line);">
          <div class="flex gap-3"><span class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"><Bot size={19} /></span><div><p class="eyebrow">步骤 2</p><h3 class="section-title mt-1">配置默认模型</h3></div></div>
          <span class:chip-brand={llmState === 'ready'} class="chip">{#if llmState === 'ready'}<CheckCircle2 size={13} />{:else if llmState === 'running'}<Circle size={12} class="animate-spin" />{:else if llmState === 'failed'}<XCircle size={13} />{/if}{stateLabel[llmState]}</span>
        </div>
        <div class="p-6">
          {#if llmState === 'ready' && defaultProvider}
            <div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--panel-soft);"><div class="flex items-start gap-3"><CheckCircle2 size={18} class="mt-0.5 shrink-0 text-success" /><div class="min-w-0"><p class="text-sm font-semibold">{defaultProvider.name} 已验证</p><p class="mt-1 truncate text-xs body-muted">{defaultProvider.model} · {defaultProvider.baseUrl}</p></div></div></div>
            <p class="mt-4 text-xs leading-5 body-muted">只有在你主动使用 AI 功能时，必要的岗位或简历上下文才会发送给当前模型服务商。</p>
            <a class="btn mt-5 w-full" href="/settings">管理模型配置 <ArrowRight size={15} /></a>
          {:else if llmDraft}
            <p class="mb-5 text-sm leading-6 body-muted">填写 API Key 并验证连接。连接成功后 Key 才会保存到系统钥匙串；失败不会保存本次输入。</p>
            <div class="space-y-4">
              <label><span class="label">Base URL</span><input class="input" bind:value={llmDraft.baseUrl} placeholder="https://token-plan-sgp.xiaomimimo.com/v1" /></label>
              <label><span class="label">模型</span><input class="input" bind:value={llmDraft.model} placeholder="mimo-v2.5" /></label>
              <label><span class="label">API Key</span><div class="relative"><KeyRound size={15} class="absolute left-3 top-3 body-muted" /><input class="input pl-9" type="password" bind:value={apiKey} placeholder={llmDraft.apiKeyRef ? '已安全保存；留空则使用现有 Key' : '粘贴你的 API Key'} /></div></label>
            </div>
            {#if llmResult}<div class="mt-4 flex items-start gap-3 rounded-xl border p-3" style={`border-color:${llmResult.ok ? 'var(--success)' : 'var(--danger)'}; background:${llmResult.ok ? 'var(--brand-faint)' : 'var(--danger-soft)'}`}><svelte:component this={llmResult.ok ? CheckCircle2 : XCircle} size={17} class={llmResult.ok ? 'text-success' : 'text-danger'} /><div><p class="text-sm font-semibold">{llmResult.message}</p><p class="mt-0.5 text-[11px] body-muted">延迟 {llmResult.latencyMs} ms · 结构化输出 {llmResult.structuredOutput ? '正常' : '未通过'}</p></div></div>{/if}
            {#if llmError}<p class="mt-3 text-xs text-danger">{llmError}</p>{/if}
            <button class="btn-primary mt-5 w-full" disabled={testingLlm || !llmDraft.baseUrl || !llmDraft.model || (!apiKey && !llmDraft.apiKeyRef)} on:click={testAndSaveLlm}>{#if testingLlm}<RefreshCw size={15} class="animate-spin" />正在测试{:else}<ShieldCheck size={15} />测试并保存{/if}</button>
          {:else}
            <div class="rounded-xl border p-4" style="border-color: var(--danger); background: var(--danger-soft);"><p class="text-sm font-semibold">没有可用的模型预设</p><p class="mt-1 text-xs leading-5 body-muted">请到设置中创建一个自定义 OpenAI 兼容模型。</p></div>
            <a class="btn mt-5 w-full" href="/settings">打开模型设置 <ArrowRight size={15} /></a>
          {/if}
        </div>
      </article>
    </section>

    <section class="mt-6 panel p-6">
      <div class="flex items-center justify-between gap-6">
        <div class="flex min-w-0 items-center gap-4"><span class="grid h-11 w-11 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"><FileText size={20} /></span><div class="min-w-0"><p class="eyebrow">主简历</p><h3 class="section-title mt-1">{$snapshot.resume ? `${$snapshot.resume.name} · 版本 ${$snapshot.resume.version}` : '导入或建立你的主简历'}</h3><p class="mt-1 truncate text-xs body-muted">{$snapshot.resume ? $snapshot.resume.sourceFileName : '主简历是岗位匹配、面试准备和 AI 修改的事实来源。'}</p></div></div>
        <a class="btn-primary shrink-0" href="/resume">{$snapshot.resume ? '打开主简历' : '前往导入'} <ArrowRight size={15} /></a>
      </div>
    </section>
    <p class="mt-5 text-center text-xs body-muted">所有岗位、简历和统计数据都保存在本机；初始化不会限制你浏览应用的其他页面。</p>
  {/if}
</div>

<style>
  @media (max-width: 980px) { .setup-grid { grid-template-columns: minmax(0, 1fr); } }
</style>
