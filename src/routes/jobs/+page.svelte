<script lang="ts">
  import { ArrowUpRight, BriefcaseBusiness, Check, CheckCircle2, ChevronDown, Clipboard, FileCheck2, FileText, Filter, Info, MapPin, MessageCircle, RefreshCw, Search, Sparkles, WandSparkles, X, XCircle } from 'lucide-svelte';
  import { page } from '$app/stores';
  import FitScore from '$lib/components/FitScore.svelte';
  import MarkdownView from '$lib/components/MarkdownView.svelte';
  import { backend } from '$lib/services/backend';
  import { refresh, snapshot, startScrape } from '$lib/stores/app';
  import type { Job, ResumePatch, SearchSpec } from '$lib/types';

  let selectedId = '';
  let query = '';
  let minScore = 0;
  let onlyNew = false;
  let activeTab: 'description' | 'fit' | 'tailor' = 'description';
  let scraping = false;
  let showSearch = false;
  let searchSpec: SearchSpec = { keyword: 'AI Agent', city: '上海', pages: 3 };
  let greetingBusy = false;
  let tailoringBusy = false;
  let renderBusy = false;
  let toast = '';
  let localPatches: ResumePatch[] = [];

  $: jobs = $snapshot.jobs.filter((job) => {
    const text = `${job.title} ${job.company} ${job.skills.join(' ')}`.toLocaleLowerCase();
    return text.includes(query.toLocaleLowerCase()) && (job.fit?.overallScore ?? 0) >= minScore && (!onlyNew || job.isNew);
  }).sort((a, b) => (b.fit?.overallScore ?? 0) - (a.fit?.overallScore ?? 0));
  $: requestedId = $page.url.searchParams.get('job');
  $: if (!selectedId && jobs.length) selectedId = requestedId && jobs.some((job) => job.id === requestedId) ? requestedId : jobs[0].id;
  $: selected = $snapshot.jobs.find((job) => job.id === selectedId) ?? jobs[0] as Job | undefined;
  $: if (selected?.patches && localPatches.length === 0) localPatches = selected.patches;

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => { if (toast === message) toast = ''; }, 2600);
  }

  async function runScrape() {
    scraping = true;
    try { await startScrape(searchSpec); showSearch = false; showToast('抓取任务已启动，可继续浏览岗位'); } finally { scraping = false; }
  }

  async function greeting() {
    if (!selected) return;
    greetingBusy = true;
    try { await backend.generateGreeting(selected.id); await refresh(); showToast('招呼语已生成'); } finally { greetingBusy = false; }
  }

  async function copyGreeting() {
    if (!selected?.greeting) return;
    await navigator.clipboard.writeText(selected.greeting);
    showToast('已复制招呼语');
  }

  async function propose() {
    if (!selected) return;
    tailoringBusy = true;
    try { localPatches = await backend.proposeTailoring(selected.id); activeTab = 'tailor'; } finally { tailoringBusy = false; }
  }

  async function updatePatch(patch: ResumePatch, status: ResumePatch['status']) {
    if (!selected) return;
    localPatches = await backend.updatePatch(selected.id, patch.id, status, patch.after);
  }

  async function acceptAll() {
    for (const patch of localPatches.filter((item) => item.status === 'pending')) await updatePatch(patch, 'accepted');
  }

  async function renderTailored() {
    if (!selected) return;
    renderBusy = true;
    try { const result = await backend.renderResume(selected.id); showToast(`已生成 ${result.fileName}`); } finally { renderBusy = false; }
  }

  const verdictLabel = (verdict?: string) => ({ strong: '高度匹配', good: '值得申请', moderate: '谨慎评估', weak: '匹配偏弱', poor: '不建议' }[verdict ?? ''] ?? '待分析');
  const constraintTone = (status: string) => status === 'pass' ? 'var(--success)' : status === 'fail' ? 'var(--danger)' : 'var(--warning)';
</script>

