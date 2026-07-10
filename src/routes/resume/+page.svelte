<script lang="ts">
  import { AlertCircle, Check, CheckCircle2, ChevronDown, Download, FileText, History, Mail, MapPin, PencilLine, Phone, Plus, Save, Sparkles, Upload, UserRound, WandSparkles } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import { importResume, refresh, savePreferences, snapshot } from '$lib/stores/app';
  import type { JobPreferences, ResumeProfile } from '$lib/types';

  let activeSection: 'content' | 'preferences' | 'facts' = 'content';
  let draft: ResumeProfile | null = null;
  let draftId = '';
  let saving = false;
  let importing = false;
  let rendering = false;
  let toast = '';
  let fileInput: HTMLInputElement;
  let targetRolesText = '';
  let citiesText = '';
  let energizingText = '';
  let drainingText = '';
  let constraintsText = '';

  $: if ($snapshot.resume && $snapshot.resume.id !== draftId) {
    draft = structuredClone($snapshot.resume);
    draftId = $snapshot.resume.id;
    syncPreferenceTexts($snapshot.resume.preferences);
  }

  function syncPreferenceTexts(preferences: JobPreferences) {
    targetRolesText = preferences.targetRoles.join('、'); citiesText = preferences.cities.join('、');
    energizingText = preferences.energizingTasks.join('、'); drainingText = preferences.drainingTasks.join('、'); constraintsText = preferences.hardConstraints.join('、');
  }
  const split = (value: string) => value.split(/[、,，\n]/).map((item) => item.trim()).filter(Boolean);
  function showToast(message: string) { toast = message; window.setTimeout(() => toast === message && (toast = ''), 2400); }

  async function save() {
    if (!draft) return;
    saving = true;
    try { draft = await backend.saveResume(draft); await refresh(); showToast('主简历已保存'); } finally { saving = false; }
  }

  async function savePrefs() {
    if (!draft) return;
    const preferences: JobPreferences = { ...draft.preferences, targetRoles: split(targetRolesText), cities: split(citiesText), energizingTasks: split(energizingText), drainingTasks: split(drainingText), hardConstraints: split(constraintsText) };
    await savePreferences(preferences); draft.preferences = preferences; showToast('求职偏好已保存，匹配置信度将更新');
  }

  async function pickResume(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0]; if (!file) return;
    importing = true;
    try { const bytes = new Uint8Array(await file.arrayBuffer()); let binary = ''; for (const byte of bytes) binary += String.fromCharCode(byte); await importResume({ fileName: file.name, contentBase64: btoa(binary) }); showToast('正在后台解析新简历'); } finally { importing = false; }
  }

  async function renderPdf() {
    rendering = true;
    try { const result = await backend.renderResume(); showToast(`已生成 ${result.fileName}`); } finally { rendering = false; }
  }
</script>

