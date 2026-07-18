<script lang="ts">
  import { onMount } from 'svelte';
  import {
    ArrowRight,
    BarChart3,
    Bot,
    BriefcaseBusiness,
    CheckCircle2,
    Circle,
    FileText,
    KeyRound,
    RefreshCw,
    Rocket,
    Search,
    ShieldCheck,
    Sparkles,
    XCircle
  } from 'lucide-svelte';
  import BossSetupCard from '$lib/components/BossSetupCard.svelte';
  import FitScore from '$lib/components/FitScore.svelte';
  import JobSearchDialog from '$lib/components/JobSearchDialog.svelte';
  import MarkdownView from '$lib/components/MarkdownView.svelte';
  import { availableProviderConfigs } from '$lib/provider-policy';
  import { createSearchSpec } from '$lib/search-spec';
  import { backend } from '$lib/services/backend';
  import { loading, refresh, snapshot, startScrape } from '$lib/stores/app';
  import type {
    AiProviderConfig,
    JobPage,
    JobQuery,
    ProviderTestResult,
    SearchSpec
  } from '$lib/types';

  let llmDraft: AiProviderConfig | null = null;
  let llmDraftId = '';
  let apiKey = '';
  let testingLlm = false;
  let llmResult: ProviderTestResult | null = null;
  let llmError = '';
  let mounted = false;
  let dashboardActive = false;
  let dashboardLoading = false;
  let dashboardError = '';
  let dashboardPage: JobPage | null = null;
  let newJobCount = 0;
  let dashboardRequestId = 0;
  let lastTerminalScrapeKey = '';
  let searchDialogOpen = false;
  let scraping = false;
  let searchSpec: SearchSpec = createSearchSpec();
  let toast = '';

  const stateLabel = {
    needs_setup: '待配置',
    running: '配置中',
    ready: '已完成',
    failed: '配置失败'
  } as const;
  const emptyJobQuery = (onlyNew = false): JobQuery => ({
    query: '',
    minScore: 0,
    onlyNew,
    salary: '',
    companyScale: '',
    city: '',
    missingDescription: false,
    cursor: null
  });

  $: setupComplete = $snapshot.readiness.boss && $snapshot.readiness.ai;
  $: llmState =
    $snapshot.configuration?.llm?.state ?? ($snapshot.readiness.ai ? 'ready' : 'needs_setup');
  $: availableProviders = availableProviderConfigs($snapshot.providers);
  $: defaultProvider =
    availableProviders.find((provider) => provider.isDefault) ??
    availableProviders.find((provider) => provider.kind === 'xiaomi') ??
    availableProviders[0];
  $: if (defaultProvider && llmDraftId !== defaultProvider.id) {
    llmDraft = structuredClone(defaultProvider);
    llmDraftId = defaultProvider.id;
    apiKey = '';
    llmResult = null;
    llmError = '';
  }
  $: latestRun = $snapshot.scrapeRuns.find((run) => Boolean(run.completedAt) && run.totalSeen > 0);
  $: topJobs = dashboardPage?.items.slice(0, 3) ?? [];
  $: scrapeTaskRunning =
    scraping ||
    $snapshot.tasks.some(
      (task) => task.kind === 'scrape' && (task.state === 'queued' || task.state === 'running')
    );
  $: terminalScrapeKey = $snapshot.tasks
    .filter(
      (task) => task.kind === 'scrape' && ['completed', 'failed', 'cancelled'].includes(task.state)
    )
    .map((task) => `${task.id}:${task.updatedAt}`)
    .join('|');
  $: if (mounted) {
    if (setupComplete && !dashboardActive) {
      dashboardActive = true;
      void loadDashboard();
    } else if (!setupComplete) {
      dashboardActive = false;
    }
  }
  $: if (
    mounted &&
    setupComplete &&
    terminalScrapeKey &&
    terminalScrapeKey !== lastTerminalScrapeKey
  ) {
    lastTerminalScrapeKey = terminalScrapeKey;
    void loadDashboard();
  }

  onMount(() => {
    lastTerminalScrapeKey = terminalScrapeKey;
    mounted = true;
  });

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

  async function loadDashboard() {
    const requestId = ++dashboardRequestId;
    dashboardLoading = true;
    dashboardError = '';
    try {
      const [allJobs, newJobs] = await Promise.all([
        backend.listJobsPage(emptyJobQuery()),
        backend.listJobsPage(emptyJobQuery(true))
      ]);
      if (requestId !== dashboardRequestId) return;
      dashboardPage = allJobs;
      newJobCount = newJobs.total;
    } catch (error) {
      if (requestId === dashboardRequestId)
        dashboardError = error instanceof Error ? error.message : String(error);
    } finally {
      if (requestId === dashboardRequestId) dashboardLoading = false;
    }
  }

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => toast === message && (toast = ''), 2600);
  }

  function openSearchDialog() {
    searchSpec = createSearchSpec($snapshot.lastSearchSpec);
    searchDialogOpen = true;
  }

  async function runScrape() {
    if (scrapeTaskRunning) return;
    scraping = true;
    try {
      await startScrape(searchSpec);
      searchDialogOpen = false;
      showToast('抓取任务已启动，请勿关闭应用');
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      scraping = false;
    }
  }