<div class="flex h-[calc(100vh-74px)] min-h-[646px] overflow-hidden">
  <aside class="filter-sidebar scrollbar-thin w-[250px] shrink-0 overflow-y-auto border-r p-4" style="border-color: var(--line); background: color-mix(in srgb, var(--canvas) 72%, var(--panel));">
    <button class="btn-primary mb-4 w-full" on:click={() => showSearch = true}><Search size={16} />抓取新岗位</button>
    <label class="relative block">
      <Search size={15} class="pointer-events-none absolute left-3 top-3 body-muted" />
      <input class="input pl-9" bind:value={query} placeholder="搜索岗位或公司" />
    </label>
    <div class="mt-5 flex items-center justify-between"><span class="eyebrow">筛选条件</span><Filter size={14} class="body-muted" /></div>
    <label class="mt-4 block"><span class="label flex justify-between"><span>最低匹配度</span><span class="text-brand">{minScore}%</span></span><input class="w-full accent-[var(--brand)]" type="range" min="0" max="90" step="10" bind:value={minScore} /></label>
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
      <div class="grid min-h-[400px] place-items-center px-8 text-center"><div><BriefcaseBusiness size={26} class="mx-auto mb-3 body-muted" /><p class="text-sm font-semibold">没有符合条件的岗位</p><p class="mt-1 text-xs body-muted">降低匹配度或清除筛选条件。</p></div></div>
    {:else}
      {#each jobs as job}
        <button class:selected={selected?.id === job.id} class="job-row w-full border-b px-4 py-4 text-left transition" style="border-color: var(--line);" on:click={() => { selectedId = job.id; activeTab = 'description'; localPatches = job.patches ?? []; }}>
          <div class="flex gap-3">
            <FitScore score={job.fit?.overallScore ?? 0} size="sm" />
            <div class="min-w-0 flex-1">
              <div class="flex items-start justify-between gap-2"><h3 class="line-clamp-1 text-sm font-semibold">{job.title}</h3>{#if job.isNew}<span class="chip-brand shrink-0 px-2 py-0.5">新</span>{/if}</div>
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
        <div class="flex items-start justify-between gap-6 pb-4">
          <div class="min-w-0">
            <div class="flex items-center gap-2"><h2 class="truncate text-[22px] font-semibold tracking-[-0.035em]">{selected.title}</h2>{#if selected.isNew}<span class="chip-brand">本次新增</span>{/if}</div>
            <p class="mt-1 text-sm body-muted">{selected.company} · {selected.companyScale} · {selected.industry}</p>
            <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm"><strong class="text-brand">{selected.salary}</strong><span class="flex items-center gap-1 body-muted"><MapPin size={14} />{selected.location}</span><span class="body-muted">{selected.experience} · {selected.degree}</span></div>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <a class="btn" href={selected.sourceUrl} target="_blank" rel="noreferrer">查看原岗位 <ArrowUpRight size={15} /></a>
            <button class="btn-primary" on:click={propose} disabled={tailoringBusy}>{tailoringBusy ? '正在分析…' : '生成专岗简历'} <WandSparkles size={15} /></button>
          </div>
        </div>
        <nav class="flex gap-6" aria-label="岗位详情切换">
          <button class:active={activeTab === 'description'} class="tab" on:click={() => activeTab = 'description'}>岗位详情</button>
          <button class:active={activeTab === 'fit'} class="tab" on:click={() => activeTab = 'fit'}>匹配分析 <span class="ml-1 text-brand">{selected.fit?.overallScore}%</span></button>
          <button class:active={activeTab === 'tailor'} class="tab" on:click={() => activeTab = 'tailor'}>专岗简历 {#if localPatches.length}<span class="ml-1 rounded-full px-1.5 py-0.5 text-[10px]" style="background: var(--brand-soft);">{localPatches.length}</span>{/if}</button>
        </nav>
      </div>

      <div class="mx-auto max-w-[920px] p-6">
        {#if activeTab === 'description'}
          <div class="grid grid-cols-[1fr_280px] gap-5 animate-lift">
            <article class="panel p-6">
              <div class="mb-5 flex items-center justify-between"><h3 class="section-title">职位描述</h3><span class="chip">最后更新于今天</span></div>
              <div class="whitespace-pre-line text-sm leading-7">{selected.description}</div>
              {#if selected.skills.length}<div class="mt-6"><p class="label">岗位技能</p><div class="flex flex-wrap gap-2">{#each selected.skills as skill}<span class="chip-brand">{skill}</span>{/each}</div></div>{/if}
              {#if selected.welfare.length}<div class="mt-6"><p class="label">福利待遇</p><div class="flex flex-wrap gap-2">{#each selected.welfare as item}<span class="chip">{item}</span>{/each}</div></div>{/if}
            </article>
            <aside class="space-y-4">
              <article class="panel p-5">
                <p class="eyebrow">招聘联系人</p><div class="mt-3 flex items-center gap-3"><div class="grid h-10 w-10 place-items-center rounded-full bg-brand-soft font-semibold text-brand">{selected.bossName?.slice(0,1) ?? 'B'}</div><div><p class="text-sm font-semibold">{selected.bossName ?? '招聘负责人'}</p><p class="text-xs body-muted">{selected.bossTitle ?? selected.company}</p></div></div>
                {#if selected.greeting}
                  <div class="mt-4 rounded-xl p-3 text-xs leading-5" style="background: var(--brand-faint);">{selected.greeting}</div>
                  <button class="btn mt-3 w-full" on:click={copyGreeting}><Clipboard size={14} />复制招呼语</button>
                {:else}
                  <button class="btn mt-4 w-full" disabled={greetingBusy} on:click={greeting}><MessageCircle size={15} />{greetingBusy ? '正在生成…' : '生成一句招呼语'}</button>
                {/if}
              </article>
              <article class="rounded-2xl border p-4" style="border-color: var(--line); background: var(--warning-soft);"><div class="flex gap-2"><Info size={16} class="mt-0.5 shrink-0 text-warning" /><p class="text-xs leading-5 body-muted">招聘网站内容可能随时变化，请在投递前打开原岗位确认有效性。</p></div></article>
            </aside>
          </div>
        {:else if activeTab === 'fit'}
          {#if selected.fit}
            <div class="space-y-5 animate-lift">
              <article class="panel grid grid-cols-[150px_1fr] items-center gap-8 p-7">
                <div class="text-center"><FitScore score={selected.fit.overallScore} size="lg" /><p class="mt-2 text-sm font-semibold">{verdictLabel(selected.fit.verdict)}</p><p class="mt-1 text-[11px] body-muted">置信度 {selected.fit.confidence}%</p></div>
                <div><span class="chip-brand"><Sparkles size={12} />AI 匹配结论</span><h3 class="mt-3 text-xl font-semibold tracking-[-0.025em]">{selected.fit.summary}</h3><p class="mt-2 text-sm leading-6 body-muted">{selected.fit.recommendation}</p><div class="mt-4 flex flex-wrap gap-2">{#each selected.fit.hardConstraints as constraint}<span class="chip" style={`color:${constraintTone(constraint.status)}`}><CheckCircle2 size={12} />{constraint.label}：{constraint.note}</span>{/each}</div></div>
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
            </div>
          {/if}
        {:else}
          <div class="animate-lift">
            {#if localPatches.length === 0}
              <div class="panel grid min-h-[430px] place-items-center p-8 text-center"><div class="max-w-md"><span class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft text-brand"><FileCheck2 size={24} /></span><h3 class="section-title">还没有专岗修改建议</h3><p class="mt-2 text-sm leading-6 body-muted">系统会基于已确认事实给出逐条改动。主简历始终保持不变。</p><button class="btn-primary mt-5" on:click={propose} disabled={tailoringBusy}><WandSparkles size={16} />{tailoringBusy ? '正在生成…' : '生成修改建议'}</button></div></div>
            {:else}
              <div class="mb-4 flex items-center justify-between"><div><h3 class="section-title">审核修改建议</h3><p class="mt-1 text-xs body-muted">每一处改写都必须有事实依据；接受后才会进入专岗版本。</p></div><div class="flex gap-2"><button class="btn" on:click={acceptAll}><Check size={15} />全部接受</button><button class="btn-primary" disabled={renderBusy || !localPatches.some((patch) => patch.status === 'accepted')} on:click={renderTailored}><FileText size={15} />{renderBusy ? '正在渲染…' : '生成 PDF'}</button></div></div>
              <div class="space-y-4">
                {#each localPatches as patch, index}
                  <article class="panel overflow-hidden">
                    <div class="flex items-center justify-between border-b px-5 py-3" style="border-color: var(--line); background: var(--panel-soft);"><div class="flex items-center gap-2"><span class="grid h-6 w-6 place-items-center rounded-lg text-xs font-semibold bg-panel">{index + 1}</span><span class="text-sm font-semibold">{patch.section}</span></div><span class={patch.status === 'accepted' ? 'chip-brand' : 'chip'}>{patch.status === 'accepted' ? '已接受' : patch.status === 'rejected' ? '已拒绝' : '待审核'}</span></div>
                    <div class="grid grid-cols-2 divide-x" style="border-color: var(--line);">
                      <div class="p-5"><p class="eyebrow mb-2">原文</p><p class="text-sm leading-6 body-muted">{patch.before}</p></div>
                      <div class="p-5"><p class="eyebrow mb-2 text-brand">建议</p><textarea class="textarea min-h-[112px] leading-6" bind:value={patch.after}></textarea></div>
                    </div>
                    <div class="flex items-center justify-between border-t px-5 py-3" style="border-color: var(--line);"><p class="max-w-[65%] text-xs leading-5 body-muted"><strong class="text-ink">修改依据：</strong>{patch.rationale}</p><div class="flex gap-2"><button class="btn h-9" on:click={() => updatePatch(patch, 'rejected')}><X size={14} />拒绝</button><button class="btn-primary h-9" on:click={() => updatePatch(patch, 'accepted')}><Check size={14} />接受</button></div></div>
                  </article>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </section>
</div>

{#if showSearch}
  <button class="fixed inset-0 z-40 bg-black/25 backdrop-blur-sm" on:click={() => showSearch = false} aria-label="关闭"></button>
  <div class="fixed left-1/2 top-1/2 z-50 w-[560px] -translate-x-1/2 -translate-y-1/2 panel p-6">
    <div class="mb-5 flex items-start justify-between"><div><p class="eyebrow">BOSS 直聘</p><h3 class="mt-1 text-xl font-semibold">抓取新岗位</h3><p class="mt-1 text-xs body-muted">自动连接专用 Chrome、确认登录、抓详情并去重保存。</p></div><button class="btn-icon" on:click={() => showSearch = false}><X size={17} /></button></div>
    <div class="grid grid-cols-2 gap-4"><label><span class="label">关键词</span><input class="input" bind:value={searchSpec.keyword} /></label><label><span class="label">城市</span><input class="input" bind:value={searchSpec.city} /></label><label><span class="label">抓取页数</span><select class="select" bind:value={searchSpec.pages}><option value={1}>1 页</option><option value={3}>3 页（推荐）</option><option value={5}>5 页</option><option value={10}>10 页</option></select></label><label><span class="label">经验要求</span><select class="select" bind:value={searchSpec.experience}><option value="">不限</option><option value="104">1–3 年</option><option value="105">3–5 年</option><option value="106">5–10 年</option></select></label></div>
    <div class="mt-6 flex items-center justify-between"><p class="flex items-center gap-2 text-xs body-muted"><Info size={14} />每次抓取都会检查登录；失效时请在自动打开的窗口登录。</p><button class="btn-primary" disabled={scraping} on:click={runScrape}>{#if scraping}<RefreshCw size={15} class="animate-spin" />等待登录/抓取{:else}<Search size={15} />开始抓取{/if}</button></div>
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
