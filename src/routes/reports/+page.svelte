<script lang="ts">
  import { onMount } from 'svelte';
  import { replaceState } from '$app/navigation';
  import {
    AlertCircle,
    BarChart3,
    BookOpenCheck,
    Building2,
    CalendarRange,
    CheckCircle2,
    Clock3,
    Database,
    Download,
    FileCheck2,
    Lightbulb,
    MapPinned,
    MessageCircleQuestion,
    RefreshCw,
    Sparkles,
    Target,
    TrendingUp,
    WalletCards
  } from 'lucide-svelte';
  import ReportBars from '$lib/components/ReportBars.svelte';
  import { chooseLocalExportPath, localExportStamp } from '$lib/export-file';
  import { backend } from '$lib/services/backend';
  import type {
    InterviewPreparationState,
    JobDataReport,
    RenderResult,
    ReportBucket,
    ReportCompetitivenessState,
    ReportSalaryBand
  } from '$lib/types';

  type ReportKeyword = {
    key: string;
    label: string;
    jobCount: number;
    lastSeen: string;
  };
  const HISTORICAL_UNCLASSIFIED_KEY = '__historical_unclassified__';

  type KeywordReportBackend = {
    listReportKeywords: () => Promise<ReportKeyword[]>;
    getJobDataReport: (keywordKeys: string[]) => Promise<JobDataReport>;
    exportJobDataReport: (keywordKeys: string[], outputPath: string) => Promise<RenderResult>;
    getInterviewPreparationState: (keywordKeys: string[]) => Promise<InterviewPreparationState>;
    generateInterviewPreparation: (
      keywordKeys: string[],
      force?: boolean
    ) => Promise<InterviewPreparationState>;
    getReportCompetitivenessState: (keywordKeys: string[]) => Promise<ReportCompetitivenessState>;
    generateReportCompetitiveness: (
      keywordKeys: string[],
      force?: boolean
    ) => Promise<ReportCompetitivenessState>;
  };

  const reportBackend = backend as unknown as KeywordReportBackend;

  let report: JobDataReport | null = null;
  let loading = true;
  let exporting = false;
  let error = '';
  let exportMessage = '';

  let interviewState: InterviewPreparationState | null = null;
  let interviewLoading = true;
  let interviewGenerating = false;
  let interviewError = '';
  let competitivenessState: ReportCompetitivenessState | null = null;
  let competitivenessLoading = true;
  let competitivenessGenerating = false;
  let competitivenessError = '';
  let keywords: ReportKeyword[] = [];
  let selectedKeywordKeys: string[] = [];
  let latestKeywordKey = '';
  let keywordsLoading = true;
  let keywordsError = '';
  let reportRequestId = 0;
  let interviewRequestId = 0;
  let competitivenessRequestId = 0;

  $: selectedKeywordLabels = keywords
    .filter((keyword) => selectedKeywordKeys.includes(keyword.key))
    .map((keyword) => keyword.label);
  $: marketContextEligible = selectedKeywordKeys.length > 0 && selectedKeywordKeys.length <= 8;
  $: competitiveness =
    competitivenessState?.effectiveSource === 'ai'
      ? competitivenessState.ai
      : competitivenessState?.local;

  const salary = (value?: number | null) => (value == null ? '—' : `${value.toFixed(1)}K`);
  const generatedTime = (value?: string | null) => {
    if (!value) return '';
    return new Intl.DateTimeFormat('zh-CN', {
      timeZone: 'Asia/Shanghai',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      hour12: false
    }).format(new Date(value));
  };

  const interviewStatusLabel = (state: InterviewPreparationState | null) => {
    if (state?.status === 'fresh') return '内容最新';
    if (state?.status === 'stale') return '数据已变化';
    return '尚未生成';
  };

  const mostRecentKeyword = (items: ReportKeyword[]) => {
    const realKeywords = items.filter((item) => item.key !== HISTORICAL_UNCLASSIFIED_KEY);
    const candidates = realKeywords.length > 0 ? realKeywords : items;
    return candidates.reduce<ReportKeyword | null>((latest, item) => {
      if (!latest) return item;
      const itemTime = Date.parse(item.lastSeen);
      const latestTime = Date.parse(latest.lastSeen);
      if (Number.isNaN(latestTime)) return item;
      if (Number.isNaN(itemTime)) return latest;
      return itemTime > latestTime ? item : latest;
    }, null);
  };

  async function loadKeywords() {
    keywordsLoading = true;
    keywordsError = '';
    try {
      keywords = await reportBackend.listReportKeywords();
      const latest = mostRecentKeyword(keywords);
      latestKeywordKey = latest?.key ?? '';
      const currentUrl = typeof window === 'undefined' ? null : new URL(window.location.href);
      const requested =
        currentUrl?.pathname === '/reports' ? currentUrl.searchParams.getAll('keyword') : [];
      const validRequested = keywords
        .filter((keyword) => requested.includes(keyword.key))
        .map((keyword) => keyword.key);
      selectedKeywordKeys = validRequested.length > 0 ? validRequested : latest ? [latest.key] : [];
      keywordsLoading = false;
      if (selectedKeywordKeys.length > 0) {
        syncReportUrl();
        await Promise.all([loadReport(), loadInterviewPreparation(), loadReportCompetitiveness()]);
      } else {
        report = null;
        interviewState = null;
        competitivenessState = null;
        loading = false;
        interviewLoading = false;
        competitivenessLoading = false;
      }
    } catch (reason) {
      keywordsError = reason instanceof Error ? reason.message : String(reason);
      report = null;
      interviewState = null;
      competitivenessState = null;
      loading = false;
      interviewLoading = false;
      competitivenessLoading = false;
    } finally {
      keywordsLoading = false;
    }
  }

  async function loadReport() {
    const keywordKeys = [...selectedKeywordKeys];
    const requestId = ++reportRequestId;
    if (keywordKeys.length === 0) {
      report = null;
      loading = false;
      error = '';
      return;
    }
    loading = true;
    error = '';
    try {
      const nextReport = await reportBackend.getJobDataReport(keywordKeys);
      if (requestId === reportRequestId) report = nextReport;
    } catch (reason) {
      if (requestId === reportRequestId)
        error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      if (requestId === reportRequestId) loading = false;
    }
  }

  async function loadInterviewPreparation() {
    const keywordKeys = [...selectedKeywordKeys];
    const requestId = ++interviewRequestId;
    if (keywordKeys.length === 0) {
      interviewState = null;
      interviewLoading = false;
      interviewError = '';
      return;
    }
    interviewLoading = true;
    interviewError = '';
    try {
      const nextState = await reportBackend.getInterviewPreparationState(keywordKeys);
      if (requestId === interviewRequestId) interviewState = nextState;
    } catch (reason) {
      if (requestId === interviewRequestId)
        interviewError = reason instanceof Error ? reason.message : String(reason);
    } finally {
      if (requestId === interviewRequestId) interviewLoading = false;
    }
  }

  async function loadReportCompetitiveness() {
    const keywordKeys = [...selectedKeywordKeys];
    const requestId = ++competitivenessRequestId;
    if (keywordKeys.length === 0) {
      competitivenessState = null;
      competitivenessLoading = false;
      competitivenessError = '';
      return;
    }
    competitivenessLoading = true;
    competitivenessError = '';
    try {
      const nextState = await reportBackend.getReportCompetitivenessState(keywordKeys);
      if (requestId === competitivenessRequestId) competitivenessState = nextState;
    } catch (reason) {
      if (requestId === competitivenessRequestId)
        competitivenessError = reason instanceof Error ? reason.message : String(reason);
    } finally {
      if (requestId === competitivenessRequestId) competitivenessLoading = false;
    }
  }

  function toggleKeyword(key: string) {
    const selected = new Set(selectedKeywordKeys);
    if (selected.has(key)) selected.delete(key);
    else selected.add(key);
    selectedKeywordKeys = keywords
      .filter((keyword) => selected.has(keyword.key))
      .map((keyword) => keyword.key);
    report = null;
    interviewState = null;
    competitivenessState = null;
    exportMessage = '';
    syncReportUrl();
    void loadReport();
    void loadInterviewPreparation();
    void loadReportCompetitiveness();
  }

  function syncReportUrl() {
    if (typeof window === 'undefined') return;
    const url = new URL(window.location.href);
    if (url.pathname !== '/reports') return;
    url.searchParams.delete('keyword');
    url.searchParams.delete('window');
    selectedKeywordKeys.forEach((key) => url.searchParams.append('keyword', key));
    const nextUrl = `${url.pathname}${url.search}${url.hash}`;
    try {
      replaceState(nextUrl, {});
    } catch {
      window.history.replaceState(window.history.state, '', nextUrl);
    }
  }

  async function generateReportCompetitiveness() {
    if (selectedKeywordKeys.length === 0) return;
    const requestId = ++competitivenessRequestId;
    competitivenessGenerating = true;
    competitivenessError = '';
    try {
      const nextState = await reportBackend.generateReportCompetitiveness(
        [...selectedKeywordKeys],
        competitivenessState?.status !== 'missing'
      );
      if (requestId === competitivenessRequestId) competitivenessState = nextState;
    } catch (reason) {
      if (requestId === competitivenessRequestId)
        competitivenessError = reason instanceof Error ? reason.message : String(reason);
    } finally {
      competitivenessGenerating = false;
    }
  }

  async function generateInterviewPreparation() {
    if (selectedKeywordKeys.length === 0) return;
    const keywordKeys = [...selectedKeywordKeys];
    const requestId = ++interviewRequestId;
    interviewGenerating = true;
    interviewError = '';
    try {
      const nextState = await reportBackend.generateInterviewPreparation(
        keywordKeys,
        interviewState?.status !== 'missing'
      );
      if (requestId === interviewRequestId) interviewState = nextState;
    } catch (reason) {
      if (requestId === interviewRequestId)
        interviewError = reason instanceof Error ? reason.message : String(reason);
    } finally {
      interviewGenerating = false;
    }
  }

  async function exportReport() {
    if (selectedKeywordKeys.length === 0) return;
    exporting = true;
    exportMessage = '';
    try {
      const fileName = `岗位数据报告_${localExportStamp()}.html`;
      const outputPath = await chooseLocalExportPath({
        title: '导出岗位数据报告',
        fileName,
        filterName: 'HTML 报告',
        extension: 'html'
      });
      if (!outputPath) return;
      const result = await reportBackend.exportJobDataReport([...selectedKeywordKeys], outputPath);
      exportMessage = `已导出：${result.path}`;
    } catch (reason) {
      exportMessage = reason instanceof Error ? reason.message : String(reason);
    } finally {
      exporting = false;
    }
  }

  onMount(() => {
    void loadKeywords();
  });

  function drilldownHref(
    filters: {
      skills?: string[];
      city?: string;
      experience?: string;
      salaryBand?: ReportSalaryBand;
    } = {}
  ) {
    const params = new URLSearchParams({ from: 'report' });
    selectedKeywordKeys.forEach((key) => params.append('keyword', key));
    filters.skills?.forEach((skill) => params.append('skill', skill));
    if (filters.city) params.set('city', filters.city);
    if (filters.experience) params.set('experience', filters.experience);
    if (filters.salaryBand) params.set('salaryBand', filters.salaryBand);
    return `/jobs?${params.toString()}`;
  }

  const skillHref = (row: ReportBucket) => drilldownHref({ skills: [row.label] });
  const skillPairHref = (row: ReportBucket) =>
    drilldownHref({
      skills: row.label
        .split(' × ')
        .map((item) => item.trim())
        .filter(Boolean)
    });
  const cityHref = (row: ReportBucket) => drilldownHref({ city: row.label });
  const experienceHref = (row: ReportBucket) => drilldownHref({ experience: row.label });
  const salaryBandCode = (label: string): ReportSalaryBand =>
    (({
      '15K 以下': 'under-15',
      '15–25K': '15-25',
      '25–35K': '25-35',
      '35–50K': '35-50',
      '50K 以上': '50-plus'
    })[label] as ReportSalaryBand) ?? '';
  const salaryBandHref = (row: ReportBucket) =>
    drilldownHref({ salaryBand: salaryBandCode(row.label) });
  const signed = (value?: number | null, suffix = '') =>
    value == null ? '暂无可比数据' : `${value > 0 ? '+' : ''}${value.toFixed(1)}${suffix}`;
  const competitivenessStatusLabel = (status: string) =>
    ({ covered: '已覆盖', strengthenable: '可强化', gap: '真实缺口', unknown: '待判断' })[status] ??
    '待判断';
  const batchUnavailableReason = (reason?: string | null) =>
    ({
      multi_keyword: '当前选择了多个关键词。多关键词并集只展示当前样本，不做批次涨跌比较。',
      no_captured_run: '当前范围没有包含搜索条件和样本摘要的新抓取批次。历史记录不会被猜测或回填。',
      no_comparable_run:
        '尚未找到跨上海时区不同日期、且关键词、城市、页数和筛选条件完全一致的前一成功批次。'
    })[reason ?? ''] ?? '当前没有满足比较规则的成功批次。';
  const resumeMarketHref = (focusSkill?: string) => {
    const params = new URLSearchParams({ assistant: '1', market: '1' });
    selectedKeywordKeys.forEach((key) => params.append('keyword', key));
    if (focusSkill) params.append('focusSkill', focusSkill);
    return `/resume?${params.toString()}`;
  };
