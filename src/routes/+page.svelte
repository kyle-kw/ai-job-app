<script lang="ts">
  import { ArrowRight, Bot, BriefcaseBusiness, Check, ChevronRight, Circle, FileText, KeyRound, LockKeyhole, Play, Search, Sparkles, Upload, UserRoundCheck } from 'lucide-svelte';
  import FitScore from '$lib/components/FitScore.svelte';
  import MarkdownView from '$lib/components/MarkdownView.svelte';
  import { importResume, loading, snapshot, startScrape } from '$lib/stores/app';

  let keyword = 'AI Agent';
  let city = '上海';
  let busyAction: string | null = null;
  let fileInput: HTMLInputElement;

  const readiness = [
    { key: 'ai', title: '连接 AI', description: '用于简历解析、匹配分析与材料生成', icon: Bot, href: '/settings' },
    { key: 'resume', title: '准备主简历', description: '导入 PDF、DOCX 或 RenderCV YAML', icon: FileText, href: '/resume' },
    { key: 'boss', title: '抓取时连接 BOSS', description: '每次抓取都会自动打开或复用专用 Chrome，并检查登录状态', icon: UserRoundCheck, href: null }
  ] as const;

  async function quickScrape() {
    busyAction = 'scrape';
    try { await startScrape({ keyword, city, pages: 3 }); } finally { busyAction = null; }
  }

  async function pickResume(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    busyAction = 'resume';
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let binary = '';
      for (const byte of bytes) binary += String.fromCharCode(byte);
      await importResume({ fileName: file.name, contentBase64: btoa(binary) });
    } finally { busyAction = null; }
  }

  $: topJobs = [...$snapshot.jobs].sort((a, b) => (b.fit?.overallScore ?? 0) - (a.fit?.overallScore ?? 0)).slice(0, 3);
  $: latestRun = $snapshot.scrapeRuns[0];
  $: readyCount = Number($snapshot.readiness.ai) + Number($snapshot.readiness.resume);
</script>