<div class="page-content h-[calc(100vh-74px)] min-h-[646px] max-w-none overflow-hidden pb-6">
  <div class="mb-5 flex items-end justify-between">
    <div><p class="eyebrow">MASTER RESUME</p><h2 class="page-title mt-1">一份可信的主简历</h2><p class="mt-1 text-sm body-muted">专岗版本从这里派生，但不会反向覆盖主简历。</p></div>
    <div class="flex gap-2"><input bind:this={fileInput} class="hidden" type="file" accept=".pdf,.docx,.yaml,.yml" on:change={pickResume} /><button class="btn" disabled={importing} on:click={() => fileInput.click()}><Upload size={15} />{importing ? '正在解析…' : '重新导入'}</button><button class="btn" disabled={!draft || rendering} on:click={renderPdf}><Download size={15} />{rendering ? '正在渲染…' : '导出 PDF'}</button><button class="btn-primary" disabled={!draft || saving} on:click={save}><Save size={15} />{saving ? '正在保存…' : '保存修改'}</button></div>
  </div>

  {#if !draft}
    <div class="panel grid min-h-[520px] place-items-center text-center"><div class="max-w-md"><span class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-brand-soft text-brand"><FileText size={27} /></span><h3 class="text-xl font-semibold">先导入一份现有简历</h3><p class="mt-2 text-sm leading-6 body-muted">支持 PDF、DOCX 和 RenderCV YAML。扫描版 PDF 请改用可复制文字的文件。</p><button class="btn-primary mt-5" on:click={() => fileInput.click()}><Upload size={16} />选择简历文件</button></div></div>
  {:else}
    <div class="grid h-[calc(100%-94px)] grid-cols-[minmax(460px,1fr)_minmax(380px,.82fr)] gap-5">
      <section class="panel flex min-h-0 flex-col overflow-hidden">
        <nav class="flex shrink-0 gap-6 border-b px-6 pt-4" style="border-color: var(--line);">
          <button class:active={activeSection === 'content'} class="tab" on:click={() => activeSection = 'content'}>简历内容</button>
          <button class:active={activeSection === 'preferences'} class="tab" on:click={() => activeSection = 'preferences'}>求职偏好</button>
          <button class:active={activeSection === 'facts'} class="tab" on:click={() => activeSection = 'facts'}>事实清单 <span class="ml-1 text-brand">{draft.facts.length}</span></button>
        </nav>
        <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6">
          {#if activeSection === 'content'}
            <div class="space-y-7 animate-lift">
              <div><div class="mb-3 flex items-center gap-2"><UserRound size={17} class="text-brand" /><h3 class="section-title">基本信息</h3></div><div class="grid grid-cols-2 gap-4"><label><span class="label">姓名</span><input class="input" bind:value={draft.name} /></label><label><span class="label">职业标题</span><input class="input" bind:value={draft.headline} /></label><label><span class="label">邮箱</span><input class="input" bind:value={draft.email} /></label><label><span class="label">电话</span><input class="input" bind:value={draft.phone} /></label><label><span class="label">所在城市</span><input class="input" bind:value={draft.location} /></label><label><span class="label">个人主页</span><input class="input" bind:value={draft.website} /></label></div></div>
              <div class="divider"></div>
              <div><div class="mb-3 flex items-center justify-between"><div class="flex items-center gap-2"><Sparkles size={17} class="text-brand" /><h3 class="section-title">个人简介</h3></div><button class="btn-ghost h-8 text-xs text-brand"><WandSparkles size={14} />AI 优化</button></div><textarea class="textarea min-h-[120px] leading-6" bind:value={draft.summary}></textarea><p class="mt-2 text-[11px] body-muted">这里是专岗优化时最先调整的内容，建议保持 3–4 行。</p></div>
              <div class="divider"></div>
              <div><div class="mb-3 flex items-center justify-between"><h3 class="section-title">技能</h3><button class="btn-ghost h-8 text-xs"><Plus size={14} />添加</button></div><div class="flex flex-wrap gap-2">{#each draft.skills as skill, index}<label class="chip-brand group cursor-text"><input class="w-[80px] bg-transparent outline-none" bind:value={draft.skills[index]} /><button class="ml-1 opacity-0 transition group-hover:opacity-100" on:click={() => draft && (draft.skills = draft.skills.filter((_, i) => i !== index))}>×</button></label>{/each}</div></div>
              <div class="divider"></div>
              <div><div class="mb-4 flex items-center justify-between"><h3 class="section-title">工作经历</h3><button class="btn-ghost h-8 text-xs"><Plus size={14} />添加经历</button></div><div class="space-y-4">{#each draft.experiences as experience}<article class="rounded-xl border p-4" style="border-color: var(--line);"><div class="grid grid-cols-2 gap-3"><input class="input font-semibold" bind:value={experience.company} /><input class="input" bind:value={experience.position} /></div><div class="mt-3 space-y-2">{#each experience.highlights as highlight, index}<div class="flex gap-2"><span class="mt-3 h-1.5 w-1.5 shrink-0 rounded-full bg-brand"></span><textarea class="textarea min-h-[66px]" bind:value={experience.highlights[index]}></textarea></div>{/each}</div></article>{/each}</div></div>
              <div class="divider"></div>
              <div><h3 class="section-title mb-4">教育经历</h3>{#each draft.education as education}<article class="grid grid-cols-2 gap-3 rounded-xl border p-4" style="border-color: var(--line);"><input class="input font-semibold" bind:value={education.institution} /><input class="input" bind:value={education.area} /><input class="input" bind:value={education.degree} /><div class="grid grid-cols-2 gap-2"><input class="input" bind:value={education.startDate} /><input class="input" bind:value={education.endDate} /></div></article>{/each}</div>
            </div>
          {:else if activeSection === 'preferences'}
            <div class="animate-lift"><div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--brand-faint);"><div class="flex gap-3"><Sparkles size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">约 60 秒，提高匹配结果的可信度</p><p class="mt-1 text-xs leading-5 body-muted">不知道的维度不会被 AI 硬猜。未填写项会降低置信度，而不会直接扣分。</p></div></div></div><div class="mt-6 space-y-5"><label><span class="label">目标岗位</span><input class="input" bind:value={targetRolesText} placeholder="AI Agent、大模型应用" /><span class="mt-1 block text-[11px] body-muted">使用顿号分隔多个方向</span></label><label><span class="label">目标城市</span><input class="input" bind:value={citiesText} placeholder="上海、杭州" /></label><label><span class="label">工作方式</span><select class="select" bind:value={draft.preferences.remotePreference}><option value="onsite">到岗办公</option><option value="hybrid">混合办公</option><option value="remote">远程办公</option><option value="flexible">均可</option></select></label><label><span class="label">让你有动力的任务</span><textarea class="textarea" bind:value={energizingText} placeholder="从 0 到 1、系统设计、Agent 工作流"></textarea></label><label><span class="label">希望减少的任务</span><textarea class="textarea" bind:value={drainingText} placeholder="长期纯维护、高频汇报"></textarea></label><label><span class="label">不可妥协的条件</span><textarea class="textarea" bind:value={constraintsText} placeholder="不接受长期出差"></textarea></label><div class="flex justify-end"><button class="btn-primary" on:click={savePrefs}><Check size={15} />保存求职偏好</button></div></div></div>
          {:else}
            <div class="animate-lift"><div class="mb-5 flex items-start justify-between"><div><h3 class="section-title">AI 可以使用的事实</h3><p class="mt-1 text-xs body-muted">专岗简历只能引用已确认事实，不能补写未经证实的经历。</p></div><span class="chip-brand"><CheckCircle2 size={13} />{draft.facts.filter((fact) => fact.confirmed).length} 已确认</span></div><div class="space-y-3">{#each draft.facts as fact}<article class="rounded-xl border p-4" style="border-color: var(--line);"><div class="flex items-start gap-3"><span class="mt-0.5 grid h-7 w-7 place-items-center rounded-lg" style={`background:${fact.confidence >= .9 ? 'var(--brand-soft)' : 'var(--warning-soft)'};color:${fact.confidence >= .9 ? 'var(--brand)' : 'var(--warning)'}`}>{#if fact.confirmed}<Check size={14} />{:else}<AlertCircle size={14} />{/if}</span><div class="min-w-0 flex-1"><p class="text-sm font-medium leading-6">{fact.value}</p><div class="mt-2 flex items-center gap-3 text-[11px] body-muted"><span>来源：{fact.source}</span><span>置信度 {Math.round(fact.confidence * 100)}%</span></div></div><button class="btn-ghost h-8 text-xs"><PencilLine size={13} />编辑</button></div></article>{/each}</div></div>
          {/if}
        </div>
      </section>

      <section class="panel flex min-h-0 flex-col overflow-hidden">
        <div class="flex h-[53px] shrink-0 items-center justify-between border-b px-5" style="border-color: var(--line);"><div class="flex items-center gap-2"><FileText size={16} class="text-brand" /><span class="text-sm font-semibold">RenderCV 预览</span></div><div class="flex items-center gap-3 text-[11px] body-muted"><span>自动保存关闭</span><button class="btn-ghost h-8"><History size={14} />版本 {draft.version}</button></div></div>
        <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6" style="background: var(--panel-soft);">
          <article class="resume-paper mx-auto min-h-[900px] max-w-[620px] bg-white px-12 py-11 text-[#17201d] shadow-xl">
            <header class="border-b-2 border-[#176b57] pb-5"><h1 class="text-[32px] font-bold tracking-[-0.04em] text-[#176b57]">{draft.name}</h1><p class="mt-1 text-[15px] font-semibold">{draft.headline}</p><div class="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[9px] text-[#5c6863]"><span class="flex items-center gap-1"><Mail size={10} />{draft.email}</span><span class="flex items-center gap-1"><Phone size={10} />{draft.phone}</span><span class="flex items-center gap-1"><MapPin size={10} />{draft.location}</span></div></header>
            <section class="resume-section"><h2>个人简介</h2><p>{draft.summary}</p></section>
            <section class="resume-section"><h2>核心技能</h2><div class="flex flex-wrap gap-x-3 gap-y-1">{#each draft.skills as skill}<span>• {skill}</span>{/each}</div></section>
            <section class="resume-section"><h2>工作经历</h2>{#each draft.experiences as experience}<div class="mb-4"><div class="flex items-baseline justify-between gap-3"><strong>{experience.position} · {experience.company}</strong><span>{experience.startDate}—{experience.endDate}</span></div><ul>{#each experience.highlights as highlight}<li>{highlight}</li>{/each}</ul></div>{/each}</section>
            <section class="resume-section"><h2>教育经历</h2>{#each draft.education as education}<div class="flex items-baseline justify-between gap-3"><strong>{education.institution} · {education.area}</strong><span>{education.startDate}—{education.endDate}</span></div><p>{education.degree}</p>{/each}</section>
          </article>
        </div>
      </section>
    </div>
  {/if}
</div>

{#if toast}<div class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift">{toast}</div>{/if}

<style>
  .tab { position: relative; padding: 0 0 13px; font-size: 13px; font-weight: 600; color: var(--muted); }
  .tab.active { color: var(--ink); }
  .tab.active::after { content: ''; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; background: var(--brand); }
  .resume-paper { font-family: "Source Sans 3", "PingFang SC", sans-serif; }
  .resume-section { margin-top: 20px; font-size: 10px; line-height: 1.55; }
  .resume-section h2 { margin-bottom: 8px; border-bottom: 1px solid #aab7b1; padding-bottom: 3px; color: #176b57; font-size: 13px; font-weight: 700; text-transform: uppercase; letter-spacing: .04em; }
  .resume-section ul { margin-top: 5px; list-style: disc; padding-left: 16px; }
  .resume-section li { margin-top: 2px; }
</style>