</script>

<div class="page-content max-w-[1240px]">
  {#if $loading}
    <div class="space-y-5">
      <div class="skeleton h-40 rounded-2xl"></div>
      <div class="grid grid-cols-3 gap-5">
        <div class="skeleton h-32 rounded-2xl"></div>
        <div class="skeleton h-32 rounded-2xl"></div>
        <div class="skeleton h-32 rounded-2xl"></div>
      </div>
      <div class="skeleton h-80 rounded-2xl"></div>
    </div>
  {:else if !setupComplete}
    <section
      class="relative overflow-hidden rounded-[24px] border px-7 py-7 shadow-panel"
      style="border-color: var(--line); background: linear-gradient(115deg, var(--panel) 0%, var(--brand-faint) 100%);"
    >
      <div class="dot-grid pointer-events-none absolute inset-y-0 right-0 w-[42%] opacity-40"></div>
      <div class="relative flex items-end justify-between gap-8">
        <div class="max-w-[720px]">
          <div
            class="mb-3 inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-semibold"
            style="background: var(--brand-soft); color: var(--brand);"
          >
            <Sparkles size={13} />首次使用向导
          </div>
          <h2 class="text-[29px] font-semibold tracking-[-0.04em]">先完成两项必要配置</h2>
          <p class="mt-2 text-sm leading-6 body-muted">
            连接 BOSS
            专用浏览器并验证默认模型后，就可以开始抓取和分析岗位。配置未完成时，其他页面仍然可以正常查看。
          </p>
        </div>
        <span
          class="hidden h-12 w-12 shrink-0 place-items-center rounded-2xl bg-panel text-brand shadow-sm lg:grid"
          ><Rocket size={22} /></span
        >
      </div>
    </section>

    <section class="setup-grid mt-6 grid grid-cols-2 gap-5">
      <BossSetupCard />
      <article class="panel overflow-hidden">
        <div
          class="flex items-start justify-between border-b px-6 py-5"
          style="border-color: var(--line);"
        >
          <div class="flex gap-3">
            <span
              class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"
              ><Bot size={19} /></span
            >
            <div>
              <p class="eyebrow">步骤 2</p>
              <h3 class="section-title mt-1">配置默认模型</h3>
            </div>
          </div>
          <span class:chip-brand={llmState === 'ready'} class="chip"
            >{#if llmState === 'ready'}<CheckCircle2
                size={13}
              />{:else if llmState === 'running'}<Circle
                size={12}
                class="animate-spin"
              />{:else if llmState === 'failed'}<XCircle size={13} />{/if}{stateLabel[
              llmState
            ]}</span
          >
        </div>
        <div class="p-6">
          {#if llmState === 'ready' && defaultProvider}
            <div
              class="rounded-xl border p-4"
              style="border-color: var(--line); background: var(--panel-soft);"
            >
              <div class="flex items-start gap-3">
                <CheckCircle2 size={18} class="mt-0.5 shrink-0 text-success" />
                <div class="min-w-0">
                  <p class="text-sm font-semibold">{defaultProvider.name} 已验证</p>
                  <p class="mt-1 truncate text-xs body-muted">
                    {defaultProvider.model} · {defaultProvider.baseUrl}
                  </p>
                </div>
              </div>
            </div>
            <p class="mt-4 text-xs leading-5 body-muted">
              只有在你主动使用 AI 功能时，必要的岗位或简历上下文才会发送给当前模型服务商。
            </p>
            <a class="btn mt-5 w-full" href="/settings">管理模型配置 <ArrowRight size={15} /></a>
          {:else if llmDraft}
            <p class="mb-5 text-sm leading-6 body-muted">
              填写 API Key 并验证连接。连接成功后 Key 才会保存到系统钥匙串；失败不会保存本次输入。
            </p>
            <div class="space-y-4">
              <label
                ><span class="label">Base URL</span><input
                  class="input"
                  bind:value={llmDraft.baseUrl}
                  placeholder="https://api.example.com/v1"
                /></label
              >{#if llmDraft.baseUrl.trim().toLowerCase().startsWith('http://')}<label
                  class="flex items-start gap-3 rounded-xl border p-3 text-xs leading-5 text-warning"
                  style="border-color: var(--warning); background: var(--warning-soft);"
                  ><input
                    class="mt-1 h-4 w-4"
                    type="checkbox"
                    bind:checked={llmDraft.allowInsecureHttp}
                  /><span
                    ><strong>允许不安全 HTTP</strong><br />API Key 和请求内容会通过明文连接发送。</span
                  ></label
                >{/if}<label
                ><span class="label">模型</span><input
                  class="input"
                  bind:value={llmDraft.model}
                  placeholder="模型名称"
                /></label
              ><label
                ><span class="label">API Key</span>
                <div class="relative">
                  <KeyRound size={15} class="absolute left-3 top-3 body-muted" /><input
                    class="input pl-9"
                    type="password"
                    bind:value={apiKey}
                    placeholder={llmDraft.apiKeyRef
                      ? '已安全保存；留空则使用现有 Key'
                      : '粘贴你的 API Key'}
                  />
                </div></label
              >
            </div>
            {#if llmResult}<div
                class="mt-4 flex items-start gap-3 rounded-xl border p-3"
                style={`border-color:${llmResult.ok ? 'var(--success)' : 'var(--danger)'}; background:${llmResult.ok ? 'var(--brand-faint)' : 'var(--danger-soft)'}`}
              >
                <svelte:component
                  this={llmResult.ok ? CheckCircle2 : XCircle}
                  size={17}
                  class={llmResult.ok ? 'text-success' : 'text-danger'}
                />
                <div>
                  <p class="text-sm font-semibold">{llmResult.message}</p>
                  <p class="mt-0.5 text-[11px] body-muted">
                    延迟 {llmResult.latencyMs} ms · 结构化输出 {llmResult.structuredOutput
                      ? '正常'
                      : '未通过'}
                  </p>
                </div>
              </div>{/if}
            {#if llmError}<p class="mt-3 text-xs text-danger">{llmError}</p>{/if}
            <button
              class="btn-primary mt-5 w-full"
              disabled={testingLlm ||
                !llmDraft.baseUrl ||
                !llmDraft.model ||
                (!apiKey && !llmDraft.apiKeyRef) ||
                (llmDraft.baseUrl.trim().toLowerCase().startsWith('http://') &&
                  !llmDraft.allowInsecureHttp)}
              on:click={testAndSaveLlm}
              >{#if testingLlm}<RefreshCw
                  size={15}
                  class="animate-spin"
                />正在测试{:else}<ShieldCheck size={15} />测试并保存{/if}</button
            >
          {:else}
            <div
              class="rounded-xl border p-4"
              style="border-color: var(--danger); background: var(--danger-soft);"
            >
              <p class="text-sm font-semibold">没有可用的模型预设</p>
              <p class="mt-1 text-xs leading-5 body-muted">
                请到设置中创建一个自定义 OpenAI 兼容模型。
              </p>
            </div>
            <a class="btn mt-5 w-full" href="/settings">打开模型设置 <ArrowRight size={15} /></a>
          {/if}
        </div>
      </article>
    </section>
    <section class="mt-6 panel p-6">
      <div class="flex items-center justify-between gap-6">
        <div class="flex min-w-0 items-center gap-4">
          <span
            class="grid h-11 w-11 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"
            ><FileText size={20} /></span
          >
          <div class="min-w-0">
            <p class="eyebrow">主简历</p>
            <h3 class="section-title mt-1">
              {$snapshot.resume
                ? `${$snapshot.resume.name} · 版本 ${$snapshot.resume.version}`
                : '导入或建立你的主简历'}
            </h3>
            <p class="mt-1 truncate text-xs body-muted">
              {$snapshot.resume
                ? $snapshot.resume.sourceFileName
                : '主简历是岗位匹配、面试准备和 AI 修改的事实来源。'}
            </p>
          </div>
        </div>
        <a class="btn-primary shrink-0" href="/resume"
          >{$snapshot.resume ? '打开主简历' : '前往导入'} <ArrowRight size={15} /></a
        >
      </div>
    </section>
  {:else}
    <section
      class="relative overflow-hidden rounded-[24px] border px-7 py-7 shadow-panel"
      style="border-color: var(--line); background: linear-gradient(115deg, var(--panel) 0%, var(--brand-faint) 100%);"
    >
      <div class="dot-grid pointer-events-none absolute inset-y-0 right-0 w-[42%] opacity-40"></div>
      <div class="relative flex items-end justify-between gap-8">
        <div class="max-w-[720px]">
          <div
            class="mb-3 inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-semibold"
            style="background: var(--brand-soft); color: var(--brand);"
          >
            <Sparkles size={13} />求职工作台
          </div>
          <h2 class="text-[29px] font-semibold tracking-[-0.04em]">
            {$snapshot.resume?.name ? `${$snapshot.resume.name}，欢迎回来` : '欢迎来到求职舱'}
          </h2>
          <p class="mt-2 text-sm leading-6 body-muted">
            集中查看本地岗位样本、优先机会与简历版本，把时间花在更值得核对和跟进的岗位上。
          </p>
        </div>
        <button
          class="btn-primary shrink-0"
          disabled={scrapeTaskRunning}
          on:click={openSearchDialog}
          ><Search size={16} />{scrapeTaskRunning ? '岗位抓取中…' : '开始岗位搜索'}</button
        >
      </div>
    </section>

    <section class="mt-6 grid grid-cols-3 gap-4">
      <article class="panel-flat p-5">
        <div class="flex items-center justify-between">
          <span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"
            ><BriefcaseBusiness size={19} /></span
          ><span class="chip">本地岗位库</span>
        </div>
        <p class="mt-5 text-3xl font-semibold">
          {dashboardLoading && !dashboardPage ? '—' : (dashboardPage?.total ?? 0)}
        </p>
        <p class="mt-1 text-xs body-muted">累计岗位</p>
      </article>
      <article class="panel-flat p-5">
        <div class="flex items-center justify-between">
          <span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"
            ><Sparkles size={19} /></span
          ><span class="chip-brand">待关注</span>
        </div>
        <p class="mt-5 text-3xl font-semibold">
          {dashboardLoading && !dashboardPage ? '—' : newJobCount}
        </p>
        <p class="mt-1 text-xs body-muted">新增岗位</p>
      </article>
      <article class="panel-flat p-5">
        <div class="flex items-center justify-between">
          <span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"
            ><BarChart3 size={19} /></span
          ><a href="/reports" class="text-xs font-semibold text-brand">打开报告</a>
        </div>
        <p class="mt-5 text-3xl font-semibold">{latestRun?.totalSeen ?? 0}</p>
        <p class="mt-1 truncate text-xs body-muted">
          {latestRun ? `最近抓取 · ${latestRun.keyword} · ${latestRun.city}` : '尚无成功抓取记录'}
        </p>
      </article>
    </section>

    {#if dashboardError}<div
        class="mt-5 flex items-center justify-between rounded-xl border px-4 py-3 text-xs text-danger"
        style="border-color: var(--danger); background: var(--danger-soft);"
      >
        <span>看板数据加载失败：{dashboardError}</span><button
          class="btn h-8"
          on:click={loadDashboard}>重试</button
        >
      </div>{/if}

    <section class="mt-6 grid grid-cols-[1.25fr_.75fr] gap-5">
      <article class="panel p-6">
        <div class="flex items-start justify-between">
          <div>
            <p class="eyebrow">下一步</p>
            <h3 class="section-title mt-1">开始一轮岗位搜索</h3>
            <p class="mt-2 text-sm leading-6 body-muted">
              选择关键词、城市与范围；确认后会先检查 BOSS 登录，再自动抓取岗位列表和详情。
            </p>
          </div>
          <span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"
            ><Search size={19} /></span
          >
        </div>
        <button class="btn-primary mt-6" disabled={scrapeTaskRunning} on:click={openSearchDialog}
          >{scrapeTaskRunning ? '已有抓取任务运行' : '设置搜索条件'}
          <ArrowRight size={15} /></button
        >
      </article>
      <article class="panel p-6">
        <p class="eyebrow">主简历</p>
        <h3 class="section-title mt-1">
          {$snapshot.resume
            ? `${$snapshot.resume.name || '未命名简历'} · 版本 ${$snapshot.resume.version}`
            : '尚未建立主简历'}
        </h3>
        <p class="mt-2 text-xs leading-5 body-muted">
          {$snapshot.resume
            ? $snapshot.resume.sourceFileName
            : '导入或创建简历后，可获得基于事实的岗位匹配与材料建议。'}
        </p>
        <a class="btn mt-6 w-full" href="/resume"
          >{$snapshot.resume ? '维护主简历' : '创建主简历'} <ArrowRight size={15} /></a
        >
      </article>
    </section>

    <section class="mt-8">
      <div class="mb-3 flex items-end justify-between">
        <div>
          <p class="eyebrow">优先处理</p>
          <h3 class="section-title mt-1">{$snapshot.resume ? '与你最接近的机会' : '最近岗位'}</h3>
        </div>
        <a href="/jobs" class="flex items-center gap-1 text-xs font-semibold text-brand"
          >查看全部 <ArrowRight size={14} /></a
        >
      </div>
      {#if dashboardLoading && !dashboardPage}
        <div class="grid grid-cols-3 gap-4">
          {#each [1, 2, 3] as _}<div class="skeleton h-32 rounded-2xl"></div>{/each}
        </div>
      {:else if topJobs.length}
        <div class="grid grid-cols-3 gap-4">
          {#each topJobs as job}<a
              href={`/jobs?job=${job.id}`}
              class="panel-flat group p-4 transition hover:-translate-y-0.5 hover:shadow-panel"
              ><div class="flex gap-3">
                {#if $snapshot.resume}<FitScore score={job.fit?.overallScore ?? 0} size="sm" />{/if}
                <div class="min-w-0">
                  <h4 class="truncate text-sm font-semibold group-hover:text-brand">{job.title}</h4>
                  <p class="mt-0.5 truncate text-xs body-muted">
                    {job.company} · {job.location.split('·')[0]}
                  </p>
                </div>
              </div>
              <div class="my-3 divider"></div>
              <div class="flex items-center justify-between gap-3">
                <span class="truncate text-sm font-semibold text-brand">{job.salary}</span><span
                  class="shrink-0 text-[11px] body-muted">{job.experience} · {job.degree}</span
                >
              </div></a
            >{/each}
        </div>
      {:else}
        <div class="panel-flat p-8 text-center">
          <BriefcaseBusiness size={24} class="mx-auto text-brand" />
          <p class="mt-3 text-sm font-semibold">岗位库还是空的</p>
          <p class="mt-1 text-xs body-muted">开始第一次岗位搜索后，机会会显示在这里。</p>
        </div>
      {/if}
    </section>

    {#if latestRun?.reportMarkdown}
      <section class="mt-8 panel p-6">
        <div class="mb-4 flex items-center justify-between">
          <div>
            <p class="eyebrow">最近一轮 · {latestRun.city}</p>
            <h3 class="section-title mt-1">本次岗位样本观察</h3>
          </div>
          <span class="chip"><BriefcaseBusiness size={13} />{latestRun.totalSeen} 个本地样本</span>
        </div>
        <MarkdownView source={latestRun.reportMarkdown} />
      </section>
    {/if}

    <section
      class="mt-6 flex items-center justify-between rounded-2xl border px-5 py-4"
      style="border-color: var(--line); background: var(--panel-soft);"
    >
      <div class="flex items-center gap-3">
        <CheckCircle2 size={18} class="text-success" />
        <div>
          <p class="text-sm font-semibold">BOSS 与默认模型已就绪</p>
          <p class="mt-0.5 text-xs body-muted">抓取前仍会验证真实登录状态；连接配置可随时调整。</p>
        </div>
      </div>
      <a class="btn" href="/settings#boss">管理连接 <ArrowRight size={15} /></a>
    </section>
  {/if}
</div>

{#if setupComplete}<JobSearchDialog
    bind:open={searchDialogOpen}
    bind:searchSpec
    {scraping}
    {scrapeTaskRunning}
    onStart={runScrape}
  />{/if}
{#if toast}<div
    class="fixed bottom-6 left-1/2 z-[80] -translate-x-1/2 rounded-xl bg-[var(--ink)] px-4 py-3 text-sm text-white shadow-xl"
  >
    {toast}
  </div>{/if}

<style>
  @media (max-width: 980px) {
    .setup-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
