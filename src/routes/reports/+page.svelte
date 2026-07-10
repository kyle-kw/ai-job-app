<script lang="ts">
  import { onMount } from 'svelte';
  import { BarChart3, Building2, CalendarRange, Database, Download, FileCheck2, MapPinned, RefreshCw, Sparkles, WalletCards } from 'lucide-svelte';
  import ReportBars from '$lib/components/ReportBars.svelte';
  import { backend } from '$lib/services/backend';
  import type { JobDataReport } from '$lib/types';

  let report: JobDataReport | null = null;
  let loading = true;
  let exporting = false;
  let error = '';
  let exportMessage = '';

  const salary = (value?: number | null) => value == null ? '—' : `${value.toFixed(1)}K`;
  const generatedTime = (value: string) => new Intl.DateTimeFormat('zh-CN', {
    timeZone: 'Asia/Shanghai', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false
  }).format(new Date(value));

  async function loadReport() {
    loading = true;
    error = '';
    try { report = await backend.getJobDataReport(); }
    catch (reason) { error = reason instanceof Error ? reason.message : String(reason); }
    finally { loading = false; }
  }

  async function exportReport() {
    exporting = true;
    exportMessage = '';
    try {
      const result = await backend.exportJobDataReport();
      exportMessage = `已导出：${result.path}`;
    } catch (reason) {
      exportMessage = reason instanceof Error ? reason.message : String(reason);
    } finally { exporting = false; }
  }

  onMount(loadReport);
</script>