</script>

<div class="page-content report-page">
  <header class="mb-6 flex items-start justify-between gap-5">
    <div>
      <p class="eyebrow">Keyword groups · Local analytics</p>
      <h2 class="page-title mt-1">岗位数据报告</h2>
      <p class="mt-2 max-w-2xl text-sm leading-6 body-muted">
        统计、AI 面试准备和导出报告始终使用同一组关键词；多选时按岗位并集去重。
      </p>
    </div>
    <div class="flex shrink-0 gap-2">
      <button
        class="btn"
        on:click={() => {
          void loadReport();
          void loadInterviewPreparation();
          void loadReportCompetitiveness();
        }}
        disabled={loading ||
          interviewGenerating ||
          competitivenessGenerating ||
          selectedKeywordKeys.length === 0}
        ><RefreshCw size={15} class={loading ? 'animate-spin' : ''} />刷新当前范围</button
      >
      <button
        class="btn-primary"
        on:click={exportReport}
        disabled={exporting || selectedKeywordKeys.length === 0 || !report?.totalJobs}
        ><Download size={15} />{exporting ? '正在导出' : '导出 HTML'}</button
      >
    </div>
  </header>

  <section class="panel mb-5 p-5" aria-labelledby="keyword-filter-title">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <h3 id="keyword-filter-title" class="section-title">报告关键词</h3>
        <p class="mt-1 text-xs leading-5 body-muted">
          可选择一个或多个抓取关键词，默认选中最近一次抓取。
        </p>
      </div>
      {#if selectedKeywordLabels.length > 0}<p class="text-xs body-muted">
          当前范围：<span class="font-semibold text-ink">{selectedKeywordLabels.join('、')}</span>
        </p>{/if}
    </div>

    {#if keywordsLoading}
      <div class="mt-4 grid grid-cols-3 gap-3" aria-label="正在读取报告关键词">
        {#each [1, 2, 3] as _}<div class="skeleton h-12 rounded-xl"></div>{/each}
      </div>
    {:else if keywordsError}
      <div
        class="mt-4 flex items-center justify-between gap-4 rounded-xl border px-4 py-3"
        role="alert"
        style="border-color: color-mix(in srgb, #c53030 30%, var(--line)); background: color-mix(in srgb, #c53030 6%, var(--panel));"
      >
        <p class="text-xs leading-5 body-muted">读取关键词失败：{keywordsError}</p>
        <button class="btn shrink-0" on:click={loadKeywords}>重试</button>
      </div>
    {:else if keywords.length > 0}
      <div class="mt-4 flex flex-wrap gap-2.5">
        {#each keywords as keyword}
          <label
            class:selected-keyword={selectedKeywordKeys.includes(keyword.key)}
            class="keyword-option flex cursor-pointer items-center gap-2 rounded-xl border px-3 py-2.5 transition"
            style="border-color: var(--line);"
          >
            <input
              type="checkbox"
              checked={selectedKeywordKeys.includes(keyword.key)}
              on:change={() => toggleKeyword(keyword.key)}
              class="h-4 w-4 accent-[var(--brand)]"
            />
            <span class="text-sm font-medium">{keyword.label}</span>
            <span class="text-[11px] body-muted">{keyword.jobCount} 个岗位</span>
            {#if keyword.key === latestKeywordKey}<span class="chip-brand px-2 py-0.5 text-[10px]"
                >最近抓取</span
              >{/if}
          </label>
        {/each}
      </div>
    {:else}
      <p class="mt-4 text-xs body-muted">
        暂无抓取关键词。完成一次岗位抓取后即可按关键词生成报告。
      </p>
    {/if}
  </section>

  {#if exportMessage}
    <div
      class="mb-5 rounded-xl border px-4 py-3 text-xs leading-5"
      style="border-color: var(--line); background: var(--brand-faint);"
    >
      {exportMessage}
    </div>
  {/if}

  {#if !keywordsLoading && selectedKeywordKeys.length === 0}
    <section class="panel grid min-h-72 place-items-center p-8 text-center">
      <div class="max-w-md">
        <BarChart3 size={28} class="mx-auto mb-3 text-brand" />
        <h3 class="section-title">
          {keywords.length > 0 ? '请至少选择一个关键词' : '还没有可用的报告关键词'}
        </h3>
        <p class="mt-2 text-sm leading-6 body-muted">
          {keywords.length > 0
            ? '选择后才会生成统计、读取对应的 AI 建议并启用导出。'
            : '先完成一次岗位抓取，再返回这里查看报告。'}
        </p>
        {#if keywords.length === 0}<a href="/jobs" class="btn-primary mt-5">前往岗位库</a>{/if}
      </div>
    </section>
  {:else}
    <section class="panel mb-7 overflow-hidden" aria-labelledby="interview-preparation-title">
      <div
        class="flex flex-wrap items-start justify-between gap-4 border-b p-6"
        style="border-color: var(--line);"
      >
        <div class="flex min-w-0 gap-3">
          <span
            class="grid h-10 w-10 shrink-0 place-items-center rounded-xl"
            style="background: var(--brand-soft); color: var(--brand);"
            ><BookOpenCheck size={20} /></span
          >
          <div>
            <div class="flex flex-wrap items-center gap-2">
              <h3 id="interview-preparation-title" class="section-title">AI 面试准备</h3>
              {#if interviewState}
                <span
                  class:stale-badge={interviewState.status === 'stale'}
                  class:fresh-badge={interviewState.status === 'fresh'}
                  class="status-badge">{interviewStatusLabel(interviewState)}</span
                >
              {/if}
            </div>
            <p class="mt-1 text-sm leading-6 body-muted">
              结合本地岗位需求与已确认的简历事实，整理技能差距、准备动作和练习问题。
            </p>
          </div>
        </div>

        {#if !interviewLoading && interviewState?.hasProvider && interviewState.reason !== 'no_jobs'}
          <button
            class="btn-primary shrink-0"
            on:click={generateInterviewPreparation}
            disabled={interviewGenerating}
          >
            <Sparkles size={15} class={interviewGenerating ? 'animate-pulse' : ''} />
            {#if interviewGenerating}
              正在生成
            {:else if interviewState.status === 'stale'}
              刷新建议
            {:else if interviewState.preparation}
              重新生成
            {:else}
              生成 AI 面试准备
            {/if}
          </button>
        {/if}
      </div>

      {#if interviewLoading && !interviewState}
        <div class="space-y-3 p-6" aria-label="正在读取面试准备状态">
          <div class="skeleton h-5 w-2/3 rounded-lg"></div>
          <div class="skeleton h-20 rounded-xl"></div>
        </div>
      {:else if !interviewState}
        <div class="p-6">
          <div
            class="flex items-start gap-3 rounded-xl border px-4 py-3 text-sm"
            style="border-color: var(--line);"
          >
            <AlertCircle size={17} class="mt-0.5 shrink-0 text-danger" />
            <div>
              <p class="font-semibold">暂时无法读取 AI 准备状态</p>
              <p class="mt-1 body-muted">{interviewError || '请稍后重试。'}</p>
              <button class="btn mt-3" on:click={loadInterviewPreparation}>重试</button>
            </div>
          </div>
        </div>
      {:else}
        <div class="p-6">
          {#if (!interviewState.hasProvider || interviewState.reason === 'no_provider') && !interviewState.preparation}
            <div
              class="flex flex-wrap items-center justify-between gap-4 rounded-xl border p-4"
              style="border-color: var(--line); background: var(--panel-soft);"
            >
              <div>
                <p class="text-sm font-semibold">先配置并验证 AI 模型</p>
                <p class="mt-1 text-xs leading-5 body-muted">
                  面试准备只会在你点击生成时调用已选中的模型服务。
                </p>
              </div>
              <a class="btn-primary" href="/settings">前往模型设置</a>
            </div>
          {:else if interviewState.reason === 'no_jobs'}
            <div
              class="flex items-start gap-3 rounded-xl border p-4"
              style="border-color: var(--line); background: var(--panel-soft);"
            >
              <Database size={18} class="mt-0.5 shrink-0 text-brand" />
              <div>
                <p class="text-sm font-semibold">需要岗位数据后才能生成</p>
                <p class="mt-1 text-xs leading-5 body-muted">
                  完成一次岗位抓取后，这里会基于本地聚合结果准备面试建议。
                </p>
              </div>
            </div>
          {:else}
            {#if !interviewState.hasProvider || interviewState.reason === 'no_provider'}
              <div
                class="mb-5 flex flex-wrap items-center justify-between gap-4 rounded-xl border p-4"
                style="border-color: var(--line); background: var(--panel-soft);"
              >
                <div>
                  <p class="text-sm font-semibold">当前模型不可用，正在展示上一次结果</p>
                  <p class="mt-1 text-xs leading-5 body-muted">
                    配置并验证默认模型后，才能刷新这份已过期的准备建议。
                  </p>
                </div>
                <a class="btn-primary" href="/settings">前往模型设置</a>
              </div>
            {/if}

            {#if !interviewState.hasResume}
              <div
                class="mb-5 flex items-start gap-3 rounded-xl border px-4 py-3"
                style="border-color: var(--line); background: var(--brand-faint);"
              >
                <Target size={17} class="mt-0.5 shrink-0 text-brand" />
                <div>
                  <p class="text-sm font-semibold">当前为通用市场模式</p>
                  <p class="mt-1 text-xs leading-5 body-muted">
                    尚未配置主简历，将只根据岗位聚合数据生成通用准备建议，不会推断个人经历或差距。
                  </p>
                </div>
              </div>
            {/if}

            {#if interviewState.status === 'stale'}
              <div
                class="mb-5 flex items-start gap-3 rounded-xl border px-4 py-3"
                style="border-color: color-mix(in srgb, #b7791f 35%, var(--line)); background: color-mix(in srgb, #f6ad55 9%, var(--panel));"
              >
                <Clock3 size={17} class="mt-0.5 shrink-0 text-warning" />
                <div>
                  <p class="text-sm font-semibold">岗位、简历或模型配置已发生变化</p>
                  <p class="mt-1 text-xs leading-5 body-muted">
                    下方保留的是上一次结果。点击“刷新建议”后再将其用于面试准备。
                  </p>
                </div>
              </div>
            {/if}

            {#if interviewState.preparation}
              <div
                class="rounded-2xl p-5"
                style="background: linear-gradient(120deg, var(--brand-faint), var(--panel-soft));"
              >
                <div class="flex items-center justify-between gap-4">
                  <p class="eyebrow">Preparation summary</p>
                  {#if interviewState.generatedAt}<span class="text-[11px] body-muted"
                      >{generatedTime(interviewState.generatedAt)} 生成</span
                    >{/if}
                </div>
                <p class="mt-2 text-sm leading-7">{interviewState.preparation.summary}</p>
              </div>

              {#if interviewState.preparation.skills.length > 0}
                <div class="mt-6">
                  <div class="mb-3 flex items-center gap-2">
                    <Target size={16} class="text-brand" />
                    <h4 class="text-sm font-semibold">优先技能与准备动作</h4>
                  </div>
                  <div class="grid grid-cols-2 gap-3">
                    {#each interviewState.preparation.skills.slice(0, 8) as skill, index}
                      <article class="rounded-xl border p-4" style="border-color: var(--line);">
                        <div class="flex items-center justify-between gap-3">
                          <div class="flex min-w-0 items-center gap-2">
                            <span
                              class="grid h-6 w-6 shrink-0 place-items-center rounded-full text-[11px] font-semibold"
                              style="background: var(--brand-soft); color: var(--brand);"
                              >{index + 1}</span
                            >
                            <h5 class="truncate text-sm font-semibold">{skill.name}</h5>
                          </div>
                          {#if skill.jobCount != null}<span class="shrink-0 text-[11px] body-muted"
                              >{skill.jobCount} 个岗位提及</span
                            >{/if}
                        </div>
                        {#if skill.gap}<p class="mt-3 text-xs leading-5 body-muted">
                            {interviewState.hasResume ? '差距' : '市场期待'}：{skill.gap}
                          </p>{/if}
                        <p class="mt-2 text-xs leading-5">
                          <span class="font-semibold text-brand">准备：</span>{skill.action}
                        </p>
                      </article>
                    {/each}
                  </div>
                </div>
              {/if}

              <div class="mt-6 grid grid-cols-2 gap-5">
                <article class="rounded-xl border p-5" style="border-color: var(--line);">
                  <div class="mb-4 flex items-center gap-2">
                    <Lightbulb size={16} class="text-brand" />
                    <h4 class="text-sm font-semibold">可准备的项目案例</h4>
                  </div>
                  {#if interviewState.preparation.projectIdeas.length > 0}
                    <ol class="space-y-3">
                      {#each interviewState.preparation.projectIdeas as idea, index}<li
                          class="flex gap-3 text-xs leading-5"
                        >
                          <span class="body-muted">{index + 1}.</span><span>{idea}</span>
                        </li>{/each}
                    </ol>
                  {:else}<p class="text-xs body-muted">
                      当前数据不足以给出可靠的项目案例建议。
                    </p>{/if}
                </article>
                <article class="rounded-xl border p-5" style="border-color: var(--line);">
                  <div class="mb-4 flex items-center gap-2">
                    <MessageCircleQuestion size={16} class="text-brand" />
                    <h4 class="text-sm font-semibold">练习问题</h4>
                  </div>
                  {#if interviewState.preparation.practiceQuestions.length > 0}
                    <ol class="space-y-3">
                      {#each interviewState.preparation.practiceQuestions as question, index}<li
                          class="flex gap-3 text-xs leading-5"
                        >
                          <span class="body-muted">{index + 1}.</span><span>{question}</span>
                        </li>{/each}
                    </ol>
                  {:else}<p class="text-xs body-muted">当前数据不足以生成可靠的练习问题。</p>{/if}
                </article>
              </div>
            {:else}
              <div
                class="grid min-h-40 place-items-center rounded-xl border border-dashed p-6 text-center"
                style="border-color: var(--line);"
              >
                <div class="max-w-lg">
                  <Sparkles size={22} class="mx-auto text-brand" />
                  <p class="mt-3 text-sm font-semibold">按需生成面试准备建议</p>
                  <p class="mt-1 text-xs leading-5 body-muted">
                    不会自动调用模型。点击右上角按钮后，只发送岗位聚合数据{interviewState.hasResume
                      ? '与已确认的简历事实'
                      : ''}。
                  </p>
                </div>
              </div>
            {/if}
          {/if}

          {#if interviewError}
            <div
              class="mt-5 flex items-start gap-3 rounded-xl border px-4 py-3"
              role="alert"
              style="border-color: color-mix(in srgb, #c53030 30%, var(--line)); background: color-mix(in srgb, #c53030 6%, var(--panel));"
            >
              <AlertCircle size={17} class="mt-0.5 shrink-0 text-danger" />
              <div>
                <p class="text-sm font-semibold">AI 面试准备生成失败</p>
                <p class="mt-1 text-xs leading-5 body-muted">
                  {interviewError} 本地统计和已有建议均未受影响。
                </p>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </section>

    {#if loading}
      <div class="space-y-5">
        <div class="skeleton h-40 rounded-2xl"></div>
        <div class="grid grid-cols-4 gap-4">
          {#each [1, 2, 3, 4] as _}<div class="skeleton h-28 rounded-2xl"></div>{/each}
        </div>
        <div class="grid grid-cols-2 gap-5">
          <div class="skeleton h-80 rounded-2xl"></div>
          <div class="skeleton h-80 rounded-2xl"></div>
        </div>
      </div>
    {:else if error}
      <section class="panel grid min-h-72 place-items-center p-8 text-center">
        <div>
          <Database size={28} class="mx-auto mb-3 text-brand" />
          <h3 class="section-title">本地报告生成失败</h3>
          <p class="mt-2 text-sm body-muted">{error}</p>
          <button class="btn mt-5" on:click={loadReport}>重试</button>
        </div>
      </section>
    {:else if !report || report.totalJobs === 0}
      <section class="panel grid min-h-[420px] place-items-center p-8 text-center">
        <div class="max-w-md">
          <span
            class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl"
            style="background: var(--brand-soft); color: var(--brand);"
            ><BarChart3 size={25} /></span
          >
          <h3 class="text-lg font-semibold">所选关键词暂无岗位数据</h3>
          <p class="mt-2 text-sm leading-6 body-muted">
            可以调整上方关键词，或完成新一轮岗位抓取。
          </p>
          <a href="/jobs" class="btn-primary mt-5">前往岗位库</a>
        </div>
      </section>
    {:else}
      <section
        class="relative overflow-hidden rounded-[24px] border p-7 shadow-panel"
        style="border-color: var(--line); background: linear-gradient(120deg, var(--panel), var(--brand-faint));"
      >
        <div class="dot-grid pointer-events-none absolute inset-y-0 right-0 w-1/3 opacity-40"></div>
        <div class="relative flex items-end justify-between gap-8">
          <div>
            <div
              class="mb-3 inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-semibold"
              style="background: var(--brand-soft); color: var(--brand);"
            >
              <CheckCircle2 size={13} />本次岗位样本观察
            </div>
            <h3 class="text-[24px] font-semibold tracking-[-0.03em]">
              从当前 {report.totalJobs} 个本地去重样本看反复出现的要求
            </h3>
            <p class="mt-2 text-sm body-muted">
              关键词 {selectedKeywordLabels.join('、')} · 数据范围 {report.dataFrom ?? '未知'} 至 {report.dataTo ??
                '未知'} · {generatedTime(report.generatedAt)} 生成
            </p>
          </div>
          <div class="flex items-center gap-2 text-xs body-muted">
            <CalendarRange size={15} />Asia/Shanghai
          </div>
        </div>
      </section>

      <section class="mt-5 grid grid-cols-4 gap-4">
        <article class="panel-flat p-5">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-xs font-semibold body-muted">本地去重岗位样本</span><Database
              size={17}
              class="text-brand"
            />
          </div>
          <strong class="text-[27px] font-semibold tabular-nums">{report.totalJobs}</strong>
          <p class="mt-1 text-xs body-muted">按岗位 ID 去重</p>
        </article>
        <article class="panel-flat p-5">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-xs font-semibold body-muted">招聘公司</span><Building2
              size={17}
              class="text-brand"
            />
          </div>
          <strong class="text-[27px] font-semibold tabular-nums">{report.totalCompanies}</strong>
          <p class="mt-1 text-xs body-muted">覆盖 {report.totalCities} 个城市</p>
        </article>
        <article class="panel-flat p-5">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-xs font-semibold body-muted">月薪中点中位数</span><WalletCards
              size={17}
              class="text-brand"
            />
          </div>
          <strong class="text-[27px] font-semibold tabular-nums"
            >{salary(report.salary.medianMidK)}</strong
          >
          <p class="mt-1 text-xs body-muted">{report.salary.sampleCount} 个可解析样本</p>
        </article>
        <article class="panel-flat p-5">
          <div class="mb-4 flex items-center justify-between">
            <span class="text-xs font-semibold body-muted">岗位详情覆盖</span><FileCheck2
              size={17}
              class="text-brand"
            />
          </div>
          <strong class="text-[27px] font-semibold tabular-nums"
            >{report.detailCoverage.toFixed(1)}%</strong
          >
          <p class="mt-1 text-xs body-muted">{report.detailJobs} 个岗位含 JD</p>
        </article>
      </section>

      <section class="panel mt-5 p-6" aria-labelledby="sample-quality-title">
        <div class="mb-5">
          <p class="eyebrow">Sample quality</p>
          <h3 id="sample-quality-title" class="section-title mt-1">样本质量与限制</h3>
          <p class="mt-1 text-xs leading-5 body-muted">
            覆盖率只描述当前本地样本中有多少岗位具备对应字段，不会通过再次访问 BOSS 补齐。
          </p>
        </div>
        <div class="grid grid-cols-5 gap-3">
          {#each [{ label: '岗位详情', metric: report.sampleQuality.detail }, { label: '薪资', metric: report.sampleQuality.salary }, { label: '技能', metric: report.sampleQuality.skill }, { label: '经验', metric: report.sampleQuality.experience }, { label: '学历', metric: report.sampleQuality.degree }] as item}
            <article class="rounded-xl p-4 surface-soft">
              <span class="text-xs body-muted">{item.label}</span><strong
                class="mt-2 block text-xl tabular-nums">{item.metric.coverage.toFixed(1)}%</strong
              ><span class="text-[11px] body-muted"
                >{item.metric.count} / {report.totalJobs} 个样本</span
              >
            </article>
          {/each}
        </div>
        <div class="mt-5 rounded-xl border px-4 py-3" style="border-color: var(--line);">
          <ul class="space-y-2">
            {#each report.sampleQuality.limitations as limitation}<li
                class="flex gap-2 text-xs leading-5 body-muted"
              >
                <AlertCircle size={14} class="mt-0.5 shrink-0 text-warning" /><span
                  >{limitation}</span
                >
              </li>{/each}
          </ul>
        </div>
      </section>

      <section class="panel mt-5 p-6" aria-labelledby="batch-comparison-title">
        <div class="mb-5">
          <p class="eyebrow">Comparable batches</p>
          <h3 id="batch-comparison-title" class="section-title mt-1">最近两次同条件样本对比</h3>
          <p class="mt-1 text-xs leading-5 body-muted">
            只比较跨不同日期、搜索范围完全一致的成功抓取批次；不会为生成报告额外访问 BOSS。
          </p>
        </div>
        {#if report.batchComparison.status === 'available' && report.batchComparison.current && report.batchComparison.previous}
          <div
            class="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border px-4 py-3 text-xs"
            style="border-color: var(--line);"
          >
            <span
              ><strong>{generatedTime(report.batchComparison.previous.completedAt)}</strong> →
              <strong>{generatedTime(report.batchComparison.current.completedAt)}</strong></span
            >
            <span class="body-muted"
              >{report.batchComparison.current.searchSpec.city} · {report.batchComparison.current
                .searchSpec.pages} 页 · 其余筛选条件一致</span
            >
          </div>
          <div class="grid grid-cols-4 gap-3">
            <article class="rounded-xl p-4 surface-soft">
              <span class="text-xs body-muted">本次岗位数</span><strong
                class="mt-2 block text-2xl tabular-nums"
                >{report.batchComparison.current.totalJobs}</strong
              ><span class="text-[11px] body-muted"
                >前次 {report.batchComparison.previous.totalJobs} 个</span
              >
            </article>
            <article class="rounded-xl p-4 surface-soft">
              <span class="text-xs body-muted">岗位数量变化</span><strong
                class="mt-2 block text-2xl tabular-nums"
                >{signed(report.batchComparison.jobCountChangePercentage, '%')}</strong
              ><span class="text-[11px] body-muted">仅限两个有限样本</span>
            </article>
            <article class="rounded-xl p-4 surface-soft">
              <span class="text-xs body-muted">本次新出现</span><strong
                class="mt-2 block text-2xl tabular-nums"
                >{report.batchComparison.newlyObservedJobs}</strong
              ><span class="text-[11px] body-muted">相对前次岗位 ID 集合</span>
            </article>
            <article class="rounded-xl p-4 surface-soft">
              <span class="text-xs body-muted">本次有限结果未再次出现</span><strong
                class="mt-2 block text-2xl tabular-nums"
                >{report.batchComparison.notObservedJobs}</strong
              ><span class="text-[11px] body-muted">不代表岗位已经下架</span>
            </article>
          </div>
          <div class="mt-5 grid grid-cols-2 gap-5">
            <article class="rounded-xl border p-4" style="border-color: var(--line);">
              <span class="text-xs body-muted">薪资中点中位数变化</span><strong
                class="mt-2 block text-xl tabular-nums"
                >{signed(report.batchComparison.salaryMedianDeltaK, 'K')}</strong
              >
              <p class="mt-1 text-[11px] body-muted">
                本次 {salary(report.batchComparison.current.medianSalaryK)} · 前次 {salary(
                  report.batchComparison.previous.medianSalaryK
                )}
              </p>
            </article>
            <article class="rounded-xl border p-4" style="border-color: var(--line);">
              <h4 class="mb-3 text-sm font-semibold">技能样本占比变化</h4>
              {#if report.batchComparison.skillChanges.length > 0}<div class="space-y-2">
                  {#each report.batchComparison.skillChanges as item}<a
                      href={drilldownHref({ skills: [item.label] })}
                      class="flex items-center justify-between gap-3 text-xs"
                      ><span
                        >{item.label}
                        <span class="body-muted"
                          >{item.currentCount}/{report.batchComparison.current.totalJobs}</span
                        ></span
                      ><span
                        class:item-rise={item.deltaPercentagePoints > 0}
                        class:item-fall={item.deltaPercentagePoints < 0}
                        class="tabular-nums">{signed(item.deltaPercentagePoints, 'pp')}</span
                      ></a
                    >{/each}
                </div>{:else}<p class="text-xs body-muted">
                  两个批次没有足够的技能摘要可比较。
                </p>{/if}
            </article>
          </div>
        {:else}
          <div
            class="flex items-start gap-3 rounded-xl border border-dashed p-5"
            style="border-color: var(--line);"
          >
            <CalendarRange size={18} class="mt-0.5 shrink-0 text-brand" />
            <div>
              <p class="text-sm font-semibold">当前不生成批次涨跌结论</p>
              <p class="mt-1 text-xs leading-5 body-muted">
                {batchUnavailableReason(report.batchComparison.reason)}
              </p>
            </div>
          </div>
        {/if}
      </section>

      <section class="panel mt-5 overflow-hidden" aria-labelledby="competitiveness-title">
        <div
          class="flex flex-wrap items-start justify-between gap-4 border-b p-6"
          style="border-color: var(--line);"
        >
          <div class="flex gap-3">
            <span
              class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"
              ><TrendingUp size={20} /></span
            >
            <div>
              <div class="flex flex-wrap items-center gap-2">
                <h3 id="competitiveness-title" class="section-title">市场需求 × 我的简历</h3>
                {#if competitivenessState}<span
                    class:stale-badge={competitivenessState.status === 'stale'}
                    class:fresh-badge={competitivenessState.status === 'fresh'}
                    class="status-badge"
                    >{competitivenessState.effectiveSource === 'ai'
                      ? 'AI 语义复核'
                      : competitivenessState.status === 'stale'
                        ? 'AI 结果已过期'
                        : '本地精确匹配'}</span
                  >{/if}
              </div>
              <p class="mt-1 text-sm leading-6 body-muted">
                用高频技能对照主简历正文和已确认事实；本地结果始终可用，AI 仅在主动点击后调用。
              </p>
            </div>
          </div>
          <div class="flex flex-wrap gap-2">
            {#if competitivenessState?.hasResume && marketContextEligible}<a
                class="btn"
                href={resumeMarketHref()}>基于当前样本优化主简历</a
              >{:else if competitivenessState?.hasResume}<span
                class="self-center text-xs body-muted">主简历优化最多选择 8 个关键词</span
              >{/if}
            {#if competitivenessState?.hasResume && competitivenessState.hasProvider && competitivenessState.reason !== 'no_jobs'}<button
                class="btn-primary shrink-0"
                on:click={generateReportCompetitiveness}
                disabled={competitivenessGenerating}
                ><Sparkles
                  size={15}
                  class={competitivenessGenerating ? 'animate-pulse' : ''}
                />{competitivenessGenerating
                  ? '正在分析'
                  : competitivenessState.status === 'fresh'
                    ? '重新运行 AI'
                    : 'AI 语义分析'}</button
              >{/if}
          </div>
        </div>
        {#if competitivenessLoading && !competitivenessState}
          <div class="grid grid-cols-2 gap-3 p-6">
            {#each [1, 2, 3, 4] as _}<div class="skeleton h-28 rounded-xl"></div>{/each}
          </div>
        {:else if !competitivenessState}
          <div class="p-6">
            <div class="rounded-xl border p-4 text-sm" style="border-color: var(--line);">
              <p class="font-semibold">暂时无法读取竞争力分析</p>
              <p class="mt-1 text-xs body-muted">{competitivenessError || '请稍后重试。'}</p>
              <button class="btn mt-3" on:click={loadReportCompetitiveness}>重试</button>
            </div>
          </div>
        {:else if !competitivenessState.hasResume}
          <div class="flex flex-wrap items-center justify-between gap-4 p-6">
            <div>
              <p class="text-sm font-semibold">先建立可信主简历</p>
              <p class="mt-1 text-xs leading-5 body-muted">
                导入或创建主简历后，才能判断已覆盖、可强化和真实缺口。
              </p>
            </div>
            <a href="/resume" class="btn-primary">前往简历</a>
          </div>
        {:else}
          <div class="p-6">
            {#if !competitivenessState.hasProvider}<div
                class="mb-5 flex flex-wrap items-center justify-between gap-4 rounded-xl border p-4"
                style="border-color: var(--line); background: var(--panel-soft);"
              >
                <div>
                  <p class="text-sm font-semibold">当前展示本地精确匹配</p>
                  <p class="mt-1 text-xs body-muted">
                    配置并验证模型后，可以主动运行一次语义复核。
                  </p>
                </div>
                <a href="/settings" class="btn">配置语义分析模型</a>
              </div>{/if}
            {#if competitivenessState.status === 'stale'}<div
                class="mb-5 flex items-start gap-3 rounded-xl border px-4 py-3"
                style="border-color: color-mix(in srgb, #b7791f 35%, var(--line)); background: color-mix(in srgb, #f6ad55 9%, var(--panel));"
              >
                <Clock3 size={17} class="mt-0.5 shrink-0 text-warning" />
                <div>
                  <p class="text-sm font-semibold">岗位、简历或模型配置已变化</p>
                  <p class="mt-1 text-xs body-muted">
                    旧 AI 结果未用于当前矩阵，下面已自动回退为最新本地结果。
                  </p>
                </div>
              </div>{/if}
            {#if competitiveness?.items.length}
              <div class="mb-4 grid grid-cols-4 gap-3">
                {#each ['covered', 'strengthenable', 'gap', 'unknown'] as status}<div
                    class="rounded-xl p-3 text-center surface-soft"
                  >
                    <strong class="block text-lg"
                      >{competitiveness.items.filter((item) => item.status === status)
                        .length}</strong
                    ><span class="text-[11px] body-muted">{competitivenessStatusLabel(status)}</span
                    >
                  </div>{/each}
              </div>
              <div class="grid grid-cols-2 gap-3">
                {#each competitiveness.items as item}<article
                    class="rounded-xl border p-4"
                    style="border-color: var(--line);"
                  >
                    <div class="flex items-start justify-between gap-3">
                      <div>
                        <div class="flex flex-wrap items-center gap-2">
                          <h4 class="text-sm font-semibold">{item.label}</h4>
                          <span
                            class:item-covered={item.status === 'covered'}
                            class:item-strengthenable={item.status === 'strengthenable'}
                            class:item-gap={item.status === 'gap'}
                            class="status-badge">{competitivenessStatusLabel(item.status)}</span
                          >
                        </div>
                        <p class="mt-1 text-[11px] body-muted">
                          {item.jobCount} 个岗位 · {item.percentage.toFixed(1)}%
                        </p>
                      </div>
                      <a
                        href={drilldownHref({ skills: [item.label] })}
                        class="text-xs font-semibold text-brand">查看岗位</a
                      >
                    </div>
                    <p class="mt-3 text-xs leading-5 body-muted">{item.rationale}</p>
                    {#if marketContextEligible && item.status === 'strengthenable'}<a
                        href={resumeMarketHref(item.label)}
                        class="mt-3 inline-flex text-xs font-semibold text-brand">生成表达优化</a
                      >{:else if marketContextEligible && item.status === 'gap'}<a
                        href={resumeMarketHref(item.label)}
                        class="mt-3 inline-flex text-xs font-semibold text-brand">核对相关经历</a
                      >{/if}
                  </article>{/each}
              </div>
            {:else}<p class="text-sm body-muted">当前岗位缺少可用于竞争力分析的结构化技能。</p>{/if}
            {#if competitivenessError}<div
                class="mt-5 flex items-start gap-3 rounded-xl border px-4 py-3"
                role="alert"
                style="border-color: color-mix(in srgb, #c53030 30%, var(--line)); background: color-mix(in srgb, #c53030 6%, var(--panel));"
              >
                <AlertCircle size={17} class="mt-0.5 shrink-0 text-danger" />
                <div>
                  <p class="text-sm font-semibold">AI 竞争力分析失败</p>
                  <p class="mt-1 text-xs body-muted">
                    {competitivenessError} 当前本地结果未受影响。
                  </p>
                </div>
              </div>{/if}
          </div>
        {/if}
      </section>

      <section class="mt-5 panel p-6">
        <div class="mb-4 flex items-center gap-2">
          <Sparkles size={17} class="text-brand" />
          <h3 class="section-title">先看本地结论</h3>
        </div>
        <div class="grid grid-cols-2 gap-x-8 gap-y-3">
          {#each report.insights as insight}<div class="flex gap-3 text-sm leading-6">
              <span class="mt-2 h-1.5 w-1.5 shrink-0 rounded-full" style="background: var(--brand);"
              ></span>
              <p>{insight}</p>
            </div>{/each}
        </div>
      </section>

      <section class="mt-7">
        <div class="mb-3">
          <p class="eyebrow">Skill demand</p>
          <h3 class="section-title mt-1">技能需求与共现组合</h3>
        </div>
        <div class="grid grid-cols-2 gap-5">
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">岗位需要哪些技能</h4>
            <ReportBars rows={report.topSkills} limit={14} hrefForRow={skillHref} />
          </article>
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">技能共现组合</h4>
            <ReportBars rows={report.skillPairs} limit={10} hrefForRow={skillPairHref} />
          </article>
        </div>
      </section>

      <section class="mt-7">
        <div class="mb-3">
          <p class="eyebrow">Candidate requirements</p>
          <h3 class="section-title mt-1">薪资与候选人门槛</h3>
        </div>
        <div class="grid grid-cols-3 gap-5">
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">薪资区间分布</h4>
            <ReportBars rows={report.salary.bands} hrefForRow={salaryBandHref} />
            <div
              class="mt-5 grid grid-cols-3 gap-2 border-t pt-4 text-center"
              style="border-color: var(--line);"
            >
              <div>
                <strong class="block text-sm">{salary(report.salary.medianLowK)}</strong><span
                  class="text-[11px] body-muted">下限中位数</span
                >
              </div>
              <div>
                <strong class="block text-sm">{salary(report.salary.medianHighK)}</strong><span
                  class="text-[11px] body-muted">上限中位数</span
                >
              </div>
              <div>
                <strong class="block text-sm">{report.salary.extraMonthsCount}</strong><span
                  class="text-[11px] body-muted">13 薪及以上</span
                >
              </div>
            </div>
          </article>
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">经验要求</h4>
            <ReportBars rows={report.experience} hrefForRow={experienceHref} />
          </article>
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">学历要求</h4>
            <ReportBars rows={report.degree} />
          </article>
        </div>
        {#if report.salaryByExperience.length > 0}<article class="panel mt-5 p-6">
            <h4 class="mb-5 text-sm font-semibold">不同经验要求的薪资中位数</h4>
            <div class="grid grid-cols-5 gap-3">
              {#each report.salaryByExperience.slice(0, 5) as item}<a
                  href={drilldownHref({ experience: item.label })}
                  class="rounded-xl p-4 surface-soft transition hover:ring-2 hover:ring-[var(--brand)]"
                  ><span class="text-xs body-muted">{item.label}</span><strong
                    class="mt-2 block text-xl tabular-nums">{item.medianK.toFixed(1)}K</strong
                  ><span class="text-[11px] body-muted">{item.count} 个薪资样本</span></a
                >{/each}
            </div>
          </article>{/if}
      </section>

      <section class="mt-7">
        <div class="mb-3">
          <p class="eyebrow">Market structure</p>
          <h3 class="section-title mt-1">市场结构</h3>
        </div>
        <div class="grid grid-cols-2 gap-5">
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">岗位方向</h4>
            <ReportBars rows={report.roles} />
          </article>
          <article class="panel p-6">
            <h4 class="mb-5 flex items-center gap-2 text-sm font-semibold">
              <MapPinned size={15} class="text-brand" />城市分布
            </h4>
            <ReportBars rows={report.cities} hrefForRow={cityHref} />
          </article>
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">行业分布</h4>
            <ReportBars rows={report.industries} />
          </article>
          <article class="panel p-6">
            <h4 class="mb-5 text-sm font-semibold">公司规模</h4>
            <ReportBars rows={report.companyScales} />
          </article>
          <article class="panel col-span-2 p-6">
            <h4 class="mb-5 text-sm font-semibold">常见福利</h4>
            <div class="max-w-4xl"><ReportBars rows={report.welfare} limit={12} /></div>
          </article>
        </div>
      </section>

      <footer
        class="mt-6 flex items-center justify-between border-t pt-5 text-xs body-muted"
        style="border-color: var(--line);"
      >
        <span>统计频率按“至少一个字段提及该项”的岗位计数，单个岗位不会重复加权。</span><span
          >全部统计在本机完成</span
        >
      </footer>
    {/if}
  {/if}
</div>

<style>
  .status-badge {
    display: inline-flex;
    align-items: center;
    border-radius: 999px;
    padding: 0.2rem 0.55rem;
    background: var(--panel-soft);
    color: var(--muted);
    font-size: 0.68rem;
    font-weight: 600;
  }

  .fresh-badge {
    background: color-mix(in srgb, #2f855a 12%, var(--panel));
    color: #2f855a;
  }
  .stale-badge {
    background: color-mix(in srgb, #b7791f 12%, var(--panel));
    color: #9c6417;
  }
  .item-rise {
    color: var(--success);
  }
  .item-fall {
    color: var(--danger);
  }
  .item-covered {
    background: color-mix(in srgb, #2f855a 12%, var(--panel));
    color: #2f855a;
  }
  .item-strengthenable {
    background: color-mix(in srgb, #3182ce 12%, var(--panel));
    color: #2467a6;
  }
  .item-gap {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .keyword-option:hover,
  .keyword-option.selected-keyword {
    border-color: color-mix(in srgb, var(--brand) 45%, var(--line)) !important;
    background: var(--brand-faint);
  }

  @media (max-width: 1280px) {
    .report-page :global(.grid-cols-4) {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .report-page :global(.grid-cols-3) {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .report-page :global(.grid-cols-5) {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 760px) {
    .report-page :global(.grid-cols-2),
    .report-page :global(.grid-cols-3),
    .report-page :global(.grid-cols-4),
    .report-page :global(.grid-cols-5) {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
