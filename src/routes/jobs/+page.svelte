<script lang="ts">
  import { ArrowUpRight, BriefcaseBusiness, Check, CheckCircle2, ChevronDown, Clipboard, Filter, Info, MapPin, MessageCircle, RefreshCw, Search, Sparkles, X, XCircle } from 'lucide-svelte';
  import { page } from '$app/stores';
  import FitScore from '$lib/components/FitScore.svelte';
  import {
    COMPANY_SCALE_FILTER_OPTIONS,
    SALARY_FILTER_OPTIONS,
    filterJobs,
    type CompanyScaleFilterCode,
    type SalaryFilterCode
  } from '$lib/job-filters';
  import { backend } from '$lib/services/backend';
  import { latestSuccessfulScrapeKeyword } from '$lib/scrape-history';
  import { refresh, snapshot, startScrape } from '$lib/stores/app';
  import type { Job, SearchSpec } from '$lib/types';

  const SCRAPE_CITIES = [
    '北京', '上海', '广州', '深圳', '杭州', '天津', '西安', '苏州', '武汉', '厦门', '长沙', '成都', '郑州',
    '重庆', '佛山', '合肥', '济南', '青岛', '南京', '东莞', '昆明', '南昌', '石家庄', '宁波', '福州'
  ] as const;
  const SCRAPE_PAGE_OPTIONS = [1, 2, 3, 4, 5] as const;

  let selectedId = '';
  let query = '';
  let minScore = 0;
  let onlyNew = false;
  let salaryFilter: SalaryFilterCode = '';
  let companyScaleFilter: CompanyScaleFilterCode = '';
  let activeTab: 'description' | 'fit' = 'description';
  let scraping = false;
  let extractionStarting = false;
  let batchStarting = false;
  let analyzingJobId = '';
  let searchDialogOpen = false;
  let searchSpec: SearchSpec = { keyword: '', city: '上海', pages: 1, salary: '', companyScale: '' };
  let greetingBusy = false;
  let toast = '';

  $: jobs = filterJobs($snapshot.jobs, {
    query,
    minScore,
    onlyNew,
    salary: salaryFilter,
    companyScale: companyScaleFilter
  }).sort((a, b) => (b.fit?.overallScore ?? 0) - (a.fit?.overallScore ?? 0));
  $: requestedId = $page.url?.searchParams.get('job') ?? null;
  $: {
    const selectedIsVisible = jobs.some((job) => job.id === selectedId);
    if (!selectedIsVisible) {
      const nextId = (requestedId && jobs.some((job) => job.id === requestedId) ? requestedId : jobs[0]?.id) ?? '';
      if (selectedId !== nextId) {
        selectedId = nextId;
        activeTab = 'description';
      }
    }
  }
  $: selected = jobs.find((job) => job.id === selectedId) as Job | undefined;
  $: pendingDetailCount = $snapshot.jobs.filter((job) => job.description.trim() && !job.structuredDetails).length;
  $: detailExtractionRunning = extractionStarting || $snapshot.tasks.some((task) => task.kind === 'job-detail-extraction' && (task.state === 'queued' || task.state === 'running'));
  $: fitBatchRunning = batchStarting || $snapshot.tasks.some((task) => task.kind === 'fit' && (task.state === 'queued' || task.state === 'running'));
  $: scrapeTaskRunning = scraping || $snapshot.tasks.some((task) => task.kind === 'scrape' && (task.state === 'queued' || task.state === 'running'));
  $: scrapeDuration = searchSpec.pages === 3 ? '60 分钟（约 1 小时）' : `${searchSpec.pages * 20} 分钟`;
  $: hasActiveFilters = Boolean(query.trim() || minScore || onlyNew || salaryFilter || companyScaleFilter);
  $: keywordMissing = !searchSpec.keyword.trim();

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => { if (toast === message) toast = ''; }, 2600);
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
    searchSpec = { ...searchSpec, keyword: latestSuccessfulScrapeKeyword($snapshot.scrapeRuns) };
    searchDialogOpen = true;
  }

  async function greeting() {
    if (!selected) return;
    greetingBusy = true;
    try { await backend.generateGreeting(selected.id); await refresh(); showToast('招呼语已生成'); } finally { greetingBusy = false; }
  }

  async function extractJobDetails() {
    extractionStarting = true;
    try {
      await backend.startJobDetailExtraction();
      await refresh();
      showToast('岗位详情批量提取已启动');
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
    salaryFilter = '';
    companyScaleFilter = '';
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
      const source = result.source === 'llm' ? 'AI 分析' : '本地基础匹配';
      showToast(result.warning || `${source}已完成${result.cacheHit ? '（使用缓存）' : ''}`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      analyzingJobId = '';
    }
  }

  async function analyzeFilteredJobs() {
    if (fitBatchRunning || jobs.length === 0) return;
    const jobIds = jobs.map((job) => job.id);
    batchStarting = true;
    try {
      await backend.startFitBatch(jobIds);
      await refresh();
      showToast(`已启动 ${jobIds.length} 个岗位的批量匹配分析`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      batchStarting = false;
    }
  }

  const verdictLabel = (verdict?: string) => ({ strong: '高度匹配', good: '值得申请', moderate: '谨慎评估', weak: '匹配偏弱', poor: '不建议' }[verdict ?? ''] ?? '待分析');
  const constraintTone = (status: string) => status === 'pass' ? 'var(--success)' : status === 'fail' ? 'var(--danger)' : 'var(--warning)';
  const analysisSourceLabel = (source?: string) => ({ llm: 'AI 分析', local: '本地基础匹配', legacy: '历史分析' }[source ?? ''] ?? '历史分析');
  const fallbackReasonLabel = (reason?: string | null) => ({
    provider_missing: '未配置可用模型，本次使用本地基础匹配。',
    llm_failed: '模型调用失败，本次已回退到本地基础匹配。',
    invalid_output: '模型结果格式无效，本次已回退到本地基础匹配。'
  }[reason ?? ''] ?? '');
</script>

<div class="flex h-[calc(100vh-74px)] min-h-[646px] overflow-hidden">
  <aside class="filter-sidebar scrollbar-thin w-[250px] shrink-0 overflow-y-auto border-r p-4" style="border-color: var(--line); background: color-mix(in srgb, var(--canvas) 72%, var(--panel));">
    <button class="btn-primary mb-2 w-full" type="button" on:click|stopPropagation={openSearchDialog} disabled={scrapeTaskRunning}><Search size={16} />{scrapeTaskRunning ? '岗位抓取中…' : '抓取新岗位'}</button>
    <button class="btn mb-2 w-full" on:click={extractJobDetails} disabled={detailExtractionRunning || pendingDetailCount === 0}><Sparkles size={15} />{detailExtractionRunning ? '正在批量提取…' : pendingDetailCount ? `批量提取详情（${pendingDetailCount}）` : '岗位详情已提取'}</button>
    <button class="btn mb-4 w-full" on:click={analyzeFilteredJobs} disabled={fitBatchRunning || jobs.length === 0}><CheckCircle2 size={15} />{fitBatchRunning ? '正在批量分析…' : `批量分析当前结果（${jobs.length}）`}</button>
    <label class="relative block">
      <Search size={15} class="pointer-events-none absolute left-3 top-3 body-muted" />
      <input class="input pl-9" bind:value={query} placeholder="搜索岗位或公司" />
    </label>
    <div class="mt-5 flex items-center justify-between"><span class="eyebrow flex items-center gap-1.5"><Filter size={13} />筛选条件</span>{#if hasActiveFilters}<button class="text-xs text-brand" on:click={clearFilters}>清除筛选</button>{/if}</div>
    <label class="mt-4 block"><span class="label flex justify-between"><span>最低匹配度</span><span class="text-brand">{minScore}%</span></span><input class="w-full accent-[var(--brand)]" type="range" min="0" max="90" step="10" bind:value={minScore} /></label>
    <label class="mt-4 block"><span class="label">薪资范围</span><select class="select" bind:value={salaryFilter}>{#each SALARY_FILTER_OPTIONS as option}<option value={option.value}>{option.label}</option>{/each}</select></label>
    <label class="mt-4 block"><span class="label">公司规模</span><select class="select" bind:value={companyScaleFilter}>{#each COMPANY_SCALE_FILTER_OPTIONS as option}<option value={option.value}>{option.label}</option>{/each}</select></label>
    <label class="mt-4 flex cursor-pointer items-center gap-2 text-sm"><input class="h-4 w-4 accent-[var(--brand)]" type="checkbox" bind:checked={onlyNew} />只看本次新增</label>
    <div class="my-5 divider"></div>
    <div class="space-y-2 text-xs body-muted">
      <div class="flex justify-between"><span>岗位数量</span><strong class="text-ink">{jobs.length}</strong></div>
      <div class="flex justify-between"><span>最高匹配</span><strong class="text-ink">{jobs[0]?.fit?.overallScore ?? 0}%</strong></div>
      <div class="flex justify-between"><span>数据来源</span><strong class="text-ink">BOSS 直聘</strong></div>
    </div>
  </aside>

  <section class="job-list scrollbar-thin w-[382px] shrink-0 overflow-y-auto border-r bg-panel" style="border-color: var(--line);">
    <div class="sticky top-0 z-10 flex h-14 items-center justify-between border-b px-4" style="border-color: var(--line); background: color-mix(in srgb, var(--panel) 92%, transparent); backdrop-filter: blur(10px);">
      <p class="text-sm font-semibold">匹配排序</p><button class="btn-ghost h-8 text-xs"><ChevronDown size={14} /> 综合推荐</button>
    </div>
    {#if jobs.length === 0}
      <div class="grid min-h-[400px] place-items-center px-8 text-center"><div><BriefcaseBusiness size={26} class="mx-auto mb-3 body-muted" /><p class="text-sm font-semibold">没有符合条件的岗位</p><p class="mt-1 text-xs body-muted">降低匹配度或清除筛选条件。</p>{#if hasActiveFilters}<button class="btn mt-4" on:click={clearFilters}>清除筛选</button>{/if}</div></div>
    {:else}
      {#each jobs as job}
        <button class:selected={selected?.id === job.id} class="job-row w-full border-b px-4 py-4 text-left transition" style="border-color: var(--line);" on:click={() => { selectedId = job.id; activeTab = 'description'; }}>
          <div class="flex gap-3">
            <FitScore score={job.fit?.overallScore ?? 0} size="sm" />
            <div class="min-w-0 flex-1">
              <div class="flex items-start justify-between gap-2"><h3 class="line-clamp-1 text-sm font-semibold">{job.title}</h3><div class="flex shrink-0 gap-1">{#if job.fit?.cacheStatus === 'stale'}<span class="chip px-2 py-0.5 text-warning">待更新</span>{/if}{#if job.isNew}<span class="chip-brand px-2 py-0.5">新</span>{/if}</div></div>
              <p class="mt-1 truncate text-xs body-muted">{job.company} · {job.location.split('·').slice(0,2).join('·')}</p>
              <div class="mt-3 flex items-center justify-between"><span class="text-sm font-semibold text-brand">{job.salary}</span><span class="text-[11px] body-muted">{job.experience} · {job.degree}</span></div>
              <div class="mt-2 flex flex-wrap gap-1">{#each job.skills.slice(0,3) as skill}<span class="chip px-2 py-0.5">{skill}</span>{/each}</div>
            </div>
          </div>
        </button>
      {/each}
    {/if}
  </section>

  <section class="scrollbar-thin min-w-0 flex-1 overflow-y-auto">
    {#if selected}
      <div class="sticky top-0 z-10 border-b bg-canvas px-6 pt-5" style="border-color: var(--line);">
        <div class="pb-4">
          <div class="min-w-0">
            <div class="flex items-center gap-2"><h2 class="truncate text-[22px] font-semibold tracking-[-0.035em]">{selected.title}</h2>{#if selected.isNew}<span class="chip-brand">本次新增</span>{/if}</div>
            <p class="mt-1 text-sm body-muted">{selected.company} · {selected.companyScale} · {selected.industry}</p>
            <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm"><strong class="text-brand">{selected.salary}</strong><span class="flex items-center gap-1 body-muted"><MapPin size={14} />{selected.location}</span><span class="body-muted">{selected.experience} · {selected.degree}</span></div>
          </div>
          <div class="mt-4 flex flex-wrap items-center justify-end gap-2">
            {#if selected.greeting}
              <button class="btn" on:click={copyGreeting}><Clipboard size={14} />复制招呼语</button>
            {:else}
              <button class="btn" disabled={greetingBusy} on:click={greeting}><MessageCircle size={15} />{greetingBusy ? '正在生成…' : '生成一句招呼语'}</button>
            {/if}
            <button class="btn" on:click={openSource}>查看原岗位 <ArrowUpRight size={15} /></button>
            <a class="btn-primary" href={`/resume?job=${encodeURIComponent(selected.id)}&assistant=1`}><Sparkles size={15} />用此岗位优化主简历</a>
          </div>
        </div>
        <nav class="flex gap-6" aria-label="岗位详情切换">
          <button class:active={activeTab === 'description'} class="tab" on:click={() => activeTab = 'description'}>岗位详情</button>
          <button class:active={activeTab === 'fit'} class="tab" on:click={() => activeTab = 'fit'}>匹配分析 {#if selected.fit}<span class="ml-1 text-brand">{selected.fit.overallScore}%</span>{/if}</button>
        </nav>
      </div>

      <div class="mx-auto max-w-[920px] p-6">
        {#if activeTab === 'description'}
          <div class="space-y-4 animate-lift">
            <article class="panel p-6">
              <div class="mb-5 flex items-center justify-between"><h3 class="section-title">职位描述</h3><span class={selected.structuredDetails ? 'chip-brand' : 'chip'}>{selected.structuredDetails ? 'AI 已提取' : '待提取'}</span></div>
              {#if selected.structuredDetails}
                <div class="space-y-7">
                  <section>
                    <p class="label">职位描述</p>
                    <p class="whitespace-pre-line text-sm leading-7">{selected.structuredDetails.jobDescription || '原始详情中未单独提供职位概述。'}</p>
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">岗位职责</p>
                    {#if selected.structuredDetails.responsibilities.length}
                      <ol class="space-y-2 text-sm leading-6">{#each selected.structuredDetails.responsibilities as item, index}<li class="flex gap-3"><span class="text-brand">{index + 1}.</span><span>{item}</span></li>{/each}</ol>
                    {:else}<p class="text-sm body-muted">原始详情中未提供。</p>{/if}
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">任职要求</p>
                    {#if selected.structuredDetails.requirements.length}
                      <ol class="space-y-2 text-sm leading-6">{#each selected.structuredDetails.requirements as item, index}<li class="flex gap-3"><span class="text-brand">{index + 1}.</span><span>{item}</span></li>{/each}</ol>
                    {:else}<p class="text-sm body-muted">原始详情中未提供。</p>{/if}
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">公司介绍</p>
                    <p class="whitespace-pre-line text-sm leading-7">{selected.structuredDetails.companyIntroduction || '原始详情中未提供。'}</p>
                  </section>
                  <section class="border-t pt-6" style="border-color: var(--line);">
                    <p class="label">工商信息</p>
                    <dl class="grid grid-cols-2 gap-x-8 gap-y-4 text-sm">
                      <div><dt class="text-xs body-muted">公司名称</dt><dd class="mt-1">{selected.structuredDetails.businessInformation.companyName || '—'}</dd></div>
                      <div><dt class="text-xs body-muted">法定代表人</dt><dd class="mt-1">{selected.structuredDetails.businessInformation.legalRepresentative || '—'}</dd></div>
                      <div><dt class="text-xs body-muted">成立日期</dt><dd class="mt-1">{selected.structuredDetails.businessInformation.establishedDate || '—'}</dd></div>
                      <div><dt class="text-xs body-muted">企业类型</dt><dd class="mt-1">{selected.structuredDetails.businessInformation.companyType || '—'}</dd></div>
                      <div><dt class="text-xs body-muted">经营状态</dt><dd class="mt-1">{selected.structuredDetails.businessInformation.operatingStatus || '—'}</dd></div>
                      <div><dt class="text-xs body-muted">注册资金</dt><dd class="mt-1">{selected.structuredDetails.businessInformation.registeredCapital || '—'}</dd></div>
                    </dl>
                  </section>
                  <details class="border-t pt-5 text-xs body-muted" style="border-color: var(--line);"><summary class="cursor-pointer font-medium text-ink">查看抓取原文</summary><div class="mt-4 whitespace-pre-line leading-6">{selected.description}</div></details>
                </div>
              {:else}
                <div class="whitespace-pre-line text-sm leading-7">{selected.description}</div>
              {/if}
              {#if selected.skills.length}<div class="mt-6"><p class="label">岗位技能</p><div class="flex flex-wrap gap-2">{#each selected.skills as skill}<span class="chip-brand">{skill}</span>{/each}</div></div>{/if}
              {#if selected.welfare.length}<div class="mt-6"><p class="label">福利待遇</p><div class="flex flex-wrap gap-2">{#each selected.welfare as item}<span class="chip">{item}</span>{/each}</div></div>{/if}
            </article>
            {#if selected.greeting}<article class="panel p-5"><p class="eyebrow">招呼语</p><p class="mt-3 text-sm leading-6">{selected.greeting}</p></article>{/if}
            <article class="rounded-2xl border p-4" style="border-color: var(--line); background: var(--warning-soft);"><div class="flex gap-2"><Info size={16} class="mt-0.5 shrink-0 text-warning" /><p class="text-xs leading-5 body-muted">招聘网站内容可能随时变化，请在投递前打开原岗位确认有效性。</p></div></article>
          </div>
        {:else if activeTab === 'fit'}
          <div class="space-y-5 animate-lift">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div><h3 class="section-title">岗位匹配分析</h3><p class="mt-1 text-xs body-muted">结果会根据当前岗位、主简历和模型配置生成。</p></div>
              <button class="btn-primary" on:click={() => analyzeSelected(Boolean(selected.fit))} disabled={Boolean(analyzingJobId)}><Sparkles size={15} />{analyzingJobId === selected.id ? '正在分析…' : selected.fit ? '重新分析' : '分析当前岗位'}</button>
            </div>
            {#if selected.fit}
              {#if selected.fit.cacheStatus === 'stale'}
                <article class="rounded-2xl border p-4" style="border-color: var(--line); background: var(--warning-soft);"><div class="flex gap-2"><Info size={16} class="mt-0.5 shrink-0 text-warning" /><p class="text-xs leading-5 body-muted">岗位、简历或模型配置已经变化，以下结果已过期。你可以暂时参考，建议重新分析。</p></div></article>
              {/if}
              {#if fallbackReasonLabel(selected.fit.fallbackReason)}
                <article class="rounded-2xl border p-4" style="border-color: var(--line); background: var(--brand-faint);"><div class="flex gap-2"><Info size={16} class="mt-0.5 shrink-0 text-brand" /><p class="text-xs leading-5 body-muted">{fallbackReasonLabel(selected.fit.fallbackReason)}</p></div></article>
              {/if}
              <article class="panel grid grid-cols-[150px_1fr] items-center gap-8 p-7">
                <div class="text-center"><FitScore score={selected.fit.overallScore} size="lg" /><p class="mt-2 text-sm font-semibold">{verdictLabel(selected.fit.verdict)}</p><p class="mt-1 text-[11px] body-muted">置信度 {selected.fit.confidence}%</p></div>
                <div><div class="flex flex-wrap items-center gap-2"><span class={selected.fit.analysisSource === 'llm' ? 'chip-brand' : 'chip'}><Sparkles size={12} />{analysisSourceLabel(selected.fit.analysisSource)}</span>{#if selected.fit.cacheStatus === 'stale'}<span class="chip text-warning">结果已过期</span>{:else if !selected.fit.cacheStatus || selected.fit.cacheStatus === 'legacy'}<span class="chip">历史缓存</span>{:else}<span class="chip text-success">最新结果</span>{/if}</div><h3 class="mt-3 text-xl font-semibold tracking-[-0.025em]">{selected.fit.summary}</h3><p class="mt-2 text-sm leading-6 body-muted">{selected.fit.recommendation}</p><div class="mt-4 flex flex-wrap gap-2">{#each selected.fit.hardConstraints as constraint}<span class="chip" style={`color:${constraintTone(constraint.status)}`}><CheckCircle2 size={12} />{constraint.label}：{constraint.note}</span>{/each}</div></div>
              </article>
              <div class="grid grid-cols-2 gap-4">
                {#each selected.fit.dimensions as dimension}
                  <article class="panel-flat p-5"><div class="mb-3 flex items-center justify-between"><h4 class="text-sm font-semibold">{dimension.label}</h4><strong class="text-lg tracking-[-0.04em]">{dimension.score === null ? '—' : dimension.score}</strong></div><div class="h-1.5 rounded-full surface-soft"><div class="h-full rounded-full" style={`width:${dimension.score ?? 0}%;background:var(--brand)`}></div></div><p class="mt-3 text-xs leading-5 body-muted">{dimension.note}</p>{#if dimension.evidence.length}<div class="mt-3 flex flex-wrap gap-1">{#each dimension.evidence.slice(0,3) as evidence}<span class="chip px-2 py-0.5">{evidence}</span>{/each}</div>{/if}</article>
                {/each}
              </div>
              <div class="grid grid-cols-2 gap-4">
                <article class="panel p-5"><h4 class="flex items-center gap-2 text-sm font-semibold"><CheckCircle2 size={17} class="text-success" />可以主打</h4><ul class="mt-4 space-y-3">{#each selected.fit.strengths as strength}<li class="flex gap-2 text-sm leading-5"><Check size={14} class="mt-0.5 shrink-0 text-success" />{strength}</li>{/each}</ul></article>
                <article class="panel p-5"><h4 class="flex items-center gap-2 text-sm font-semibold"><XCircle size={17} class="text-warning" />需要处理的缺口</h4><ul class="mt-4 space-y-3">{#each selected.fit.gaps as gap}<li class="flex gap-2 text-sm leading-5"><X size={14} class="mt-0.5 shrink-0 text-warning" />{gap}</li>{/each}</ul></article>
              </div>
            {:else}
              <div class="panel grid min-h-[360px] place-items-center p-8 text-center"><div class="max-w-md"><span class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft text-brand"><Sparkles size={24} /></span><h3 class="section-title">还没有匹配分析</h3><p class="mt-2 text-sm leading-6 body-muted">点击分析后会优先使用已验证的模型；未配置模型或模型失败时，仍会返回本地基础匹配结果。</p><button class="btn-primary mt-5" on:click={() => analyzeSelected(false)} disabled={Boolean(analyzingJobId)}><Sparkles size={16} />{analyzingJobId === selected.id ? '正在分析…' : '分析当前岗位'}</button></div></div>
            {/if}
            <article class="rounded-2xl border p-4" style="border-color: var(--line); background: var(--brand-faint);"><div class="flex gap-2"><Info size={16} class="mt-0.5 shrink-0 text-brand" /><p class="text-xs leading-5 body-muted">只有主动点击分析时才会调用模型。发送内容仅包含当前岗位和匹配所需的简历信息。</p></div></article>
          </div>
        {/if}
      </div>
    {/if}
  </section>
</div>

{#if searchDialogOpen}
  <button class="fixed inset-0 z-40 bg-black/25 backdrop-blur-sm" on:click={() => searchDialogOpen = false} aria-label="关闭"></button>
  <div class="fixed left-1/2 top-1/2 z-50 w-[640px] -translate-x-1/2 -translate-y-1/2 panel p-6">
    <div class="mb-5 flex items-start justify-between"><div><p class="eyebrow">BOSS 直聘</p><h3 class="mt-1 text-xl font-semibold">抓取新岗位</h3><p class="mt-1 text-xs body-muted">自动连接专用 Chrome、确认登录、抓详情并去重保存。</p></div><button class="btn-icon" on:click={() => searchDialogOpen = false}><X size={17} /></button></div>
    <div class="grid grid-cols-2 gap-4">
      <label><span class="label">关键词</span><input class="input" bind:value={searchSpec.keyword} placeholder="例如：数据分析、财务会计" required aria-required="true" /><span class="mt-1 block min-h-[16px] text-[11px] text-warning">{keywordMissing ? '请输入岗位关键词后开始抓取。' : ''}</span></label>
      <label><span class="label">城市</span><select class="select" bind:value={searchSpec.city}>{#each SCRAPE_CITIES as city}<option value={city}>{city}</option>{/each}</select></label>
      <label><span class="label">抓取页数</span><select class="select" bind:value={searchSpec.pages}>{#each SCRAPE_PAGE_OPTIONS as pages}<option value={pages}>{pages} 页{pages === 1 ? '（推荐）' : ''}</option>{/each}</select></label>
      <label><span class="label">经验要求</span><select class="select" bind:value={searchSpec.experience}><option value="">不限</option><option value="104">1–3 年</option><option value="105">3–5 年</option><option value="106">5–10 年</option></select></label>
      <label><span class="label">薪资范围</span><select class="select" bind:value={searchSpec.salary}>{#each SALARY_FILTER_OPTIONS as option}<option value={option.value}>{option.label}</option>{/each}</select></label>
      <label><span class="label">公司规模</span><select class="select" bind:value={searchSpec.companyScale}>{#each COMPANY_SCALE_FILTER_OPTIONS as option}<option value={option.value}>{option.label}</option>{/each}</select></label>
    </div>
    <div class="mt-5 rounded-xl border px-4 py-3" style="border-color: color-mix(in srgb, #b7791f 35%, var(--line)); background: var(--warning-soft);">
      <div class="flex items-start gap-2"><Info size={16} class="mt-0.5 shrink-0 text-warning" /><div><p class="text-sm font-semibold">预计耗时：{scrapeDuration}</p><p class="mt-1 text-xs leading-5 body-muted">抓取期间请勿关闭应用；可以切换页面，任务不会中断。实际耗时受网络和岗位数量影响。</p></div></div>
    </div>
    <div class="mt-5 flex items-center justify-between gap-4"><p class="flex items-center gap-2 text-xs body-muted"><Info size={14} />每次抓取都会检查登录；失效时请在自动打开的窗口登录。</p><button class="btn-primary shrink-0" disabled={scrapeTaskRunning || keywordMissing} on:click={runScrape}>{#if scraping}<RefreshCw size={15} class="animate-spin" />正在启动{:else if scrapeTaskRunning}<RefreshCw size={15} class="animate-spin" />已有任务运行{:else}<Search size={15} />开始抓取{/if}</button></div>
  </div>
{/if}

{#if toast}<div class="fixed bottom-6 left-1/2 z-[60] -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift">{toast}</div>{/if}

<style>
  .job-row:hover { background: var(--brand-faint); }
  .job-row.selected { background: var(--brand-faint); box-shadow: inset 3px 0 0 var(--brand); }
  .tab { position: relative; padding: 0 0 13px; font-size: 13px; font-weight: 600; color: var(--muted); }
  .tab.active { color: var(--ink); }
  .tab.active::after { content: ''; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; border-radius: 2px; background: var(--brand); }
  @media (max-width: 1280px) {
    .filter-sidebar { width: 210px; }
    .job-list { width: 330px; }
  }
</style>