<div class="page-content report-page">
  <header class="mb-6 flex items-start justify-between gap-5">
    <div>
      <p class="eyebrow">All jobs · Local analytics</p>
      <h2 class="page-title mt-1">全量岗位数据报告</h2>
      <p class="mt-2 max-w-2xl text-sm leading-6 body-muted">参考 DataAnalysis 的统计口径，分析 SQLite 中全部去重岗位。计算在本机完成，不调用 LLM。</p>
    </div>
    <div class="flex shrink-0 gap-2">
      <button class="btn" on:click={loadReport} disabled={loading}><RefreshCw size={15} class={loading ? 'animate-spin' : ''} />重新生成</button>
      <button class="btn-primary" on:click={exportReport} disabled={exporting || !report?.totalJobs}><Download size={15} />{exporting ? '正在导出' : '导出 HTML'}</button>
    </div>
  </header>

  {#if exportMessage}
    <div class="mb-5 rounded-xl border px-4 py-3 text-xs leading-5" style="border-color: var(--line); background: var(--brand-faint);">{exportMessage}</div>
  {/if}

  {#if loading}
    <div class="space-y-5"><div class="skeleton h-40 rounded-2xl"></div><div class="grid grid-cols-4 gap-4">{#each [1,2,3,4] as _}<div class="skeleton h-28 rounded-2xl"></div>{/each}</div><div class="grid grid-cols-2 gap-5"><div class="skeleton h-80 rounded-2xl"></div><div class="skeleton h-80 rounded-2xl"></div></div></div>
  {:else if error}
    <section class="panel grid min-h-72 place-items-center p-8 text-center"><div><Database size={28} class="mx-auto mb-3 text-brand" /><h3 class="section-title">报告生成失败</h3><p class="mt-2 text-sm body-muted">{error}</p><button class="btn mt-5" on:click={loadReport}>重试</button></div></section>
  {:else if !report || report.totalJobs === 0}
    <section class="panel grid min-h-[420px] place-items-center p-8 text-center"><div class="max-w-md"><span class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl" style="background: var(--brand-soft); color: var(--brand);"><BarChart3 size={25} /></span><h3 class="text-lg font-semibold">岗位库还没有数据</h3><p class="mt-2 text-sm leading-6 body-muted">完成至少一轮岗位抓取后，这里会自动汇总薪资、经验、学历、技能组合、行业与公司分布。</p><a href="/" class="btn-primary mt-5">返回工作台抓取岗位</a></div></section>
  {:else}
    <section class="relative overflow-hidden rounded-[24px] border p-7 shadow-panel" style="border-color: var(--line); background: linear-gradient(120deg, var(--panel), var(--brand-faint));">
      <div class="dot-grid pointer-events-none absolute inset-y-0 right-0 w-1/3 opacity-40"></div>
      <div class="relative flex items-end justify-between gap-8">
        <div><div class="mb-3 inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-semibold" style="background: var(--brand-soft); color: var(--brand);"><Sparkles size={13} />本地全量分析</div><h3 class="text-[24px] font-semibold tracking-[-0.03em]">从 {report.totalJobs} 个去重岗位看市场真正需要什么</h3><p class="mt-2 text-sm body-muted">数据范围 {report.dataFrom ?? '未知'} 至 {report.dataTo ?? '未知'} · {generatedTime(report.generatedAt)} 生成</p></div>
        <div class="flex items-center gap-2 text-xs body-muted"><CalendarRange size={15} />Asia/Shanghai</div>
      </div>
    </section>

    <section class="mt-5 grid grid-cols-4 gap-4">
      <article class="panel-flat p-5"><div class="mb-4 flex items-center justify-between"><span class="text-xs font-semibold body-muted">有效岗位样本</span><Database size={17} class="text-brand" /></div><strong class="text-[27px] font-semibold tabular-nums">{report.totalJobs}</strong><p class="mt-1 text-xs body-muted">按岗位 ID 去重</p></article>
      <article class="panel-flat p-5"><div class="mb-4 flex items-center justify-between"><span class="text-xs font-semibold body-muted">招聘公司</span><Building2 size={17} class="text-brand" /></div><strong class="text-[27px] font-semibold tabular-nums">{report.totalCompanies}</strong><p class="mt-1 text-xs body-muted">覆盖 {report.totalCities} 个城市</p></article>
      <article class="panel-flat p-5"><div class="mb-4 flex items-center justify-between"><span class="text-xs font-semibold body-muted">月薪中点中位数</span><WalletCards size={17} class="text-brand" /></div><strong class="text-[27px] font-semibold tabular-nums">{salary(report.salary.medianMidK)}</strong><p class="mt-1 text-xs body-muted">{report.salary.sampleCount} 个可解析样本</p></article>
      <article class="panel-flat p-5"><div class="mb-4 flex items-center justify-between"><span class="text-xs font-semibold body-muted">岗位详情覆盖</span><FileCheck2 size={17} class="text-brand" /></div><strong class="text-[27px] font-semibold tabular-nums">{report.detailCoverage.toFixed(1)}%</strong><p class="mt-1 text-xs body-muted">{report.detailJobs} 个岗位含 JD</p></article>
    </section>

    <section class="mt-5 panel p-6">
      <div class="mb-4 flex items-center gap-2"><Sparkles size={17} class="text-brand" /><h3 class="section-title">先看结论</h3></div>
      <div class="grid grid-cols-2 gap-x-8 gap-y-3">{#each report.insights as insight, index}<div class="flex gap-3 text-sm leading-6"><span class="mt-2 h-1.5 w-1.5 shrink-0 rounded-full" style="background: var(--brand);"></span><p>{insight}</p></div>{/each}</div>
    </section>

    <section class="mt-7">
      <div class="mb-3"><p class="eyebrow">Skill demand</p><h3 class="section-title mt-1">技能需求与共现组合</h3></div>
      <div class="grid grid-cols-2 gap-5"><article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">岗位需要哪些技能</h4><ReportBars rows={report.topSkills} limit={14} /></article><article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">技能共现组合</h4><ReportBars rows={report.skillPairs} limit={10} /></article></div>
    </section>

    <section class="mt-7">
      <div class="mb-3"><p class="eyebrow">Candidate requirements</p><h3 class="section-title mt-1">薪资与候选人门槛</h3></div>
      <div class="grid grid-cols-3 gap-5">
        <article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">薪资区间分布</h4><ReportBars rows={report.salary.bands} /><div class="mt-5 grid grid-cols-3 gap-2 border-t pt-4 text-center" style="border-color: var(--line);"><div><strong class="block text-sm">{salary(report.salary.medianLowK)}</strong><span class="text-[11px] body-muted">下限中位数</span></div><div><strong class="block text-sm">{salary(report.salary.medianHighK)}</strong><span class="text-[11px] body-muted">上限中位数</span></div><div><strong class="block text-sm">{report.salary.extraMonthsCount}</strong><span class="text-[11px] body-muted">13 薪及以上</span></div></div></article>
        <article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">经验要求</h4><ReportBars rows={report.experience} /></article>
        <article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">学历要求</h4><ReportBars rows={report.degree} /></article>
      </div>
      {#if report.salaryByExperience.length > 0}<article class="panel mt-5 p-6"><h4 class="mb-5 text-sm font-semibold">不同经验要求的薪资中位数</h4><div class="grid grid-cols-5 gap-3">{#each report.salaryByExperience.slice(0, 5) as item}<div class="rounded-xl p-4 surface-soft"><span class="text-xs body-muted">{item.label}</span><strong class="mt-2 block text-xl tabular-nums">{item.medianK.toFixed(1)}K</strong><span class="text-[11px] body-muted">{item.count} 个薪资样本</span></div>{/each}</div></article>{/if}
    </section>

    <section class="mt-7">
      <div class="mb-3"><p class="eyebrow">Market structure</p><h3 class="section-title mt-1">市场结构</h3></div>
      <div class="grid grid-cols-2 gap-5"><article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">岗位方向</h4><ReportBars rows={report.roles} /></article><article class="panel p-6"><h4 class="mb-5 flex items-center gap-2 text-sm font-semibold"><MapPinned size={15} class="text-brand" />城市分布</h4><ReportBars rows={report.cities} /></article><article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">行业分布</h4><ReportBars rows={report.industries} /></article><article class="panel p-6"><h4 class="mb-5 text-sm font-semibold">公司规模</h4><ReportBars rows={report.companyScales} /></article><article class="panel col-span-2 p-6"><h4 class="mb-5 text-sm font-semibold">常见福利</h4><div class="max-w-4xl"><ReportBars rows={report.welfare} limit={12} /></div></article></div>
    </section>

    <footer class="mt-6 flex items-center justify-between border-t pt-5 text-xs body-muted" style="border-color: var(--line);"><span>统计频率按“至少一个字段提及该项”的岗位计数，单个岗位不会重复加权。</span><span>全部计算在本机完成</span></footer>
  {/if}
</div>

<style>
  @media (max-width: 1280px) {
    .report-page :global(.grid-cols-4) { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .report-page :global(.grid-cols-3) { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .report-page :global(.grid-cols-5) { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
</style>
