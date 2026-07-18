<script lang="ts">
  import { onMount } from 'svelte';
  import { replaceState } from '$app/navigation';
  import {
    ArrowUpRight,
    BriefcaseBusiness,
    Check,
    CheckCircle2,
    ChevronDown,
    Clipboard,
    Download,
    Filter,
    Info,
    MapPin,
    MessageCircle,
    Search,
    Sparkles,
    Trash2,
    X,
    XCircle
  } from 'lucide-svelte';
  import DeleteJobDialog from '$lib/components/DeleteJobDialog.svelte';
  import FitScore from '$lib/components/FitScore.svelte';
  import JobSkillFilter from '$lib/components/JobSkillFilter.svelte';
  import JobSearchDialog from '$lib/components/JobSearchDialog.svelte';
  import { chooseLocalExportPath, localExportStamp } from '$lib/export-file';
  import { COMPANY_SCALE_FILTER_OPTIONS, type CompanyScaleFilterCode } from '$lib/job-filters';
  import { backend } from '$lib/services/backend';
  import { latestCompletedScrapeRun } from '$lib/scrape-history';
  import { createSearchSpec } from '$lib/search-spec';
  import { refresh, snapshot, startScrape } from '$lib/stores/app';
  import type {
    Job,
    JobFilterOptions,
    JobQuery,
    JobSort,
    ReportSalaryBand,
    SearchSpec
  } from '$lib/types';

  let selectedId = '';
  let query = '';
  let minScore = 0;
  let onlyNew = false;
  let companyScaleFilter: CompanyScaleFilterCode = '';
  let cityFilter = '';
  let missingDescription = false;
  let keywordKeys: string[] = [];
  let skillFilters: string[] = [];
  let experienceFilter = '';
  let salaryBandFilter: ReportSalaryBand = '';
  let fromReport = false;
  let requestedJobId: string | null = null;
  let jobFilterOptions: JobFilterOptions = { cities: [], experiences: [], skills: [] };
  let sort: JobSort = 'recommended';
  let activeTab: 'description' | 'fit' = 'description';
  let scraping = false;
  let extractionStarting = false;
  let batchStarting = false;
  let batchMenuOpen = false;
  let analyzingJobId = '';
  let exportingJobs = false;
  let deletingJobId = '';
  let bulkDeleting = false;
  let deleteConfirmation:
    { mode: 'single'; job: Job } | { mode: 'bulk'; count: number; query: JobQuery } | null = null;
  let searchDialogOpen = false;
  let searchSpec: SearchSpec = createSearchSpec();
  let greetingBusy = false;
  let toast = '';
  let jobs: Job[] = [];
  let totalJobs = 0;
  let pendingDetailCount = 0;
  let nextCursor: string | null = null;
  let jobsLoading = false;
  let jobsError = '';
  let jobsRequestId = 0;
  let mounted = false;
  let filterTimer: number | undefined;
  let lastFilterKey = '';
  let lastTerminalTaskKey = '';

  const makeFilterKey = (
    nextQuery: string,
    nextMinScore: number,
    nextOnlyNew: boolean,
    nextCompanyScale: CompanyScaleFilterCode,
    nextCity: string,
    nextMissingDescription: boolean,
    nextKeywordKeys: string[],
    nextSkills: string[],
    nextExperience: string,
    nextSalaryBand: ReportSalaryBand,
    nextSort: JobSort
  ) =>
    JSON.stringify([
      nextQuery.trim(),
      nextMinScore,
      nextOnlyNew,
      nextCompanyScale,
      nextCity,
      nextMissingDescription,
      nextKeywordKeys,
      nextSkills,
      nextExperience,
      nextSalaryBand,
      nextSort
    ]);
  $: filterKey = makeFilterKey(
    query,
    minScore,
    onlyNew,
    companyScaleFilter,
    cityFilter,
    missingDescription,
    keywordKeys,
    skillFilters,
    experienceFilter,
    salaryBandFilter,
    sort
  );
  $: if (mounted && filterKey !== lastFilterKey) {
    lastFilterKey = filterKey;
    requestedJobId = null;
    syncJobsUrl();
    window.clearTimeout(filterTimer);
    filterTimer = window.setTimeout(() => void reloadJobs(), 220);
  }
  $: terminalTaskKey = $snapshot.tasks
    .filter(
      (task) =>
        ['scrape', 'job-detail-extraction', 'fit'].includes(task.kind) &&
        ['completed', 'failed', 'cancelled'].includes(task.state)
    )
    .map((task) => `${task.id}:${task.updatedAt}`)
    .join('|');
  $: if (mounted && terminalTaskKey && terminalTaskKey !== lastTerminalTaskKey) {
    lastTerminalTaskKey = terminalTaskKey;
    void reloadJobs();
    void reloadFilterOptions();
  }
  $: {
    const selectedIsVisible = jobs.some((job) => job.id === selectedId);
    if (!selectedIsVisible) {
      const nextId =
        (requestedJobId && jobs.some((job) => job.id === requestedJobId)
          ? requestedJobId
          : jobs[0]?.id) ?? '';
      if (selectedId !== nextId) {
        selectedId = nextId;
        activeTab = 'description';
      }
    }
  }
  $: selected = jobs.find((job) => job.id === selectedId) as Job | undefined;
  $: detailExtractionRunning =
    extractionStarting ||
    $snapshot.tasks.some(
      (task) =>
        task.kind === 'job-detail-extraction' &&
        (task.state === 'queued' || task.state === 'running')
    );
  $: fitBatchRunning =
    batchStarting ||
    $snapshot.tasks.some(
      (task) => task.kind === 'fit' && (task.state === 'queued' || task.state === 'running')
    );
  $: scrapeTaskRunning =
    scraping ||
    $snapshot.tasks.some(
      (task) => task.kind === 'scrape' && (task.state === 'queued' || task.state === 'running')
    );
  $: latestCompletedScrape = latestCompletedScrapeRun($snapshot.scrapeRuns);
  $: hasActiveFilters = Boolean(
    query.trim() ||
    minScore ||
    onlyNew ||
    companyScaleFilter ||
    cityFilter ||
    missingDescription ||
    keywordKeys.length ||
    skillFilters.length ||
    experienceFilter ||
    salaryBandFilter
  );

  function currentJobQuery(cursor: string | null = null): JobQuery {
    return {
      query,
      minScore,
      onlyNew,
      salary: '',
      companyScale: companyScaleFilter,
      city: cityFilter,
      missingDescription,
      keywordKeys,
      skills: skillFilters,
      experience: experienceFilter,
      salaryBand: salaryBandFilter,
      sort,
      cursor
    };
  }

  async function reloadFilterOptions() {
    try {
      jobFilterOptions = await backend.listJobFilterOptions();
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    }
  }

  async function reloadJobs() {
    const requestId = ++jobsRequestId;
    jobsLoading = true;
    jobsError = '';
    try {
      const result = await backend.listJobsPage(currentJobQuery());
      if (requestId !== jobsRequestId) return;
      jobs = result.items;
      totalJobs = result.total;
      pendingDetailCount = result.pendingDetailCount;
      nextCursor = result.nextCursor ?? null;
      if (requestedJobId && !jobs.some((job) => job.id === requestedJobId)) {
        try {
          jobs = [await backend.getJob(requestedJobId), ...jobs];
        } catch {
          /* invalid deep link */
        }
      }
    } catch (error) {
      if (requestId === jobsRequestId)
        jobsError = error instanceof Error ? error.message : String(error);
    } finally {
      if (requestId === jobsRequestId) jobsLoading = false;
    }
  }

  async function loadNextPage() {
    if (!nextCursor || jobsLoading) return;
    const cursor = nextCursor;
    const requestId = ++jobsRequestId;
    jobsLoading = true;
    jobsError = '';
    try {
      const result = await backend.listJobsPage(currentJobQuery(cursor));
      if (requestId !== jobsRequestId) return;
      const known = new Set(jobs.map((job) => job.id));
      jobs = [...jobs, ...result.items.filter((job) => !known.has(job.id))];
      totalJobs = result.total;
      pendingDetailCount = result.pendingDetailCount;
      nextCursor = result.nextCursor ?? null;
    } catch (error) {
      if (requestId === jobsRequestId)
        jobsError = error instanceof Error ? error.message : String(error);
    } finally {
      if (requestId === jobsRequestId) jobsLoading = false;
    }
  }

  function infiniteScroll(node: HTMLElement) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) void loadNextPage();
      },
      { rootMargin: '240px' }
    );
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function dismissBatchMenu(node: HTMLElement) {
    const onPointerDown = (event: PointerEvent) => {
      const details = node as HTMLDetailsElement;
      if (details.open && event.target && !node.contains(event.target as Node)) {
        details.open = false;
        batchMenuOpen = false;
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && (node as HTMLDetailsElement).open) {
        (node as HTMLDetailsElement).open = false;
        batchMenuOpen = false;
      }
    };
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return {
      destroy() {
        document.removeEventListener('pointerdown', onPointerDown);
        document.removeEventListener('keydown', onKeyDown);
      }
    };
  }

  onMount(() => {
    const params = new URL(window.location.href).searchParams;
    requestedJobId = params.get('job');
    fromReport = params.get('from') === 'report';
    keywordKeys = [
      ...new Set(
        params
          .getAll('keyword')
          .map((value) => value.trim())
          .filter(Boolean)
      )
    ];
    skillFilters = [
      ...new Set(
        params
          .getAll('skill')
          .map((value) => value.trim())
          .filter(Boolean)
      )
    ];
    cityFilter = params.get('city')?.trim() ?? '';
    experienceFilter = params.get('experience')?.trim() ?? '';
    const requestedSalaryBand = params.get('salaryBand') ?? '';
    salaryBandFilter = ['under-15', '15-25', '25-35', '35-50', '50-plus'].includes(
      requestedSalaryBand
    )
      ? (requestedSalaryBand as ReportSalaryBand)
      : '';
    mounted = true;
    lastFilterKey = makeFilterKey(
      query,
      minScore,
      onlyNew,
      companyScaleFilter,
      cityFilter,
      missingDescription,
      keywordKeys,
      skillFilters,
      experienceFilter,
      salaryBandFilter,
      sort
    );
    lastTerminalTaskKey = terminalTaskKey;
    void reloadJobs();
    void reloadFilterOptions();
    return () => window.clearTimeout(filterTimer);
  });

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => {
      if (toast === message) toast = '';
    }, 2600);
  }

  async function runScrape() {
    if (scrapeTaskRunning) {
      showToast('已有岗位抓取任务正在运行');
      return;
    }
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

  function openSearchDialog() {
    searchSpec = createSearchSpec($snapshot.lastSearchSpec);
    searchDialogOpen = true;
  }

  async function greeting() {
    if (!selected) return;
    greetingBusy = true;
    try {
      await backend.generateGreeting(selected.id);
      await refresh();
      await reloadJobs();
      showToast('招呼语已生成');
    } finally {
      greetingBusy = false;
    }
  }

  async function extractJobDetails() {
    extractionStarting = true;
    try {
      await backend.startJobDetailExtraction();
      await refresh();
      await reloadJobs();
      showToast('结构化详情批量提取已启动');
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      extractionStarting = false;
    }
  }

  async function copyGreeting() {
    if (!selected?.greeting) return;
    await navigator.clipboard.writeText(selected.greeting);
    showToast('已复制招呼语');
  }

  function clearFilters() {
    query = '';
    minScore = 0;
    onlyNew = false;
    companyScaleFilter = '';
    cityFilter = '';
    missingDescription = false;
    keywordKeys = [];
    skillFilters = [];
    experienceFilter = '';
    salaryBandFilter = '';
    fromReport = false;
  }

  function syncJobsUrl() {
    if (typeof window === 'undefined') return;
    const url = new URL(window.location.href);
    for (const key of [
      'job',
      'from',
      'window',
      'keyword',
      'skill',
      'city',
      'experience',
      'salaryBand'
    ])
      url.searchParams.delete(key);
    if (fromReport) {
      url.searchParams.set('from', 'report');
    }
    keywordKeys.forEach((key) => url.searchParams.append('keyword', key));
    skillFilters.forEach((skill) => url.searchParams.append('skill', skill));
    if (cityFilter) url.searchParams.set('city', cityFilter);
    if (experienceFilter) url.searchParams.set('experience', experienceFilter);
    if (salaryBandFilter) url.searchParams.set('salaryBand', salaryBandFilter);
    const nextUrl = `${url.pathname}${url.search}${url.hash}`;
    replaceState(nextUrl, {});
  }

  function reportReturnHref() {
    const params = new URLSearchParams();
    keywordKeys.forEach((key) => params.append('keyword', key));
    return `/reports?${params.toString()}`;
  }

  async function openSource() {
    if (!selected) return;
    try {
      await backend.openJobSource(selected.id);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      try {
        await navigator.clipboard.writeText(selected.sourceUrl);
        showToast(`${message}；岗位链接已复制`);
      } catch {
        showToast(`${message}；复制失败，请稍后重试`);
      }
    }
  }

  async function analyzeSelected(force = false) {
    if (!selected || analyzingJobId) return;
    const jobId = selected.id;
    analyzingJobId = jobId;
    try {
      const result = await backend.analyzeJob(jobId, force);
      await refresh();
      await reloadJobs();
      const source = result.source === 'llm' ? 'AI 分析' : '本地基础匹配';
      showToast(result.warning || `${source}已完成${result.cacheHit ? '（使用缓存）' : ''}`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      analyzingJobId = '';
    }
  }

  async function analyzeFilteredJobs() {
    batchMenuOpen = false;
    if (fitBatchRunning || totalJobs === 0) return;
    batchStarting = true;
    try {
      await backend.startFitBatchForQuery(currentJobQuery());
      await refresh();
      showToast(`已启动 ${totalJobs} 个岗位的批量匹配分析`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      batchStarting = false;
    }
  }

  async function exportCurrentJobs() {
    batchMenuOpen = false;
    if (exportingJobs) return;
    exportingJobs = true;
    try {
      const fileName = `岗位筛选结果_${localExportStamp()}.json`;
      const outputPath = await chooseLocalExportPath({
        title: '导出当前筛选结果 JSON',
        fileName,
        filterName: '岗位 JSON',
        extension: 'json'
      });
      if (!outputPath) return;
      const result = await backend.exportJobsJson(outputPath, currentJobQuery());
      showToast(`已导出：${result.path}`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      exportingJobs = false;
    }
  }

  function requestDeleteSelectedJob() {
    if (selected && !deletingJobId) deleteConfirmation = { mode: 'single', job: selected };
  }

  function requestDeleteFilteredMissingJobs() {
    if (!missingDescription || bulkDeleting || totalJobs === 0) return;
    deleteConfirmation = { mode: 'bulk', count: totalJobs, query: currentJobQuery() };
  }

  function closeDeleteConfirmation() {
    if (!deletingJobId && !bulkDeleting) deleteConfirmation = null;
  }

  async function confirmDeleteJobs() {
    const confirmation = deleteConfirmation;
    if (!confirmation) return;
    if (confirmation.mode === 'single') deletingJobId = confirmation.job.id;
    else bulkDeleting = true;
    try {
      const result =
        confirmation.mode === 'single'
          ? await backend.deleteJob(confirmation.job.id)
          : await backend.deleteMissingDescriptionJobs(confirmation.query);
      selectedId = '';
      await reloadFilterOptions();
      await reloadJobs();
      deleteConfirmation = null;
      showToast(
        confirmation.mode === 'single'
          ? '岗位已删除'
          : `已删除 ${result.deletedCount} 个无原始详情岗位`
      );
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      deletingJobId = '';
      bulkDeleting = false;
    }
  }

  const verdictLabel = (verdict?: string) =>
    ({
      strong: '高度匹配',
      good: '值得申请',
      moderate: '谨慎评估',
      weak: '匹配偏弱',
      poor: '不建议'
    })[verdict ?? ''] ?? '待分析';
  const constraintTone = (status: string) =>
    status === 'pass' ? 'var(--success)' : status === 'fail' ? 'var(--danger)' : 'var(--warning)';
  const analysisSourceLabel = (source?: string) =>
    ({ llm: 'AI 分析', local: '本地基础匹配', legacy: '历史分析' })[source ?? ''] ?? '历史分析';
  const fallbackReasonLabel = (reason?: string | null) =>
    ({
      provider_missing: '未配置可用模型，本次使用本地基础匹配。',
      llm_failed: '模型调用失败，本次已回退到本地基础匹配。',
      invalid_output: '模型结果格式无效，本次已回退到本地基础匹配。'
    })[reason ?? ''] ?? '';
  const JOB_SALARY_BAND_OPTIONS: Array<{ value: ReportSalaryBand; label: string }> = [
    { value: '', label: '不限' },
    { value: 'under-15', label: '15K 以下' },
    { value: '15-25', label: '15–25K' },
    { value: '25-35', label: '25–35K' },
    { value: '35-50', label: '35–50K' },
    { value: '50-plus', label: '50K 以上' }
  ];

  function latestScrapeLabel() {
    if (!latestCompletedScrape) return '完成一次抓取后可用';
    const completedAt = latestCompletedScrape.completedAt ?? latestCompletedScrape.startedAt;
    const time = new Intl.DateTimeFormat('zh-CN', {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    }).format(new Date(completedAt));
    return `${latestCompletedScrape.keyword} · ${time} · 新增 ${latestCompletedScrape.inserted}`;
  }
</script>

<div class="flex h-[calc(100vh-74px)] min-h-[646px] overflow-hidden">
  <aside
    class="filter-sidebar scrollbar-thin w-[250px] shrink-0 overflow-y-auto border-r p-4"
    style="border-color: var(--line); background: color-mix(in srgb, var(--canvas) 72%, var(--panel));"
  >
    <button
      class="btn-primary mb-4 w-full"
      type="button"
      on:click|stopPropagation={openSearchDialog}
      disabled={scrapeTaskRunning}
      ><Search size={16} />{scrapeTaskRunning ? '岗位抓取中…' : '抓取新岗位'}</button
    >
    <label class="relative block">
      <Search size={15} class="pointer-events-none absolute left-3 top-3 body-muted" />
      <input class="input pl-9" bind:value={query} placeholder="搜索岗位或公司" />
    </label>
    <div class="mt-5 flex items-center justify-between">
      <span class="eyebrow flex items-center gap-1.5"><Filter size={13} />筛选条件</span
      >{#if hasActiveFilters}<button class="text-xs text-brand" on:click={clearFilters}
          >清除筛选</button
        >{/if}
    </div>
    <label class="mt-4 block"
      ><span class="label flex justify-between"
        ><span>最低匹配度</span><span class="text-brand">{minScore}%</span></span
      ><input
        class="w-full accent-[var(--brand)]"
        type="range"
        min="0"
        max="90"
        step="10"
        bind:value={minScore}
      /></label
    >
    <label class="mt-4 block"
      ><span class="label">城市</span><select class="select" bind:value={cityFilter}
        ><option value="">不限</option
        >{#if cityFilter && !jobFilterOptions.cities.includes(cityFilter)}<option value={cityFilter}
            >{cityFilter}</option
          >{/if}{#each jobFilterOptions.cities as city}<option value={city}>{city}</option
          >{/each}</select
      ></label
    >
    <label class="mt-4 block"
      ><span class="label">薪资范围</span><select class="select" bind:value={salaryBandFilter}
        >{#each JOB_SALARY_BAND_OPTIONS as option}<option value={option.value}
            >{option.label}</option
          >{/each}</select
      ></label
    >
    <label class="mt-4 block"
      ><span class="label">经验要求</span><select class="select" bind:value={experienceFilter}
        ><option value="">不限</option
        >{#if experienceFilter && !jobFilterOptions.experiences.includes(experienceFilter)}<option
            value={experienceFilter}>{experienceFilter}</option
          >{/if}{#each jobFilterOptions.experiences as experience}<option value={experience}
            >{experience}</option
          >{/each}</select
      ></label
    >
    <label class="mt-4 block"
      ><span class="label">公司规模</span><select class="select" bind:value={companyScaleFilter}
        >{#each COMPANY_SCALE_FILTER_OPTIONS as option}<option value={option.value}
            >{option.label}</option
          >{/each}</select
      ></label
    >
    <div class="mt-4">
      <span class="label">技能（同时包含）</span><JobSkillFilter
        options={jobFilterOptions.skills}
        bind:selected={skillFilters}
      />
    </div>
    <label
      class="mt-4 flex items-center gap-2 text-sm"
      class:cursor-pointer={Boolean(latestCompletedScrape)}
      class:opacity-60={!latestCompletedScrape}
      ><input
        class="h-4 w-4 accent-[var(--brand)]"
        type="checkbox"
        bind:checked={onlyNew}
        disabled={!latestCompletedScrape}
      />最近一次抓取新增</label
    >
    <p class="ml-6 mt-1 truncate text-[11px] body-muted" title={latestScrapeLabel()}>
      {latestScrapeLabel()}
    </p>
    <label class="mt-3 flex cursor-pointer items-center gap-2 text-sm"
      ><input
        class="h-4 w-4 accent-[var(--brand)]"
        type="checkbox"
        bind:checked={missingDescription}
      />只看无原始详情</label
    >
    {#if missingDescription && totalJobs > 0}
      <button
        class="btn-danger mt-4 w-full"
        on:click={requestDeleteFilteredMissingJobs}
        disabled={bulkDeleting}
        ><Trash2 size={15} />{bulkDeleting ? '正在删除…' : `删除无详情岗位（${totalJobs}）`}</button
      >
    {/if}
    <div class="my-5 divider"></div>
    <div class="space-y-2 text-xs body-muted">
      <div class="flex justify-between">
        <span>岗位数量</span><strong class="text-ink">{totalJobs}</strong>
      </div>
      <div class="flex justify-between">
        <span>数据来源</span><strong class="text-ink">BOSS 直聘</strong>
      </div>
      <div class="flex items-center justify-between gap-2">
        <span>全部岗位详情</span>{#if pendingDetailCount > 0}<button
            class="font-semibold text-brand disabled:opacity-60"
            on:click={extractJobDetails}
            disabled={detailExtractionRunning}
            >{detailExtractionRunning ? '提取中…' : `${pendingDetailCount} 条待提取`}</button
          >{:else}<strong class="text-success">已完成</strong>{/if}
      </div>
    </div>
  </aside>

  <section
    class="job-list scrollbar-thin w-[382px] shrink-0 overflow-y-auto border-r bg-panel"
    style="border-color: var(--line);"
  >
    <div
      class="sticky top-0 z-10 border-b"
      style="border-color: var(--line); background: color-mix(in srgb, var(--panel) 94%, transparent); backdrop-filter: blur(10px);"
    >
      <div class="flex h-12 items-center justify-between gap-2 px-4">
        <div class="flex min-w-0 items-baseline gap-2">
          <p class="shrink-0 text-sm font-semibold">岗位列表</p>
          <span class="truncate text-[11px] body-muted" aria-label={`当前结果 ${totalJobs}`}
            >{totalJobs}</span
          >
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <label
            ><span class="sr-only">岗位排序</span><select
              class="select h-8 w-auto min-w-[104px] py-0 text-xs"
              aria-label="岗位排序"
              bind:value={sort}
              ><option value="recommended">综合推荐</option><option value="recent">最近发现</option
              ><option value="salary-desc">薪资优先</option></select
            ></label
          >
          <details class="group relative" bind:open={batchMenuOpen} use:dismissBatchMenu>
            <!-- svelte-ignore a11y_no_redundant_roles -->
            <summary
              class="btn h-8 cursor-pointer list-none px-2.5 text-[11px] [&::-webkit-details-marker]:hidden"
              role="button"
              aria-label="批量操作"
              ><span>批量</span><ChevronDown
                size={13}
                class="transition group-open:rotate-180"
              /></summary
            >
            <div
              class="absolute right-0 top-9 z-30 w-60 rounded-xl border bg-panel p-2 shadow-xl"
              style="border-color: var(--line);"
            >
              <p
                class="px-2 pb-1.5 pt-1 text-[10px] font-semibold uppercase tracking-[0.08em] body-muted"
              >
                仅作用于当前筛选结果
              </p>
              <button
                class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-xs hover:bg-[var(--brand-faint)] disabled:opacity-50"
                on:click={analyzeFilteredJobs}
                disabled={fitBatchRunning || totalJobs === 0}
                ><CheckCircle2 size={14} class="text-brand" />{fitBatchRunning
                  ? '正在批量分析…'
                  : `分析当前结果（${totalJobs}）`}</button
              >
              <button
                class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-xs hover:bg-[var(--brand-faint)] disabled:opacity-50"
                on:click={exportCurrentJobs}
                disabled={exportingJobs || totalJobs === 0}
                ><Download size={14} class="text-brand" />{exportingJobs
                  ? '正在导出…'
                  : `导出当前结果（${totalJobs}）`}</button
              >
            </div>
          </details>
        </div>
      </div>
      {#if fromReport}
        <div
          class="flex items-center justify-between gap-2 border-t px-4 py-2 text-[11px]"
          style="border-color: var(--line); background: var(--brand-faint);"
        >
          <span class="min-w-0 truncate"
            ><strong class="text-brand">来自数据报告</strong>{#if keywordKeys.length}<span
                class="body-muted"
              >
                · {keywordKeys.length} 组关键词</span
              >{/if}</span
          >
          <span class="flex shrink-0 items-center gap-2"
            >{#if keywordKeys.length}<button
                class="text-brand"
                aria-label="移除关键词范围"
                on:click={() => (keywordKeys = [])}>移除范围</button
              >{/if}<a href={reportReturnHref()} class="font-semibold text-brand">返回报告</a></span
          >
        </div>
      {/if}
    </div>
    {#if jobsLoading && jobs.length === 0}
      <div class="grid min-h-[400px] place-items-center px-8 text-center">
        <p class="text-sm body-muted">正在加载岗位…</p>
      </div>
    {:else if jobsError && jobs.length === 0}
      <div class="grid min-h-[400px] place-items-center px-8 text-center">
        <div>
          <p class="text-sm text-danger">{jobsError}</p>
          <button class="btn mt-4" on:click={() => void reloadJobs()}>重试</button>
        </div>
      </div>
    {:else if jobs.length === 0}
      <div class="grid min-h-[400px] place-items-center px-8 text-center">
        <div>
          <BriefcaseBusiness size={26} class="mx-auto mb-3 body-muted" />
          <p class="text-sm font-semibold">没有符合条件的岗位</p>
          <p class="mt-1 text-xs body-muted">降低匹配度或清除筛选条件。</p>
          {#if hasActiveFilters}<button class="btn mt-4" on:click={clearFilters}>清除筛选</button
            >{/if}
        </div>
      </div>
    {:else}
      {#each jobs as job}
        <button
          class:selected={selected?.id === job.id}
          class="job-row w-full border-b px-4 py-4 text-left transition"
          style="border-color: var(--line);"
          on:click={() => {
            selectedId = job.id;
            activeTab = 'description';
          }}
        >
          <div class="flex gap-3">
            <FitScore score={job.fit?.overallScore ?? 0} size="sm" />
            <div class="min-w-0 flex-1">
              <div class="flex items-start justify-between gap-2">
                <h3 class="line-clamp-1 text-sm font-semibold">{job.title}</h3>
                <div class="flex shrink-0 gap-1">
                  {#if job.fit?.cacheStatus === 'stale'}<span class="chip px-2 py-0.5 text-warning"
                      >待更新</span
                    >{/if}{#if job.isNew}<span class="chip-brand px-2 py-0.5">新</span>{/if}
                </div>
              </div>
              <p class="mt-1 truncate text-xs body-muted">
                {job.company} · {job.location.split('·').slice(0, 2).join('·')}
              </p>
              <div class="mt-3 flex items-center justify-between">
                <span class="text-sm font-semibold text-brand">{job.salary}</span><span
                  class="text-[11px] body-muted">{job.experience} · {job.degree}</span
                >
              </div>
              <div class="mt-2 flex flex-wrap gap-1">
                {#each job.skills.slice(0, 3) as skill}<span class="chip px-2 py-0.5">{skill}</span
                  >{/each}
              </div>
            </div>
          </div>
        </button>
      {/each}
      {#if nextCursor}<div
          class="grid h-16 place-items-center text-xs body-muted"
          use:infiniteScroll
        >
          {jobsLoading ? '正在加载更多…' : '继续滚动加载更多'}
        </div>{:else}<div class="py-5 text-center text-xs body-muted">
          已加载全部 {totalJobs} 个岗位
        </div>{/if}
      {#if jobsError}<div class="p-4 text-center text-xs text-danger">
          {jobsError}<button class="btn ml-2" on:click={() => void loadNextPage()}>重试</button>
        </div>{/if}
    {/if}
  </section>

  <section class="scrollbar-thin min-w-0 flex-1 overflow-y-auto">
    {#if selected}
      <div
        class="sticky top-0 z-10 border-b bg-canvas px-6 pt-5"
        style="border-color: var(--line);"
      >
        <div class="pb-4">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <h2 class="truncate text-[22px] font-semibold tracking-[-0.035em]">
                {selected.title}
              </h2>
              {#if selected.isNew}<span class="chip-brand">最近新增</span>{/if}
            </div>
            <p class="mt-1 text-sm body-muted">
              {selected.company} · {selected.companyScale} · {selected.industry}
            </p>
            <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
              <strong class="text-brand">{selected.salary}</strong><span
                class="flex items-center gap-1 body-muted"
                ><MapPin size={14} />{selected.location}</span
              ><span class="body-muted">{selected.experience} · {selected.degree}</span>
            </div>
          </div>
          <div class="mt-4 flex flex-wrap items-center justify-end gap-2">
            {#if selected.greeting}
              <button class="btn" on:click={copyGreeting}><Clipboard size={14} />复制招呼语</button>
            {:else}
              <button class="btn" disabled={greetingBusy} on:click={greeting}
                ><MessageCircle size={15} />{greetingBusy ? '正在生成…' : '生成一句招呼语'}</button
              >
            {/if}
            <button
              class="btn-danger"
              on:click={requestDeleteSelectedJob}
              disabled={deletingJobId === selected.id}
              ><Trash2 size={14} />{deletingJobId === selected.id
                ? '正在删除…'
                : '删除岗位'}</button
            >
            <button class="btn" on:click={openSource}>查看原岗位 <ArrowUpRight size={15} /></button>
            <a
              class="btn-primary"
              href={`/resume?job=${encodeURIComponent(selected.id)}&assistant=1`}
              ><Sparkles size={15} />用此岗位优化主简历</a
            >
          </div>
        </div>
        <nav class="flex gap-6" aria-label="岗位详情切换">
          <button
            class:active={activeTab === 'description'}
            class="tab"
            on:click={() => (activeTab = 'description')}>岗位详情</button
          >
          <button
            class:active={activeTab === 'fit'}
            class="tab"
            on:click={() => (activeTab = 'fit')}
            >匹配分析 {#if selected.fit}<span class="ml-1 text-brand"
                >{selected.fit.overallScore}%</span
              >{/if}</button
          >
        </nav>
      </div>

      <div class="mx-auto max-w-[920px] p-6">
        {#if activeTab === 'description'}
          <div class="space-y-4 animate-lift">
            <article class="panel p-6">
              <div class="mb-5 flex items-center justify-between">
                <h3 class="section-title">职位描述</h3>
                <span class={selected.structuredDetails ? 'chip-brand' : 'chip'}
                  >{selected.structuredDetails ? 'AI 已提取' : '待提取'}</span
                >
              </div>
              {#if selected.structuredDetails}
                <div class="space-y-7">
                  <section>
                    <p class="label">职位描述</p>
                    <p class="whitespace-pre-line text-sm leading-7">
                      {selected.structuredDetails.jobDescription ||
                        '原始详情中未单独提供职位概述。'}
                    </p>
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">岗位职责</p>
                    {#if selected.structuredDetails.responsibilities.length}
                      <ol class="space-y-2 text-sm leading-6">
                        {#each selected.structuredDetails.responsibilities as item, index}<li
                            class="flex gap-3"
                          >
                            <span class="text-brand">{index + 1}.</span><span>{item}</span>
                          </li>{/each}
                      </ol>
                    {:else}<p class="text-sm body-muted">原始详情中未提供。</p>{/if}
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">任职要求</p>
                    {#if selected.structuredDetails.requirements.length}
                      <ol class="space-y-2 text-sm leading-6">
                        {#each selected.structuredDetails.requirements as item, index}<li
                            class="flex gap-3"
                          >
                            <span class="text-brand">{index + 1}.</span><span>{item}</span>
                          </li>{/each}
                      </ol>
                    {:else}<p class="text-sm body-muted">原始详情中未提供。</p>{/if}
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">公司介绍</p>
                    <p class="whitespace-pre-line text-sm leading-7">
                      {selected.structuredDetails.companyIntroduction || '原始详情中未提供。'}
                    </p>
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">工商信息</p>
                    <dl class="grid grid-cols-2 gap-x-8 gap-y-4 text-sm">
                      <div>
                        <dt class="text-xs body-muted">公司名称</dt>
                        <dd class="mt-1">
                          {selected.structuredDetails.businessInformation.companyName || '—'}
                        </dd>
                      </div>
                      <div>
                        <dt class="text-xs body-muted">法定代表人</dt>
                        <dd class="mt-1">
                          {selected.structuredDetails.businessInformation.legalRepresentative ||
                            '—'}
                        </dd>
                      </div>
                      <div>
                        <dt class="text-xs body-muted">成立日期</dt>
                        <dd class="mt-1">
                          {selected.structuredDetails.businessInformation.establishedDate || '—'}
                        </dd>
                      </div>
                      <div>
                        <dt class="text-xs body-muted">企业类型</dt>
                        <dd class="mt-1">
                          {selected.structuredDetails.businessInformation.companyType || '—'}
                        </dd>
                      </div>
                      <div>
                        <dt class="text-xs body-muted">经营状态</dt>
                        <dd class="mt-1">
                          {selected.structuredDetails.businessInformation.operatingStatus || '—'}
                        </dd>
                      </div>
                      <div>
                        <dt class="text-xs body-muted">注册资金</dt>
                        <dd class="mt-1">
                          {selected.structuredDetails.businessInformation.registeredCapital || '—'}
                        </dd>
                      </div>
                    </dl>
                  </section>
                  <details
                    class="border-t pt-5 text-xs body-muted"
                    style="border-color: var(--line);"
                  >
                    <summary class="cursor-pointer font-medium text-ink">查看抓取原文</summary>
                    <div class="mt-4 whitespace-pre-line leading-6">{selected.description}</div>
                  </details>
                </div>
              {:else}
                <div class="whitespace-pre-line text-sm leading-7">
                  {selected.description || '暂无原始职位详情。'}
                </div>
              {/if}
              {#if selected.skills.length}<div class="mt-6">
                  <p class="label">岗位技能</p>
                  <div class="flex flex-wrap gap-2">
                    {#each selected.skills as skill}<span class="chip-brand">{skill}</span>{/each}
                  </div>
                </div>{/if}
              {#if selected.welfare.length}<div class="mt-6">
                  <p class="label">福利待遇</p>
                  <div class="flex flex-wrap gap-2">
                    {#each selected.welfare as item}<span class="chip">{item}</span>{/each}
                  </div>
                </div>{/if}
            </article>
            {#if selected.greeting}<article class="panel p-5">
                <p class="eyebrow">招呼语</p>
                <p class="mt-3 text-sm leading-6">{selected.greeting}</p>
              </article>{/if}
            <article
              class="rounded-2xl border p-4"
              style="border-color: var(--line); background: var(--warning-soft);"
            >
              <div class="flex gap-2">
                <Info size={16} class="mt-0.5 shrink-0 text-warning" />
                <p class="text-xs leading-5 body-muted">
                  招聘网站内容可能随时变化，请在投递前打开原岗位确认有效性。
                </p>
              </div>
            </article>
          </div>
        {:else if activeTab === 'fit'}
          <div class="space-y-5 animate-lift">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h3 class="section-title">岗位匹配分析</h3>
                <p class="mt-1 text-xs body-muted">结果会根据当前岗位、主简历和模型配置生成。</p>
              </div>
              <button
                class="btn-primary"
                on:click={() => analyzeSelected(Boolean(selected.fit))}
                disabled={Boolean(analyzingJobId)}
                ><Sparkles size={15} />{analyzingJobId === selected.id
                  ? '正在分析…'
                  : selected.fit
                    ? '重新分析'
                    : '分析当前岗位'}</button
              >
            </div>
            {#if selected.fit}
              {#if selected.fit.cacheStatus === 'stale'}
                <article
                  class="rounded-2xl border p-4"
                  style="border-color: var(--line); background: var(--warning-soft);"
                >
                  <div class="flex gap-2">
                    <Info size={16} class="mt-0.5 shrink-0 text-warning" />
                    <p class="text-xs leading-5 body-muted">
                      岗位、简历或模型配置已经变化，以下结果已过期。你可以暂时参考，建议重新分析。
                    </p>
                  </div>
                </article>
              {/if}
              {#if fallbackReasonLabel(selected.fit.fallbackReason)}
                <article
                  class="rounded-2xl border p-4"
                  style="border-color: var(--line); background: var(--brand-faint);"
                >
                  <div class="flex gap-2">
                    <Info size={16} class="mt-0.5 shrink-0 text-brand" />
                    <p class="text-xs leading-5 body-muted">
                      {fallbackReasonLabel(selected.fit.fallbackReason)}
                    </p>
                  </div>
                </article>
              {/if}
              <article class="panel grid grid-cols-[150px_1fr] items-center gap-8 p-7">
                <div class="text-center">
                  <FitScore score={selected.fit.overallScore} size="lg" />
                  <p class="mt-2 text-sm font-semibold">{verdictLabel(selected.fit.verdict)}</p>
                  <p class="mt-1 text-[11px] body-muted">置信度 {selected.fit.confidence}%</p>
                </div>
                <div>
                  <div class="flex flex-wrap items-center gap-2">
                    <span class={selected.fit.analysisSource === 'llm' ? 'chip-brand' : 'chip'}
                      ><Sparkles size={12} />{analysisSourceLabel(
                        selected.fit.analysisSource
                      )}</span
                    >{#if selected.fit.cacheStatus === 'stale'}<span class="chip text-warning"
                        >结果已过期</span
                      >{:else if !selected.fit.cacheStatus || selected.fit.cacheStatus === 'legacy'}<span
                        class="chip">历史缓存</span
                      >{:else}<span class="chip text-success">最新结果</span>{/if}
                  </div>
                  <h3 class="mt-3 text-xl font-semibold tracking-[-0.025em]">
                    {selected.fit.summary}
                  </h3>
                  <p class="mt-2 text-sm leading-6 body-muted">{selected.fit.recommendation}</p>
                  <div class="mt-4 flex flex-wrap gap-2">
                    {#each selected.fit.hardConstraints as constraint}<span
                        class="chip"
                        style={`color:${constraintTone(constraint.status)}`}
                        ><CheckCircle2 size={12} />{constraint.label}：{constraint.note}</span
                      >{/each}
                  </div>
                </div>
              </article>
              <div class="grid grid-cols-2 gap-4">
                {#each selected.fit.dimensions as dimension}
                  <article class="panel-flat p-5">
                    <div class="mb-3 flex items-center justify-between">
                      <h4 class="text-sm font-semibold">{dimension.label}</h4>
                      <strong class="text-lg tracking-[-0.04em]"
                        >{dimension.score === null ? '—' : dimension.score}</strong
                      >
                    </div>
                    <div class="h-1.5 rounded-full surface-soft">
                      <div
                        class="h-full rounded-full"
                        style={`width:${dimension.score ?? 0}%;background:var(--brand)`}
                      ></div>
                    </div>
                    <p class="mt-3 text-xs leading-5 body-muted">{dimension.note}</p>
                    {#if dimension.evidence.length}<div class="mt-3 flex flex-wrap gap-1">
                        {#each dimension.evidence.slice(0, 3) as evidence}<span
                            class="chip px-2 py-0.5">{evidence}</span
                          >{/each}
                      </div>{/if}
                  </article>
                {/each}
              </div>
              <div class="grid grid-cols-2 gap-4">
                <article class="panel p-5">
                  <h4 class="flex items-center gap-2 text-sm font-semibold">
                    <CheckCircle2 size={17} class="text-success" />可以主打
                  </h4>
                  <ul class="mt-4 space-y-3">
                    {#each selected.fit.strengths as strength}<li
                        class="flex gap-2 text-sm leading-5"
                      >
                        <Check size={14} class="mt-0.5 shrink-0 text-success" />{strength}
                      </li>{/each}
                  </ul>
                </article>
                <article class="panel p-5">
                  <h4 class="flex items-center gap-2 text-sm font-semibold">
                    <XCircle size={17} class="text-warning" />需要处理的缺口
                  </h4>
                  <ul class="mt-4 space-y-3">
                    {#each selected.fit.gaps as gap}<li class="flex gap-2 text-sm leading-5">
                        <X size={14} class="mt-0.5 shrink-0 text-warning" />{gap}
                      </li>{/each}
                  </ul>
                </article>
              </div>
            {:else}
              <div class="panel grid min-h-[360px] place-items-center p-8 text-center">
                <div class="max-w-md">
                  <span
                    class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft text-brand"
                    ><Sparkles size={24} /></span
                  >
                  <h3 class="section-title">还没有匹配分析</h3>
                  <p class="mt-2 text-sm leading-6 body-muted">
                    点击分析后会优先使用已验证的模型；未配置模型或模型失败时，仍会返回本地基础匹配结果。
                  </p>
                  <button
                    class="btn-primary mt-5"
                    on:click={() => analyzeSelected(false)}
                    disabled={Boolean(analyzingJobId)}
                    ><Sparkles size={16} />{analyzingJobId === selected.id
                      ? '正在分析…'
                      : '分析当前岗位'}</button
                  >
                </div>
              </div>
            {/if}
            <article
              class="rounded-2xl border p-4"
              style="border-color: var(--line); background: var(--brand-faint);"
            >
              <div class="flex gap-2">
                <Info size={16} class="mt-0.5 shrink-0 text-brand" />
                <p class="text-xs leading-5 body-muted">
                  只有主动点击分析时才会调用模型。发送内容仅包含当前岗位和匹配所需的简历信息。
                </p>
              </div>
            </article>
          </div>
        {/if}
      </div>
    {/if}
  </section>
</div>

<JobSearchDialog
  bind:open={searchDialogOpen}
  bind:searchSpec
  {scraping}
  {scrapeTaskRunning}
  onStart={runScrape}
/>

<DeleteJobDialog
  open={Boolean(deleteConfirmation)}
  mode={deleteConfirmation?.mode ?? 'single'}
  jobTitle={deleteConfirmation?.mode === 'single' ? deleteConfirmation.job.title : ''}
  company={deleteConfirmation?.mode === 'single' ? deleteConfirmation.job.company : ''}
  count={deleteConfirmation?.mode === 'bulk' ? deleteConfirmation.count : 0}
  busy={Boolean(deletingJobId || bulkDeleting)}
  onCancel={closeDeleteConfirmation}
  onConfirm={confirmDeleteJobs}
/>

{#if toast}<div
    class="fixed bottom-6 left-1/2 z-[60] -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift"
  >
    {toast}
  </div>{/if}

<style>
  .job-row:hover {
    background: var(--brand-faint);
  }
  .job-row.selected {
    background: var(--brand-faint);
    box-shadow: inset 3px 0 0 var(--brand);
  }
  .tab {
    position: relative;
    padding: 0 0 13px;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  .tab.active {
    color: var(--ink);
  }
  .tab.active::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--brand);
  }
  @media (max-width: 1280px) {
    .filter-sidebar {
      width: 210px;
    }
    .job-list {
      width: 330px;
    }
  }
</style>