<div class="page-content">
  {#if $loading}
    <div class="space-y-5">
      <div class="skeleton h-28 rounded-2xl"></div>
      <div class="grid grid-cols-3 gap-4">{#each [1,2,3] as _}<div class="skeleton h-36 rounded-2xl"></div>{/each}</div>
      <div class="skeleton h-80 rounded-2xl"></div>
    </div>
  {:else}
    <section class="relative overflow-hidden rounded-[24px] border px-7 py-6 shadow-panel" style="border-color: var(--line); background: linear-gradient(115deg, var(--panel) 0%, var(--brand-faint) 100%);">
      <div class="dot-grid pointer-events-none absolute inset-y-0 right-0 w-[42%] opacity-40"></div>
      <div class="relative flex items-end justify-between gap-8">
        <div class="max-w-[650px]">
          <div class="mb-3 inline-flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-semibold" style="background: var(--brand-soft); color: var(--brand);"><Sparkles size={13} />求职工作台</div>
          <h2 class="text-[29px] font-semibold tracking-[-0.04em]">晚上好，{$snapshot.resume?.name || '欢迎回来'}</h2>
          <p class="mt-2 text-sm leading-6 body-muted">
            {#if $snapshot.readiness.resume && $snapshot.jobs.length > 0}
              已经有 {$snapshot.jobs.length} 个岗位可以比较。先看高匹配机会，再决定把时间花在哪里。
            {:else}
              先完成任意一项。简历解析和岗位抓取可以同时进行，不需要等候。
            {/if}
          </p>
        </div>
        <a href="/jobs" class="btn-primary shrink-0">查看推荐岗位 <ArrowRight size={16} /></a>
      </div>
    </section>

    <section class="mt-7">
      <div class="mb-3 flex items-end justify-between">
        <div><p class="eyebrow">准备状态</p><h3 class="section-title mt-1">两项准备，抓取时自动连接 BOSS</h3></div>
        <span class="text-xs body-muted">{readyCount} / 2 已完成</span>
      </div>
      <div class="grid grid-cols-3 gap-4">
        {#each readiness as item, index}
          {@const ready = item.key !== 'boss' && $snapshot.readiness[item.key]}
          <article class="panel-flat animate-lift p-5" style={`animation-delay:${index * 55}ms`}>
            <div class="mb-4 flex items-start justify-between">
              <span class="grid h-10 w-10 place-items-center rounded-xl" style={`background:${ready ? 'var(--brand-soft)' : 'var(--panel-soft)'}; color:${ready ? 'var(--brand)' : 'var(--muted)'}`}><svelte:component this={item.icon} size={19} /></span>
              <span class={ready ? 'chip-brand' : 'chip'}>{#if item.key === 'boss'}<Circle size={10} /> 随抓随连{:else if ready}<Check size={12} /> 已就绪{:else}<Circle size={10} /> 待完成{/if}</span>
            </div>
            <h4 class="text-sm font-semibold">{item.title}</h4>
            <p class="mt-1 min-h-10 text-xs leading-5 body-muted">{item.description}</p>
            {#if item.key === 'boss'}
              <div class="mt-4 flex items-center gap-1 text-xs font-semibold text-brand"><Check size={14} />已合并到“开始抓取”</div>
            {:else if ready}
              <div class="mt-4 flex items-center gap-1 text-xs font-semibold" style="color: var(--success);"><Check size={14} />无需再次配置</div>
            {:else if item.key === 'resume'}
              <button class="btn-ghost -ml-3 mt-3 text-brand" disabled={busyAction === 'resume'} on:click={() => fileInput.click()}>{busyAction === 'resume' ? '正在解析…' : '选择简历'} <ChevronRight size={14} /></button>
            {:else}
              <a class="btn-ghost -ml-3 mt-3 text-brand" href={item.href}>去连接 <ChevronRight size={14} /></a>
            {/if}
          </article>
        {/each}
      </div>
      <input bind:this={fileInput} class="hidden" type="file" accept=".pdf,.docx,.yaml,.yml" on:change={pickResume} />
    </section>

    <section class="mt-7 grid grid-cols-[1.35fr_.65fr] gap-5">
      <article class="panel p-6">
        <div class="mb-5 flex items-start justify-between">
          <div><p class="eyebrow">并行入口</p><h3 class="section-title mt-1">开始一轮岗位搜索</h3><p class="mt-1 text-xs body-muted">默认 3 页，自动抓详情、去重并生成市场观察。</p></div>
          <span class="grid h-10 w-10 place-items-center rounded-xl" style="background: var(--brand-soft); color: var(--brand);"><Search size={19} /></span>
        </div>
        <div class="grid grid-cols-[1fr_150px_auto] gap-3">
          <label><span class="label">关键词</span><input class="input" bind:value={keyword} placeholder="例如：AI Agent" /></label>
          <label><span class="label">城市</span><input class="input" bind:value={city} placeholder="上海" /></label>
          <div class="flex items-end"><button class="btn-primary w-full" disabled={busyAction === 'scrape'} on:click={quickScrape}>{#if busyAction === 'scrape'}<Circle class="animate-spin" size={16} /> 等待登录/抓取{:else}<Play size={15} /> 开始抓取{/if}</button></div>
        </div>
        <div class="mt-5 flex items-center gap-3 border-t pt-4 text-xs body-muted" style="border-color: var(--line);">
          <LockKeyhole size={15} class="text-brand" />
          <span>点击后自动连接 BOSS 专用 Chrome；遇到登录或验证码会暂停并等待你处理。</span>
        </div>
      </article>

      <article class="panel p-6">
        <p class="eyebrow">简历入口</p>
        <h3 class="section-title mt-1">{ $snapshot.resume ? '主简历已建立' : '把现有简历交给我们'}</h3>
        {#if $snapshot.resume}
          <div class="mt-4 flex items-center gap-3 rounded-xl p-3 surface-soft">
            <span class="grid h-10 w-10 place-items-center rounded-lg bg-panel text-brand"><FileText size={18} /></span>
            <div class="min-w-0"><p class="truncate text-sm font-semibold">{$snapshot.resume.sourceFileName}</p><p class="text-[11px] body-muted">版本 {$snapshot.resume.version} · {$snapshot.resume.skills.length} 项技能</p></div>
          </div>
          <a href="/resume" class="btn mt-4 w-full">检查并完善简历 <ArrowRight size={15} /></a>
        {:else}
          <button class="mt-4 grid w-full place-items-center rounded-xl border border-dashed p-5 text-center transition hover:border-brand" style="border-color: var(--line-strong);" on:click={() => fileInput.click()}>
            <Upload size={21} class="mb-2 text-brand" /><span class="text-sm font-semibold">选择 PDF、DOCX 或 YAML</span><span class="mt-1 text-[11px] body-muted">仅扫描件暂不支持</span>
          </button>
        {/if}
      </article>
    </section>

    {#if topJobs.length > 0}
      <section class="mt-8">
        <div class="mb-3 flex items-end justify-between"><div><p class="eyebrow">优先处理</p><h3 class="section-title mt-1">与你最接近的机会</h3></div><a href="/jobs" class="flex items-center gap-1 text-xs font-semibold text-brand">查看全部 <ArrowRight size={14} /></a></div>
        <div class="grid grid-cols-3 gap-4">
          {#each topJobs as job}
            <a href={`/jobs?job=${job.id}`} class="panel-flat group p-4 transition hover:-translate-y-0.5 hover:shadow-panel">
              <div class="flex gap-3">
                <FitScore score={job.fit?.overallScore ?? 0} size="sm" />
                <div class="min-w-0"><h4 class="truncate text-sm font-semibold group-hover:text-brand">{job.title}</h4><p class="mt-0.5 truncate text-xs body-muted">{job.company} · {job.location.split('·')[0]}</p></div>
              </div>
              <div class="my-3 divider"></div>
              <div class="flex items-center justify-between"><span class="text-sm font-semibold" style="color: var(--brand);">{job.salary}</span><span class="text-[11px] body-muted">{job.experience} · {job.degree}</span></div>
            </a>
          {/each}
        </div>
      </section>
    {/if}

    {#if latestRun?.reportMarkdown && latestRun.totalSeen > 0}
      <section class="mt-8 panel p-6">
        <div class="mb-4 flex items-center justify-between"><div><p class="eyebrow">最近一轮 · {latestRun.city}</p><h3 class="section-title mt-1">岗位市场观察</h3></div><span class="chip"><BriefcaseBusiness size={13} /> {latestRun.totalSeen} 个岗位</span></div>
        <MarkdownView source={latestRun.reportMarkdown} />
      </section>
    {/if}

    {#if !$snapshot.settings.privacyAcknowledged}
      <section class="mt-7 flex items-start gap-4 rounded-2xl border p-4" style="border-color: var(--line); background: var(--blue-soft);">
        <span class="mt-0.5 text-[var(--blue)]"><KeyRound size={19} /></span>
        <div class="flex-1"><p class="text-sm font-semibold">发送给模型之前，我们会明确告诉你</p><p class="mt-1 text-xs leading-5 body-muted">简历与 JD 只会发送到你选择的模型服务。应用不上传本地岗位库，也不启用遥测。</p></div>
        <a class="btn h-9" href="/settings#privacy">查看隐私设置</a>
      </section>
    {/if}
  {/if}
</div>
