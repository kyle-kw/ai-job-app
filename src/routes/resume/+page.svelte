<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { downloadDir, join } from '@tauri-apps/api/path';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { Check, ChevronDown, Download, FileText, History, Plus, Save, Sparkles, Upload, UserRound, WandSparkles } from 'lucide-svelte';
  import ResumeChatDialog from '$lib/components/ResumeChatDialog.svelte';
  import ResumeFactsEditor from '$lib/components/ResumeFactsEditor.svelte';
  import ResumePaper from '$lib/components/ResumePaper.svelte';
  import ResumeVersionDrawer from '$lib/components/ResumeVersionDrawer.svelte';
  import { backend } from '$lib/services/backend';
  import { RESUME_TEMPLATES, resumeTemplate, suggestedProfessionalSkillGroups } from '$lib/resume-templates';
  import { safeResumeFileName } from '$lib/resume-format';
  import { readResumeAsBase64 } from '$lib/resume-import';
  import { modalFocus } from '$lib/modal-focus';
  import type { ResumeTemplateDefinition } from '$lib/resume-templates';
  import { importResume, refresh, savePreferences, snapshot } from '$lib/stores/app';
  import type { JobPreferences, ResumeColorTheme, ResumeCommitResult, ResumeProfile, ResumeTemplateId } from '$lib/types';

  let activeSection: 'content' | 'preferences' | 'facts' = 'content';
  let draft: ResumeProfile | null = null;
  let baselineResume: ResumeProfile | null = null;
  let pendingExternalResume: ResumeProfile | null = null;
  let saving = false;
  let importing = false;
  let rendering = false;
  let templateCreating = '';
  let toast = '';
  let assistantOpen = false;
  let versionDrawerOpen = false;
  let initialChatJobId: string | null = null;
  let fileInput: HTMLInputElement;
  let targetRolesText = '';
  let citiesText = '';
  let energizingText = '';
  let drainingText = '';
  let constraintsText = '';
  let previewTemplate: ResumeTemplateDefinition | null = null;
  let exportDialogOpen = false;
  let exportColorTheme: ResumeColorTheme = 'navy';
  const exportColorThemes: ReadonlyArray<{ id: ResumeColorTheme; label: string; description: string; accent: string }> = [
    { id: 'navy', label: '经典蓝', description: '参考简历原始配色，专业清晰', accent: '#1F407A' },
    { id: 'pine', label: '松柏绿', description: '清晰自然，适合技术与产品岗位', accent: '#176B57' },
    { id: 'graphite', label: '石墨黑', description: '克制简洁，强调黑白打印效果', accent: '#24292F' }
  ];

  $: draftPreferences = draft ? {
    ...draft.preferences,
    targetRoles: split(targetRolesText),
    cities: split(citiesText),
    energizingTasks: split(energizingText),
    drainingTasks: split(drainingText),
    hardConstraints: split(constraintsText)
  } : null;
  $: effectiveDraft = draft && draftPreferences ? { ...draft, preferences: draftPreferences } : null;
  $: hasUnsavedChanges = Boolean(effectiveDraft && baselineResume && JSON.stringify(effectiveDraft) !== JSON.stringify(baselineResume));
  $: if ($snapshot.resume) {
    const incoming = $snapshot.resume;
    const isNewResume = !baselineResume || incoming.id !== baselineResume.id;
    const isNewVersion = Boolean(baselineResume && incoming.id === baselineResume.id && incoming.version > baselineResume.version);
    if (isNewResume || (isNewVersion && !hasUnsavedChanges)) adoptResume(incoming);
    else if (isNewVersion && hasUnsavedChanges) pendingExternalResume = structuredClone(incoming);
  }
  $: aiReady = Boolean($snapshot.readiness.ai && $snapshot.providers.some((provider) => provider.isDefault && provider.verified));
  $: previewSections = draft ? resumeTemplate(draft.templateId).sectionOrder : [];

  function syncPreferenceTexts(preferences: JobPreferences) {
    targetRolesText = preferences.targetRoles.join('、'); citiesText = preferences.cities.join('、');
    energizingText = preferences.energizingTasks.join('、'); drainingText = preferences.drainingTasks.join('、'); constraintsText = preferences.hardConstraints.join('、');
  }
  const split = (value: string) => value.split(/[、,，\n]/).map((item) => item.trim()).filter(Boolean);
  function showToast(message: string) { toast = message; window.setTimeout(() => toast === message && (toast = ''), 2400); }

  function adoptResume(resume: ResumeProfile) {
    draft = structuredClone(resume);
    baselineResume = structuredClone(resume);
    pendingExternalResume = null;
    syncPreferenceTexts(resume.preferences);
  }

  async function save(): Promise<boolean> {
    if (!effectiveDraft) return false;
    if (pendingExternalResume) {
      showToast('主简历已有新版本，请先载入最新版本再继续保存');
      return false;
    }
    saving = true;
    try {
      const saved = await backend.saveResume(structuredClone(effectiveDraft));
      adoptResume(saved);
      await refresh();
      showToast('主简历已保存');
      return true;
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
      return false;
    } finally { saving = false; }
  }

  async function savePrefs() {
    if (!draft) return;
    const preferences: JobPreferences = { ...draft.preferences, targetRoles: split(targetRolesText), cities: split(citiesText), energizingTasks: split(energizingText), drainingTasks: split(drainingText), hardConstraints: split(constraintsText) };
    try {
      await savePreferences(preferences);
      draft.preferences = preferences;
      if (baselineResume) baselineResume = { ...baselineResume, preferences: structuredClone(preferences) };
      showToast('求职偏好已保存，匹配置信度将更新');
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    }
  }

  async function pickResume(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0]; if (!file) return;
    importing = true;
    try {
      const contentBase64 = await readResumeAsBase64(file);
      await importResume({ fileName: file.name, contentBase64 });
      showToast('正在后台解析新简历');
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      importing = false;
      (event.currentTarget as HTMLInputElement).value = '';
    }
  }

  async function requestImport() {
    assistantOpen = false;
    if (hasUnsavedChanges) {
      const shouldSave = window.confirm('检测到未保存的简历修改。点击“确定”先保存；点击“取消”可选择放弃修改或终止导入。');
      if (shouldSave) {
        if (!await save()) return;
      } else {
        const shouldDiscard = window.confirm('放弃当前未保存修改并重新导入？点击“取消”返回继续编辑。');
        if (!shouldDiscard) return;
        if ($snapshot.resume) adoptResume($snapshot.resume);
      }
    }
    fileInput?.click();
  }

  function renderPdf() {
    if (!draft) return;
    exportColorTheme = 'navy';
    exportDialogOpen = true;
  }

  async function confirmPdfExport() {
    if (!draft) return;
    rendering = true;
    let resumeWasSaved = false;
    try {
      const fileName = safeResumeFileName(draft.name);
      const isTauri = Boolean(window.__TAURI_INTERNALS__);
      const defaultPath = isTauri ? await join(await downloadDir(), fileName) : fileName;
      exportDialogOpen = false;
      const outputPath = isTauri ? await saveDialog({
        title: '导出 PDF 简历',
        defaultPath,
        filters: [{ name: 'PDF 简历', extensions: ['pdf'] }]
      }) : defaultPath;
      if (!outputPath) return;
      if (hasUnsavedChanges) {
        if (!await save()) return;
        resumeWasSaved = true;
      }
      const result = await backend.renderResume({ outputPath, colorTheme: exportColorTheme });
      showToast(`已导出 ${result.fileName}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      showToast(resumeWasSaved ? `简历已保存，但 PDF 导出失败：${message}` : message);
    } finally { rendering = false; }
  }

  async function openAssistant(jobId: string | null = null) {
    if (saving) return;
    if (draft && hasUnsavedChanges) {
      const shouldSave = window.confirm('检测到未保存的简历修改。点击“确定”先保存；点击“取消”可选择放弃修改或继续编辑。');
      if (shouldSave) {
        if (!await save()) return;
      } else {
        const shouldDiscard = window.confirm('放弃当前未保存修改并打开 AI 对话？点击“取消”返回继续编辑。');
        if (!shouldDiscard) return;
        if ($snapshot.resume) adoptResume($snapshot.resume);
      }
    }
    initialChatJobId = jobId && $snapshot.jobs.some((job) => job.id === jobId) ? jobId : null;
    assistantOpen = true;
  }

  function openVersionHistory() {
    versionDrawerOpen = true;
  }

  async function handleResumeCommit(event: CustomEvent<ResumeCommitResult>, action: 'applied' | 'restored') {
    adoptResume(event.detail.resume);
    await refresh();
    showToast(action === 'applied' ? `AI 修改已应用，已创建版本 ${event.detail.resume.version}` : `已恢复并创建版本 ${event.detail.resume.version}`);
  }

  async function createFromTemplate(templateId: ResumeTemplateId) {
    templateCreating = templateId;
    try {
      const created = await backend.createResumeFromTemplate(templateId);
      adoptResume(created);
      await refresh();
      showToast(`已创建${resumeTemplate(templateId).label}主简历`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally {
      templateCreating = '';
    }
  }

  function openTemplatePreview(templateId: ResumeTemplateId) {
    const template = resumeTemplate(templateId);
    if (template.sample) previewTemplate = template;
  }

  async function usePreviewedTemplate() {
    if (!previewTemplate) return;
    const template = previewTemplate;
    previewTemplate = null;
    if (draft) {
      draft.templateId = template.id;
      showToast(`已切换为${template.label}结构，原有内容未改动`);
      return;
    }
    await createFromTemplate(template.id);
  }

  function addSkillGroup() {
    if (!draft) return;
    draft.professionalSkills = [...draft.professionalSkills, { id: crypto.randomUUID(), label: '新分组', items: [] }];
  }

  function addSkill(groupIndex: number) {
    if (!draft) return;
    draft.professionalSkills[groupIndex].items = [...draft.professionalSkills[groupIndex].items, ''];
  }

  function applySuggestedGroups() {
    if (!draft) return;
    const existing = new Set(draft.professionalSkills.map((group) => group.label));
    const additions = suggestedProfessionalSkillGroups(draft.templateId).filter((group) => !existing.has(group.label));
    draft.professionalSkills = [...draft.professionalSkills, ...additions];
  }

  function addProject() {
    if (!draft) return;
    draft.projects = [...draft.projects, { id: crypto.randomUUID(), name: '新项目', summary: '', startDate: '', endDate: '', highlights: [''] }];
  }

  function addCertification() {
    if (!draft) return;
    draft.certifications = [...draft.certifications, { id: crypto.randomUUID(), name: '新证书', issuer: '', date: '' }];
  }

  function addEducation() {
    if (!draft) return;
    draft.education = [...draft.education, { id: crypto.randomUUID(), institution: '', area: '', degree: '', degreeDetail: '', startDate: '', endDate: '', highlights: [] }];
  }

  onMount(() => {
    if ($page.url?.searchParams.get('assistant') === '1') {
      void openAssistant($page.url.searchParams.get('job'));
    }
  });
</script>

<div class="page-content h-[calc(100vh-74px)] min-h-[646px] max-w-none overflow-hidden pb-6">
  <div class="mb-5 flex items-end justify-between">
    <div><p class="eyebrow">MASTER RESUME</p><h2 class="page-title mt-1">一份可信的主简历</h2><p class="mt-1 text-sm body-muted">结构化内容是唯一真源；手工修改和 AI 修改都会保存为可恢复的本地版本。</p></div>
    <div class="flex flex-wrap justify-end gap-2">
      <input bind:this={fileInput} class="hidden" type="file" accept=".pdf,.docx,.yaml,.yml" on:change={pickResume} />
      <button class="btn" disabled={importing} on:click={requestImport}><Upload size={15} />{importing ? '正在解析…' : draft ? '重新导入' : '导入简历'}</button>
      <button class="btn" on:click={openVersionHistory}><History size={15} />版本历史</button>
      <button class="btn" on:click={() => openAssistant(null)}><Sparkles size={15} />AI 对话</button>
      <button class="btn" disabled={!draft || rendering} on:click={renderPdf}><Download size={15} />{rendering ? '正在渲染…' : '导出 PDF'}</button>
      <button class="btn-primary" disabled={!draft || saving || !hasUnsavedChanges || Boolean(pendingExternalResume)} on:click={save}><Save size={15} />{saving ? '正在保存…' : hasUnsavedChanges ? '保存修改' : '已保存'}</button>
    </div>
  </div>

  {#if pendingExternalResume}
    <div class="mb-4 flex items-center justify-between gap-4 rounded-xl border px-4 py-3" role="alert" style="border-color: var(--warning); background: var(--warning-soft);">
      <div><p class="text-sm font-semibold">检测到新的简历版本 {pendingExternalResume.version}</p><p class="mt-1 text-xs body-muted">当前本地修改尚未覆盖；载入新版本会放弃这些未保存修改。</p></div>
      <button class="btn shrink-0" on:click={() => pendingExternalResume && adoptResume(pendingExternalResume)}>载入最新版本</button>
    </div>
  {/if}

  {#if !draft}
    <div class="panel min-h-[520px] p-8">
      <div class="mx-auto max-w-4xl text-center"><span class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-brand-soft text-brand"><FileText size={27} /></span><h3 class="text-xl font-semibold">导入现有简历，或从可信空白模板开始</h3><p class="mt-2 text-sm leading-6 body-muted">模板只提供章节顺序与专业技能分组，不会预填或虚构个人经历。</p><button class="btn-primary mt-5" on:click={requestImport}><Upload size={16} />选择简历文件</button></div>
      <div class="mx-auto mt-8 grid max-w-5xl grid-cols-4 gap-4">
        {#each RESUME_TEMPLATES as template}
          <article class="flex min-h-[170px] flex-col rounded-2xl border p-5 text-left transition hover:-translate-y-0.5" style="border-color: var(--line); background: var(--panel-soft);">
            <p class="text-sm font-semibold">{template.label}</p><p class="mt-2 flex-1 text-xs leading-5 body-muted">{template.description}</p>
            <div class="mt-4 flex flex-wrap items-center gap-3">
              {#if template.sample}<button class="btn-ghost h-8 px-0 text-xs" on:click={() => openTemplatePreview(template.id)}>查看完整示例</button>{/if}
              <button class="text-xs font-semibold text-brand" disabled={Boolean(templateCreating)} on:click={() => createFromTemplate(template.id)}>{templateCreating === template.id ? '正在创建…' : '使用此模板'}</button>
            </div>
          </article>
        {/each}
      </div>
    </div>
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
              <div><div class="mb-3 flex items-center gap-2"><UserRound size={17} class="text-brand" /><h3 class="section-title">基本信息</h3></div><div class="grid grid-cols-2 gap-4"><label><span class="label">姓名</span><input class="input" bind:value={draft.name} /></label><label><span class="label">职业标题</span><input class="input" bind:value={draft.headline} /></label><label><span class="label">邮箱</span><input class="input" bind:value={draft.email} /></label><label><span class="label">电话</span><input class="input" bind:value={draft.phone} /></label><label><span class="label">所在城市</span><input class="input" bind:value={draft.location} /></label><label><span class="label">个人主页</span><input class="input" bind:value={draft.website} /></label><label class="col-span-2"><span class="label">简历结构模板</span><div class="flex items-center gap-2"><select class="select" bind:value={draft.templateId}>{#each RESUME_TEMPLATES as template}<option value={template.id}>{template.label} · {template.description}</option>{/each}</select>{#if resumeTemplate(draft.templateId).sample}<button class="btn shrink-0" type="button" on:click={() => openTemplatePreview(draft!.templateId)}>查看示例</button>{/if}</div><span class="mt-1 block text-[11px] body-muted">切换模板只调整章节顺序，不会改写已有内容。</span></label></div></div>
              <div class="divider"></div>
              <div><div class="mb-3 flex items-center justify-between"><div class="flex items-center gap-2"><Sparkles size={17} class="text-brand" /><h3 class="section-title">个人简介</h3></div><button class="btn-ghost h-8 text-xs text-brand" on:click={() => openAssistant(null)}><WandSparkles size={14} />AI 优化</button></div><textarea class="textarea min-h-[120px] leading-6" bind:value={draft.summary}></textarea><p class="mt-2 text-[11px] body-muted">建议保持 3–4 行；AI 只会提出待审核修改，不会直接覆盖。</p></div>
              <div class="divider"></div>
              <div><div class="mb-3 flex items-center justify-between"><div><h3 class="section-title">专业技能</h3><p class="mt-1 text-[11px] body-muted">按岗位相关能力分组；分组标题不是个人事实。</p></div><div class="flex gap-2"><button class="btn-ghost h-8 text-xs" on:click={applySuggestedGroups}>使用模板分组</button><button class="btn-ghost h-8 text-xs" on:click={addSkillGroup}><Plus size={14} />添加分组</button></div></div><div class="space-y-3">{#each draft.professionalSkills as group, groupIndex}<article class="rounded-xl border p-4" style="border-color: var(--line);"><div class="flex items-center gap-2"><input class="input h-9 font-semibold" bind:value={group.label} /><button class="btn-ghost h-8 text-xs" on:click={() => addSkill(groupIndex)}><Plus size={13} />技能</button><button class="btn-ghost h-8 text-xs" aria-label="删除专业技能分组" on:click={() => draft && (draft.professionalSkills = draft.professionalSkills.filter((_, index) => index !== groupIndex))}>×</button></div><div class="mt-3 flex flex-wrap gap-2">{#each group.items as skill, skillIndex}<label class="chip-brand group cursor-text"><input class="w-[96px] bg-transparent outline-none" bind:value={group.items[skillIndex]} /><button class="ml-1 opacity-0 transition group-hover:opacity-100" on:click={() => group.items = group.items.filter((_, index) => index !== skillIndex)}>×</button></label>{/each}</div></article>{/each}</div></div>
              <div class="divider"></div>
              <div><div class="mb-4 flex items-center justify-between"><h3 class="section-title">工作经历</h3><button class="btn-ghost h-8 text-xs"><Plus size={14} />添加经历</button></div><div class="space-y-4">{#each draft.experiences as experience}<article class="rounded-xl border p-4" style="border-color: var(--line);"><div class="grid grid-cols-2 gap-3"><input class="input font-semibold" bind:value={experience.company} /><input class="input" bind:value={experience.position} /></div><div class="mt-3 space-y-2">{#each experience.highlights as highlight, index}<div class="flex gap-2"><span class="mt-3 h-1.5 w-1.5 shrink-0 rounded-full bg-brand"></span><textarea class="textarea min-h-[66px]" bind:value={experience.highlights[index]}></textarea></div>{/each}</div></article>{/each}</div></div>
              <div class="divider"></div>
              <div><div class="mb-4 flex items-center justify-between"><h3 class="section-title">项目经历</h3><button class="btn-ghost h-8 text-xs" on:click={addProject}><Plus size={14} />添加项目</button></div><div class="space-y-4">{#each draft.projects as project, projectIndex}<article class="rounded-xl border p-4" style="border-color: var(--line);"><div class="flex gap-3"><input class="input font-semibold" bind:value={project.name} placeholder="项目名称" /><button class="btn-ghost h-9" aria-label="删除项目" on:click={() => draft && (draft.projects = draft.projects.filter((_, index) => index !== projectIndex))}>×</button></div><textarea class="textarea mt-3" bind:value={project.summary} placeholder="项目简介"></textarea><div class="mt-3 grid grid-cols-2 gap-3"><input class="input" bind:value={project.startDate} placeholder="开始时间" /><input class="input" bind:value={project.endDate} placeholder="结束时间" /></div><div class="mt-3 space-y-2">{#each project.highlights as highlight, index}<textarea class="textarea min-h-[60px]" bind:value={project.highlights[index]} placeholder="项目成果"></textarea>{/each}</div></article>{/each}</div></div>
              <div class="divider"></div>
              <div><div class="mb-4 flex items-center justify-between"><h3 class="section-title">证书 / 专业资质</h3><button class="btn-ghost h-8 text-xs" on:click={addCertification}><Plus size={14} />添加证书</button></div><div class="space-y-3">{#each draft.certifications as certification, certificationIndex}<article class="grid grid-cols-[1fr_1fr_140px_auto] gap-3 rounded-xl border p-4" style="border-color: var(--line);"><input class="input font-semibold" bind:value={certification.name} placeholder="证书名称" /><input class="input" bind:value={certification.issuer} placeholder="颁发机构" /><input class="input" bind:value={certification.date} placeholder="取得日期" /><button class="btn-ghost h-10" aria-label="删除证书" on:click={() => draft && (draft.certifications = draft.certifications.filter((_, index) => index !== certificationIndex))}>×</button></article>{/each}</div></div>
              <div class="divider"></div>
              <div><div class="mb-4 flex items-center justify-between"><h3 class="section-title">教育经历</h3><button class="btn-ghost h-8 text-xs" on:click={addEducation}><Plus size={14} />添加教育经历</button></div><div class="space-y-4">{#each draft.education as education, educationIndex (education.id)}<article class="rounded-xl border p-4" style="border-color: var(--line);"><div class="grid grid-cols-[1fr_1fr_auto] gap-3"><input class="input font-semibold" bind:value={education.institution} placeholder="学校" /><input class="input" bind:value={education.area} placeholder="专业" /><button class="btn-ghost h-10" aria-label={`删除教育经历：${education.institution || educationIndex + 1}`} on:click={() => draft && (draft.education = draft.education.filter((_, index) => index !== educationIndex))}>×</button></div><div class="mt-3 grid grid-cols-2 gap-3"><label><span class="label">学历</span><select class="select" bind:value={education.degree}><option value="">请选择</option><option value="本科">本科</option><option value="硕士">硕士</option><option value="博士">博士</option><option value="其他">其他</option></select></label>{#if education.degree === '其他'}<label><span class="label">学历原文</span><input class="input" bind:value={education.degreeDetail} placeholder="例如：大专、Bachelor of Science" /></label>{:else}<div class="grid grid-cols-2 gap-2"><label><span class="label">开始时间</span><input class="input" bind:value={education.startDate} placeholder="2019.09" /></label><label><span class="label">结束时间</span><input class="input" bind:value={education.endDate} placeholder="2023.06" /></label></div>{/if}</div>{#if education.degree === '其他'}<div class="mt-3 grid grid-cols-2 gap-3"><input class="input" bind:value={education.startDate} placeholder="开始时间" /><input class="input" bind:value={education.endDate} placeholder="结束时间" /></div>{/if}</article>{/each}</div></div>
            </div>
          {:else if activeSection === 'preferences'}
            <div class="animate-lift"><div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--brand-faint);"><div class="flex gap-3"><Sparkles size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">约 60 秒，提高匹配结果的可信度</p><p class="mt-1 text-xs leading-5 body-muted">不知道的维度不会被 AI 硬猜。未填写项会降低置信度，而不会直接扣分。</p></div></div></div><div class="mt-6 space-y-5"><label><span class="label">目标岗位</span><input class="input" bind:value={targetRolesText} placeholder="AI Agent、大模型应用" /><span class="mt-1 block text-[11px] body-muted">使用顿号分隔多个方向</span></label><label><span class="label">目标城市</span><input class="input" bind:value={citiesText} placeholder="上海、杭州" /></label><label><span class="label">工作方式</span><select class="select" bind:value={draft.preferences.remotePreference}><option value="onsite">到岗办公</option><option value="hybrid">混合办公</option><option value="remote">远程办公</option><option value="flexible">均可</option></select></label><label><span class="label">让你有动力的任务</span><textarea class="textarea" bind:value={energizingText} placeholder="从 0 到 1、系统设计、Agent 工作流"></textarea></label><label><span class="label">希望减少的任务</span><textarea class="textarea" bind:value={drainingText} placeholder="长期纯维护、高频汇报"></textarea></label><label><span class="label">不可妥协的条件</span><textarea class="textarea" bind:value={constraintsText} placeholder="不接受长期出差"></textarea></label><div class="flex justify-end"><button class="btn-primary" on:click={savePrefs}><Check size={15} />保存求职偏好</button></div></div></div>
          {:else}
            <ResumeFactsEditor
              resume={draft}
              {saving}
              {hasUnsavedChanges}
              on:factschange={(event) => draft && (draft.facts = event.detail.facts)}
              on:save={save}
              on:notice={(event) => showToast(event.detail.message)}
            />
          {/if}
        </div>
      </section>

      <section class="panel flex min-h-0 flex-col overflow-hidden">
        <div class="flex h-[53px] shrink-0 items-center justify-between border-b px-5" style="border-color: var(--line);"><div class="flex items-center gap-2"><FileText size={16} class="text-brand" /><span class="text-sm font-semibold">RenderCV 预览</span></div><div class="flex items-center gap-3 text-[11px] body-muted"><span>{hasUnsavedChanges ? '有未保存修改' : '内容已保存'}</span><button class="btn-ghost h-8" on:click={openVersionHistory}><History size={14} />版本 {draft.version}</button></div></div>
        <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6" style="background: var(--panel-soft);">
          <ResumePaper resume={draft} sections={previewSections} />
        </div>
      </section>
    </div>
  {/if}
</div>

{#if exportDialogOpen}
  <button class="fixed inset-0 z-[75] bg-black/35 backdrop-blur-sm" tabindex="-1" on:click={() => exportDialogOpen = false} aria-label="关闭导出颜色选择"></button>
  <div class="fixed left-1/2 top-1/2 z-[76] w-[760px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 panel p-6" role="dialog" aria-modal="true" aria-labelledby="export-color-title" tabindex="-1" use:modalFocus={{ close: () => exportDialogOpen = false, canClose: !rendering }}>
    <div class="flex items-start justify-between gap-4"><div><p class="eyebrow">PDF 导出</p><h3 id="export-color-title" class="mt-1 text-xl font-semibold">选择颜色主题</h3><p class="mt-1 text-xs body-muted">PDF 将沿用右侧预览版式；颜色只影响本次导出，不会修改主简历或章节顺序。</p></div><button class="btn-icon" on:click={() => exportDialogOpen = false} aria-label="关闭">×</button></div>
    <div class="mt-6 grid grid-cols-3 gap-4">
      {#each exportColorThemes as theme}
        <button type="button" aria-pressed={exportColorTheme === theme.id} class:selected-export={exportColorTheme === theme.id} class="export-theme-card rounded-2xl border p-4 text-left transition" style={`--swatch-accent: ${theme.accent};`} on:click={() => exportColorTheme = theme.id}>
          <span class="export-swatch"><span></span><span></span><span></span></span>
          <span class="mt-4 flex items-center gap-2 text-sm font-semibold"><span class="h-3 w-3 rounded-full" style={`background: ${theme.accent};`}></span>{theme.label}</span>
          <span class="mt-1 block text-xs leading-5 body-muted">{theme.description}</span>
        </button>
      {/each}
    </div>
    <div class="mt-6 flex justify-end gap-2"><button class="btn" on:click={() => exportDialogOpen = false}>取消</button><button class="btn-primary" disabled={rendering} on:click={confirmPdfExport}><Download size={15} />选择保存位置</button></div>
  </div>
{/if}

<ResumeChatDialog
  bind:open={assistantOpen}
  resume={draft}
  jobs={$snapshot.jobs}
  {aiReady}
  initialJobId={initialChatJobId}
  on:applied={(event) => handleResumeCommit(event, 'applied')}
  on:requestimport={requestImport}
/>
<ResumeVersionDrawer
  bind:open={versionDrawerOpen}
  resume={draft}
  {hasUnsavedChanges}
  on:restored={(event) => handleResumeCommit(event, 'restored')}
/>

{#if previewTemplate?.sample}
  <button class="fixed inset-0 z-[70] bg-black/35 backdrop-blur-sm" tabindex="-1" on:click={() => previewTemplate = null} aria-label="关闭模板示例"></button>
  <div class="fixed inset-y-5 left-1/2 z-[71] flex w-[760px] max-w-[calc(100vw-32px)] -translate-x-1/2 flex-col overflow-hidden panel" role="dialog" aria-modal="true" aria-labelledby="template-preview-title" tabindex="-1" use:modalFocus={{ close: () => previewTemplate = null }}>
    <div class="flex shrink-0 items-start justify-between gap-4 border-b px-6 py-5" style="border-color: var(--line);">
      <div><p class="eyebrow">完整示例</p><h3 id="template-preview-title" class="mt-1 text-xl font-semibold">{previewTemplate.label}简历</h3><p class="mt-1 text-xs body-muted">3–5 年社招候选人写法参考</p></div>
      <button class="btn-icon" on:click={() => previewTemplate = null} aria-label="关闭">×</button>
    </div>
    <div class="mx-6 mt-5 shrink-0 rounded-xl border px-4 py-3 text-sm font-semibold text-warning" style="border-color: color-mix(in srgb, var(--warning) 35%, var(--line)); background: var(--warning-soft);">示例内容，请勿直接用于投递。示例不会写入主简历或事实库。</div>
    <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6" style="background: var(--panel-soft);">
      <ResumePaper resume={previewTemplate.sample} sections={previewTemplate.sectionOrder} sample />
    </div>
    <div class="flex shrink-0 items-center justify-between gap-4 border-t px-6 py-4" style="border-color: var(--line);">
      <p class="text-xs body-muted">{draft ? '只切换章节结构，保留当前全部内容。' : '创建时只使用空白结构和技能分组。'}</p>
      <button class="btn-primary shrink-0" on:click={usePreviewedTemplate}>{draft ? '切换至此结构' : '使用此模板'}</button>
    </div>
  </div>
{/if}

{#if toast}<div class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift">{toast}</div>{/if}

<style>
  .tab { position: relative; padding: 0 0 13px; font-size: 13px; font-weight: 600; color: var(--muted); }
  .tab.active { color: var(--ink); }
  .tab.active::after { content: ''; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; background: var(--brand); }
  .export-theme-card { border-color: var(--line); background: var(--panel-soft); }
  .export-theme-card:hover, .export-theme-card.selected-export { border-color: var(--swatch-accent); transform: translateY(-2px); }
  .export-theme-card.selected-export { box-shadow: 0 0 0 2px color-mix(in srgb, var(--swatch-accent) 24%, transparent); }
  .export-swatch { display: flex; height: 104px; flex-direction: column; gap: 9px; border-radius: 10px; background: white; padding: 18px 16px; box-shadow: inset 0 0 0 1px rgba(0,0,0,.08); }
  .export-swatch span { display: block; height: 5px; border-radius: 999px; background: #cbd3cf; }
  .export-swatch span:first-child { width: 58%; height: 9px; background: var(--swatch-accent); }
  .export-swatch span:nth-child(2) { background: var(--swatch-accent); opacity: .72; }
</style>
