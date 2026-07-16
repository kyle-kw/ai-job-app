<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { page } from '$app/stores';
  import { downloadDir, join } from '@tauri-apps/api/path';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { ArrowDown, ArrowUp, BriefcaseBusiness, Check, ChevronDown, Download, FileText, GripVertical, History, Plus, RefreshCw, Save, ShieldCheck, Sparkles, Trash2, Upload, UserRound, WandSparkles } from 'lucide-svelte';
  import ResumeChatDialog from '$lib/components/ResumeChatDialog.svelte';
  import ResumeCoveragePanel from '$lib/components/ResumeCoveragePanel.svelte';
  import ResumeDeleteDialog from '$lib/components/ResumeDeleteDialog.svelte';
  import ResumeFactsEditor from '$lib/components/ResumeFactsEditor.svelte';
  import ResumeHealthDrawer from '$lib/components/ResumeHealthDrawer.svelte';
  import ResumePaper from '$lib/components/ResumePaper.svelte';
  import ResumeRebaseDialog from '$lib/components/ResumeRebaseDialog.svelte';
  import ResumeVersionDrawer from '$lib/components/ResumeVersionDrawer.svelte';
  import { backend } from '$lib/services/backend';
  import { RESUME_TEMPLATES, resumeTemplate, suggestedProfessionalSkillGroups } from '$lib/resume-templates';
  import { safeResumeFileName } from '$lib/resume-format';
  import { readResumeAsBase64 } from '$lib/resume-import';
  import { modalFocus } from '$lib/modal-focus';
  import { moveItem, removeAt } from '$lib/list-order';
  import { analyzeResumeHealth } from '$lib/resume-health';
  import { buildLocalResumeCoverage, coverageHighlightKeywords } from '$lib/resume-coverage';
  import type { ResumeTemplateDefinition } from '$lib/resume-templates';
  import { importResume, refresh, savePreferences, snapshot } from '$lib/stores/app';
  import type { Job, JobOption, JobPreferences, MarketResumeContextRequest, ResumeColorTheme, ResumeCommitResult, ResumeCoverageItem, ResumeCoverageReport, ResumeHealthIssue, ResumeProfile, ResumeRebasePreview, ResumeRebaseResolution, ResumeTemplateId, ResumeVariantCommitResult, ResumeVariantDetail, ResumeVariantSummary } from '$lib/types';

  let resumeMode: 'master' | 'variant' = 'master';
  let activeSection: 'content' | 'preferences' | 'facts' | 'coverage' = 'content';
  let draft: ResumeProfile | null = null;
  let baselineResume: ResumeProfile | null = null;
  let pendingExternalResume: ResumeProfile | null = null;
  let variants: ResumeVariantSummary[] = [];
  let activeVariant: ResumeVariantDetail | null = null;
  let variantsLoading = false;
  let creatingVariant = false;
  let jobOptions: JobOption[] = [];
  let jobQuery = '';
  let selectedVariantJobId = '';
  let activeVariantJob: Job | null = null;
  let requestedVariantJobId = '';
  let aiCoverageReport: ResumeCoverageReport | null = null;
  let coverageAnalyzing = false;
  let rebasePreview: ResumeRebasePreview | null = null;
  let rebaseLoading = false;
  let rebaseDialogOpen = false;
  let saving = false;
  let importing = false;
  let rendering = false;
  let templateCreating = '';
  let toast = '';
  let assistantOpen = false;
  let versionDrawerOpen = false;
  let healthDrawerOpen = false;
  let healthAssistantPrompt = '';
  let initialChatJobId: string | null = null;
  let initialChatMarketContext: MarketResumeContextRequest | null = null;
  let fileInput: HTMLInputElement;
  let targetRolesText = '';
  let citiesText = '';
  let energizingText = '';
  let drainingText = '';
  let constraintsText = '';
  let previewTemplate: ResumeTemplateDefinition | null = null;
  let exportDialogOpen = false;
  let pendingRemoval: { label: string; apply: () => void } | null = null;
  let dragSource: { kind: string; index: number; parent?: number } | null = null;
  let exportColorTheme: ResumeColorTheme = 'navy';
  const exportColorThemes: ReadonlyArray<{ id: ResumeColorTheme; label: string; description: string; accent: string }> = [
    { id: 'navy', label: '经典蓝', description: '参考简历原始配色，专业清晰', accent: '#1F407A' },
    { id: 'pine', label: '松柏绿', description: '清晰自然，适合技术与产品岗位', accent: '#176B57' },
    { id: 'graphite', label: '石墨黑', description: '克制简洁，强调黑白打印效果', accent: '#24292F' }
  ];

  const emptyHealthResume: ResumeProfile = {
    id: '', name: '', headline: '', email: '', phone: '', location: '', website: '', summary: '', templateId: 'general',
    professionalSkills: [], experiences: [], education: [], projects: [], certifications: [], facts: [],
    preferences: { targetRoles: [], cities: [], remotePreference: 'flexible', energizingTasks: [], drainingTasks: [], hardConstraints: [] },
    sourceFileName: '', updatedAt: '', version: 0
  };

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
  $: if ($snapshot.resume && resumeMode === 'master') {
    const incoming = $snapshot.resume;
    const isNewResume = !baselineResume || incoming.id !== baselineResume.id;
    const isNewVersion = Boolean(baselineResume && incoming.id === baselineResume.id && incoming.version > baselineResume.version);
    if (isNewResume || (isNewVersion && !hasUnsavedChanges)) adoptResume(incoming);
    else if (isNewVersion && hasUnsavedChanges) pendingExternalResume = structuredClone(incoming);
  }
  $: aiReady = Boolean($snapshot.readiness.ai && $snapshot.providers.some((provider) => provider.isDefault && provider.verified));
  $: previewSections = draft ? resumeTemplate(draft.templateId).sectionOrder : [];
  $: activeTargetLabel = resumeMode === 'master' ? '主简历' : activeVariant?.name ?? '岗位版本';
  $: if (resumeMode === 'variant' && activeVariant?.jobId && activeVariant.jobId !== requestedVariantJobId) {
    requestedVariantJobId = activeVariant.jobId;
    void loadActiveVariantJob(activeVariant.jobId);
  }
  $: localCoverageReport = activeVariantJob && effectiveDraft && activeVariant
    ? buildLocalResumeCoverage(activeVariantJob, { kind: 'variant', id: activeVariant.id }, effectiveDraft)
    : null;
  $: coverageReport = aiCoverageReport && !hasUnsavedChanges && activeVariant
    && aiCoverageReport.target.id === activeVariant.id && aiCoverageReport.targetVersion === activeVariant.version
      ? aiCoverageReport : localCoverageReport;
  $: previewCoverageKeywords = resumeMode === 'variant' ? coverageHighlightKeywords(coverageReport) : [];
  $: healthReport = analyzeResumeHealth(effectiveDraft ?? draft ?? emptyHealthResume);

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

  function adoptVariant(variant: ResumeVariantDetail) {
    activeVariant = structuredClone(variant);
    draft = structuredClone(variant.profile);
    baselineResume = structuredClone(variant.profile);
    pendingExternalResume = null;
    syncPreferenceTexts(variant.profile.preferences);
  }

  async function loadVariants(preferredId?: string) {
    variantsLoading = true;
    try {
      variants = await backend.listResumeVariants();
      const id = preferredId ?? activeVariant?.id ?? variants[0]?.id;
      if (id) adoptVariant(await backend.getResumeVariant(id));
      else { activeVariant = null; draft = null; baselineResume = null; }
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
    } finally { variantsLoading = false; }
  }

  async function loadVariantJobs() {
    try { jobOptions = await backend.listJobOptions(jobQuery); }
    catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
  }

  async function loadActiveVariantJob(jobId: string) {
    activeVariantJob = null;
    aiCoverageReport = null;
    try { activeVariantJob = await backend.getJob(jobId); }
    catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
  }

  async function switchResumeMode(mode: 'master' | 'variant') {
    if (mode === resumeMode) return;
    if (hasUnsavedChanges) {
      const shouldSave = window.confirm(`当前${activeTargetLabel}有未保存修改。点击“确定”先保存，点击“取消”可选择放弃。`);
      if (shouldSave) { if (!await save()) return; }
      else if (!window.confirm('放弃当前未保存修改并切换？')) return;
    }
    resumeMode = mode;
    activeSection = 'content';
    healthAssistantPrompt = '';
    if (mode === 'master') {
      activeVariant = null;
      if ($snapshot.resume) adoptResume($snapshot.resume); else { draft = null; baselineResume = null; }
    } else {
      await Promise.all([loadVariants(), loadVariantJobs()]);
    }
  }

  async function selectVariant(id: string) {
    if (!id || id === activeVariant?.id) return;
    if (hasUnsavedChanges && !window.confirm('放弃当前岗位版本的未保存修改并切换？')) return;
    try { adoptVariant(await backend.getResumeVariant(id)); activeSection = 'content'; }
    catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
  }

  async function startVariantCreation() {
    if (hasUnsavedChanges && !window.confirm('放弃当前岗位版本的未保存修改并创建其他岗位版本？')) return;
    activeVariant = null;
    draft = null;
    baselineResume = null;
    selectedVariantJobId = '';
    jobQuery = '';
    activeSection = 'content';
    await loadVariantJobs();
  }

  async function createVariant() {
    if (!$snapshot.resume || !selectedVariantJobId) return;
    creatingVariant = true;
    try {
      const created = await backend.createResumeVariant(selectedVariantJobId, $snapshot.resume.version);
      adoptVariant(created);
      variants = await backend.listResumeVariants();
      showToast(`已创建岗位版本：${created.name}`);
    } catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
    finally { creatingVariant = false; }
  }

  async function save(): Promise<boolean> {
    if (!effectiveDraft) return false;
    if (resumeMode === 'master' && pendingExternalResume) {
      showToast('主简历已有新版本，请先载入最新版本再继续保存');
      return false;
    }
    saving = true;
    try {
      if (resumeMode === 'variant') {
        if (!activeVariant) return false;
        const result = await backend.saveResumeVariant(activeVariant.id, structuredClone(effectiveDraft), activeVariant.version);
        adoptVariant(result.variant);
        variants = await backend.listResumeVariants();
        showToast(`岗位版本已保存为 v${result.variant.version}`);
      } else {
        const saved = await backend.saveResume(structuredClone(effectiveDraft));
        adoptResume(saved);
        await refresh();
        showToast('主简历已保存');
      }
      return true;
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error));
      return false;
    } finally { saving = false; }
  }

  async function openRebase() {
    if (!activeVariant || rebaseLoading) return;
    if (!$snapshot.resume) { showToast('主简历暂时不可用，当前岗位版本仍可查看和导出，但不能同步。'); return; }
    if (hasUnsavedChanges && !await save()) return;
    rebaseLoading = true;
    rebasePreview = null;
    rebaseDialogOpen = true;
    try { rebasePreview = await backend.previewResumeVariantRebase(activeVariant.id); }
    catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
    finally { rebaseLoading = false; }
  }

  async function applyRebase(resolutions: ResumeRebaseResolution[]) {
    if (!activeVariant || !rebasePreview) return;
    rebaseLoading = true;
    try {
      const result = await backend.applyResumeVariantRebase(
        activeVariant.id, rebasePreview.variantVersion, rebasePreview.masterVersion, resolutions
      );
      adoptVariant(result.variant);
      variants = await backend.listResumeVariants();
      rebasePreview = null;
      rebaseDialogOpen = false;
      showToast(`已同步主简历 v${result.variant.baseResumeVersion}`);
    } catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
    finally { rebaseLoading = false; }
  }

  async function deleteActiveVariant() {
    if (!activeVariant || !window.confirm(`删除岗位版本“${activeVariant.name}”及其历史记录？此操作不可恢复。`)) return;
    try {
      await backend.deleteResumeVariant(activeVariant.id);
      activeVariant = null;
      await loadVariants();
      showToast('岗位版本已删除');
    } catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
  }

  async function analyzeCoverage() {
    if (!activeVariant || coverageAnalyzing) return;
    if (hasUnsavedChanges && !await save()) return;
    coverageAnalyzing = true;
    try {
      aiCoverageReport = await backend.analyzeResumeCoverage({ kind: 'variant', id: activeVariant.id });
      showToast('岗位语义覆盖分析已更新');
    } catch (error) { showToast(error instanceof Error ? error.message : String(error)); }
    finally { coverageAnalyzing = false; }
  }

  function focusCoverageItem(item: ResumeCoverageItem) {
    const path = item.resumePaths[0];
    if (!path) return;
    void focusHealthIssue({ id: item.id, code: 'coverage', severity: 'suggestion', path, label: item.label, message: item.rationale });
  }

  function optimizeCoverageItem(item: ResumeCoverageItem) {
    const prompt = `请针对岗位要求“${item.label}”强化当前岗位版本。只能使用以下已确认事实证据 ID：${item.evidenceFactIds.join('、')}。请提出待审核修改，不要新增事实。`;
    void openAssistant(activeVariant?.jobId ?? null, prompt);
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
    if (healthReport.errorCount > 0 && !window.confirm(`简历体检仍有 ${healthReport.errorCount} 个严重问题，仍要继续导出吗？`)) return;
    rendering = true;
    let resumeWasSaved = false;
    try {
      const fileName = safeResumeFileName(resumeMode === 'variant' && activeVariant
        ? [draft.name, activeVariant.company, activeVariant.jobTitle].filter(Boolean).join('-')
        : draft.name);
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
      const result = await backend.renderResume({
        outputPath,
        colorTheme: exportColorTheme,
        target: { kind: resumeMode, id: resumeMode === 'variant' ? activeVariant!.id : draft.id }
      });
      showToast(`已导出 ${result.fileName}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      showToast(resumeWasSaved ? `简历已保存，但 PDF 导出失败：${message}` : message);
    } finally { rendering = false; }
  }

  async function openAssistant(jobId: string | null = null, initialPrompt = '', marketContext: MarketResumeContextRequest | null = null) {
    if (saving) return;
    if (resumeMode === 'variant' && !$snapshot.resume) { showToast('主简历暂时不可用，当前岗位版本不能调用 AI。'); return; }
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
    initialChatJobId = resumeMode === 'variant' ? activeVariant?.jobId ?? null : jobId;
    initialChatMarketContext = resumeMode === 'master' ? marketContext : null;
    healthAssistantPrompt = initialPrompt;
    assistantOpen = true;
  }

  function openVersionHistory() {
    versionDrawerOpen = true;
  }

  async function handleResumeCommit(event: CustomEvent<ResumeCommitResult | ResumeVariantCommitResult>, action: 'applied' | 'restored') {
    if ('variant' in event.detail) {
      adoptVariant(event.detail.variant);
      variants = await backend.listResumeVariants();
      showToast(action === 'applied' ? `AI 修改已应用，已创建岗位版本 ${event.detail.variant.version}` : `已恢复并创建岗位版本 ${event.detail.variant.version}`);
    } else {
      adoptResume(event.detail.resume);
      await refresh();
      showToast(action === 'applied' ? `AI 修改已应用，已创建版本 ${event.detail.resume.version}` : `已恢复并创建版本 ${event.detail.resume.version}`);
    }
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
    draft.professionalSkills = draft.professionalSkills.map((group, index) => index === groupIndex
      ? { ...group, items: [...group.items, ''] }
      : group);
  }

  function applySuggestedGroups() {
    if (!draft) return;
    const existing = new Set(draft.professionalSkills.map((group) => group.label));
    const additions = suggestedProfessionalSkillGroups(draft.templateId).filter((group) => !existing.has(group.label));
    draft.professionalSkills = [...draft.professionalSkills, ...additions];
  }

  function addExperience() {
    if (!draft) return;
    draft.experiences = [...draft.experiences, {
      id: crypto.randomUUID(),
      company: '',
      position: '',
      location: '',
      startDate: '',
      endDate: '',
      highlights: ['']
    }];
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

  function addHighlight(section: 'experiences' | 'projects' | 'education', itemIndex: number) {
    updateHighlights(section, itemIndex, (highlights) => [...highlights, '']);
  }

  function removeHighlight(section: 'experiences' | 'projects' | 'education', itemIndex: number, highlightIndex: number) {
    if (!draft) return;
    const item = draft[section][itemIndex];
    const value = item?.highlights[highlightIndex];
    if (value === undefined) return;
    requestItemRemoval(`成果“${value || highlightIndex + 1}”`, value, () => {
      updateHighlights(section, itemIndex, (highlights) => removeAt(highlights, highlightIndex));
    });
  }

  function moveHighlight(section: 'experiences' | 'projects' | 'education', itemIndex: number, from: number, to: number) {
    updateHighlights(section, itemIndex, (highlights) => moveItem(highlights, from, to));
  }

  function updateHighlights(
    section: 'experiences' | 'projects' | 'education',
    itemIndex: number,
    update: (highlights: string[]) => string[]
  ) {
    if (!draft) return;
    if (section === 'experiences') {
      draft.experiences = draft.experiences.map((item, index) => index === itemIndex
        ? { ...item, highlights: update(item.highlights) }
        : item);
    } else if (section === 'projects') {
      draft.projects = draft.projects.map((item, index) => index === itemIndex
        ? { ...item, highlights: update(item.highlights) }
        : item);
    } else {
      draft.education = draft.education.map((item, index) => index === itemIndex
        ? { ...item, highlights: update(item.highlights) }
        : item);
    }
  }

  function moveResumeItem(section: 'professionalSkills' | 'experiences' | 'projects' | 'certifications' | 'education', from: number, to: number) {
    if (!draft) return;
    if (section === 'professionalSkills') draft.professionalSkills = moveItem(draft.professionalSkills, from, to);
    else if (section === 'experiences') draft.experiences = moveItem(draft.experiences, from, to);
    else if (section === 'projects') draft.projects = moveItem(draft.projects, from, to);
    else if (section === 'certifications') draft.certifications = moveItem(draft.certifications, from, to);
    else draft.education = moveItem(draft.education, from, to);
  }

  function clearDragSource() {
    dragSource = null;
  }

  function beginDrag(event: DragEvent, kind: string, index: number, parent?: number) {
    dragSource = { kind, index, parent };
    event.dataTransfer?.setData('text/plain', `${kind}:${parent ?? ''}:${index}`);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  }

  function dropItem(event: DragEvent, kind: string, index: number, parent?: number) {
    event.preventDefault();
    if (!dragSource || dragSource.kind !== kind || dragSource.parent !== parent) return;
    if (kind.startsWith('highlight:') && parent !== undefined) {
      const section = kind.slice('highlight:'.length) as 'experiences' | 'projects' | 'education';
      moveHighlight(section, parent, dragSource.index, index);
    } else {
      moveResumeItem(kind as 'professionalSkills' | 'experiences' | 'projects' | 'certifications' | 'education', dragSource.index, index);
    }
    dragSource = null;
  }

  async function focusHealthIssue(issue: ResumeHealthIssue) {
    healthDrawerOpen = false;
    activeSection = 'content';
    await tick();
    const candidates = [issue.path];
    let current = issue.path;
    while (current.includes('/')) {
      current = current.slice(0, current.lastIndexOf('/'));
      if (current) candidates.push(current);
    }
    for (const path of candidates) {
      const target = document.querySelector<HTMLElement>(`[data-resume-path="${CSS.escape(path)}"]`);
      if (!target) continue;
      target.scrollIntoView({ behavior: 'smooth', block: 'center' });
      window.setTimeout(() => {
        const focusable = target.matches('input,textarea,select,button') ? target : target.querySelector<HTMLElement>('input,textarea,select,button');
        focusable?.focus();
      }, 220);
      break;
    }
  }

  function optimizeHealthIssues(issues: ResumeHealthIssue[]) {
    healthDrawerOpen = false;
    const prompt = `请根据以下本地简历体检问题提出修改建议，只使用已确认事实：\n${issues.slice(0, 12).map((item) => `- ${item.label}：${item.message}`).join('\n')}`;
    void openAssistant(null, prompt);
  }

  const emptyItemLabels = new Set(['新分组', '新项目', '新证书']);

  function hasEnteredContent(value: unknown): boolean {
    if (typeof value === 'string') {
      const normalized = value.trim();
      return Boolean(normalized && !emptyItemLabels.has(normalized));
    }
    if (Array.isArray(value)) return value.some(hasEnteredContent);
    if (value && typeof value === 'object') {
      return Object.entries(value).some(([key, child]) => key !== 'id' && hasEnteredContent(child));
    }
    return false;
  }

  function requestItemRemoval(label: string, value: unknown, apply: () => void) {
    if (!hasEnteredContent(value)) {
      apply();
      return;
    }
    pendingRemoval = { label, apply };
  }

  function cancelItemRemoval() {
    pendingRemoval = null;
  }

  function confirmItemRemoval() {
    const removal = pendingRemoval;
    pendingRemoval = null;
    removal?.apply();
  }

  function removeSkillGroup(groupIndex: number) {
    if (!draft) return;
    const group = draft.professionalSkills[groupIndex];
    if (!group) return;
    requestItemRemoval(`技能分组“${group.label || groupIndex + 1}”`, group, () => {
      if (draft) draft.professionalSkills = draft.professionalSkills.filter((item) => item.id !== group.id);
    });
  }

  function removeSkill(groupIndex: number, skillIndex: number) {
    if (!draft) return;
    const group = draft.professionalSkills[groupIndex];
    const skill = group?.items[skillIndex];
    if (skill === undefined) return;
    requestItemRemoval(`技能“${skill || skillIndex + 1}”`, skill, () => {
      if (!draft) return;
      draft.professionalSkills = draft.professionalSkills.map((item) => item.id === group.id
        ? { ...item, items: removeAt(item.items, skillIndex) }
        : item);
    });
  }

  function removeExperience(experienceIndex: number) {
    if (!draft) return;
    const experience = draft.experiences[experienceIndex];
    if (!experience) return;
    const name = [experience.company, experience.position].filter(Boolean).join(' / ') || `第 ${experienceIndex + 1} 条工作经历`;
    requestItemRemoval(`工作经历“${name}”`, experience, () => {
      if (draft) draft.experiences = draft.experiences.filter((item) => item.id !== experience.id);
    });
  }

  function removeProject(projectIndex: number) {
    if (!draft) return;
    const project = draft.projects[projectIndex];
    if (!project) return;
    requestItemRemoval(`项目“${project.name || projectIndex + 1}”`, project, () => {
      if (draft) draft.projects = draft.projects.filter((item) => item.id !== project.id);
    });
  }

  function removeCertification(certificationIndex: number) {
    if (!draft) return;
    const certification = draft.certifications[certificationIndex];
    if (!certification) return;
    requestItemRemoval(`证书“${certification.name || certificationIndex + 1}”`, certification, () => {
      if (draft) draft.certifications = draft.certifications.filter((item) => item.id !== certification.id);
    });
  }

  function removeEducation(educationIndex: number) {
    if (!draft) return;
    const education = draft.education[educationIndex];
    if (!education) return;
    requestItemRemoval(`教育经历“${education.institution || educationIndex + 1}”`, education, () => {
      if (draft) draft.education = draft.education.filter((item) => item.id !== education.id);
    });
  }

  onMount(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === 's') {
        event.preventDefault();
        if (hasUnsavedChanges && !saving) void save();
      }
    };
    window.addEventListener('keydown', handleShortcut);
    window.addEventListener('dragend', clearDragSource);
    if ($page.url?.searchParams.get('assistant') === '1') {
      const marketMode = $page.url.searchParams.get('market') === '1';
      const marketContext = marketMode ? {
        keywordKeys: $page.url.searchParams.getAll('keyword'),
        focusSkills: $page.url.searchParams.getAll('focusSkill')
      } : null;
      const focusLabel = marketContext?.focusSkills.join('、');
      const marketPrompt = marketMode
        ? focusLabel
          ? `请基于当前本地岗位样本，先核对我是否有与“${focusLabel}”相关的真实经历，再优化主简历表达。市场需求不是我的经历证据；缺少已确认事实时请先提问，不要直接生成修改。`
          : '请基于当前本地岗位样本为主简历安排优化优先级。市场需求不是我的经历证据；只能使用已确认事实，遇到真实缺口请先提问，不要直接生成修改。'
        : '';
      void openAssistant(marketMode ? null : $page.url.searchParams.get('job'), marketPrompt, marketContext);
    }
    return () => {
      window.removeEventListener('keydown', handleShortcut);
      window.removeEventListener('dragend', clearDragSource);
    };
  });
</script>

<div class="page-content flex h-[calc(100vh-74px)] min-h-[646px] max-w-none flex-col overflow-hidden pb-6">
  <div class="mb-5 flex shrink-0 items-end justify-between">
    <div class="min-w-0">
      <div class="mb-2 flex flex-wrap items-center gap-3">
        <div class="inline-flex rounded-xl p-1" style="background: var(--panel-soft);"><button class:active-mode={resumeMode === 'master'} class="mode-button" on:click={() => void switchResumeMode('master')}><FileText size={14} />主简历</button><button class:active-mode={resumeMode === 'variant'} class="mode-button" on:click={() => void switchResumeMode('variant')}><BriefcaseBusiness size={14} />岗位版本</button></div>
        {#if resumeMode === 'variant' && variants.length}
          <select class="select h-9 w-[260px] py-1 text-xs" value={activeVariant?.id ?? ''} aria-label="选择岗位版本" on:change={(event) => void selectVariant(event.currentTarget.value)}><option value="" disabled>选择已有岗位版本</option>{#each variants as variant}<option value={variant.id}>{variant.company} · {variant.jobTitle}{variant.stale ? ' · 待同步' : ''}</option>{/each}</select>
          <button class="btn h-9 text-xs" on:click={() => void startVariantCreation()}><Plus size={14} />新建岗位版本</button>
        {/if}
      </div>
      <p class="eyebrow">{resumeMode === 'master' ? 'MASTER RESUME' : 'TAILORED RESUME'}</p><h2 class="page-title mt-1">{resumeMode === 'master' ? '一份可信的主简历' : activeVariant?.name ?? '为目标岗位创建定制版本'}</h2><p class="mt-1 text-sm body-muted">{resumeMode === 'master' ? '结构化内容是唯一真源；手工修改和 AI 修改都会保存为可恢复的本地版本。' : '岗位版本固定主简历基线，专岗调整不会覆盖主简历。'}</p>
    </div>
    <div class="flex flex-wrap justify-end gap-2">
      <input bind:this={fileInput} class="hidden" type="file" accept=".pdf,.docx,.yaml,.yml" on:change={pickResume} />
      {#if resumeMode === 'master'}<button class="btn" disabled={importing} on:click={requestImport}><Upload size={15} />{importing ? '正在解析…' : draft ? '重新导入' : '导入简历'}</button>{/if}
      {#if resumeMode === 'variant' && activeVariant?.stale}<button class="btn" disabled={rebaseLoading || !$snapshot.resume} title={$snapshot.resume ? '' : '主简历暂时不可用'} on:click={openRebase}><RefreshCw size={15} class={rebaseLoading ? 'animate-spin' : ''} />同步主简历</button>{/if}
      <button class="btn" on:click={openVersionHistory}><History size={15} />版本历史</button>
      <button class="btn" disabled={!draft} on:click={() => healthDrawerOpen = true}><ShieldCheck size={15} />体检 {draft ? healthReport.issues.length : 0}</button>
      <button class="btn" disabled={!draft || Boolean(resumeMode === 'variant' && !$snapshot.resume)} on:click={() => openAssistant(null)}><Sparkles size={15} />AI 对话</button>
      <button class="btn" disabled={!draft || rendering} on:click={renderPdf}><Download size={15} />{rendering ? '正在渲染…' : '导出 PDF'}</button>
      {#if resumeMode === 'variant' && activeVariant}<button class="btn" on:click={deleteActiveVariant} aria-label="删除当前岗位版本"><Trash2 size={15} /></button>{/if}
      <button class="btn-primary" disabled={!draft || saving || !hasUnsavedChanges || Boolean(resumeMode === 'master' && pendingExternalResume)} on:click={save}><Save size={15} />{saving ? '正在保存…' : hasUnsavedChanges ? '保存修改' : '已保存'}</button>
    </div>
  </div>

  {#if pendingExternalResume}
    <div class="mb-4 flex shrink-0 items-center justify-between gap-4 rounded-xl border px-4 py-3" role="alert" style="border-color: var(--warning); background: var(--warning-soft);">
      <div><p class="text-sm font-semibold">检测到新的简历版本 {pendingExternalResume.version}</p><p class="mt-1 text-xs body-muted">当前本地修改尚未覆盖；载入新版本会放弃这些未保存修改。</p></div>
      <button class="btn shrink-0" on:click={() => pendingExternalResume && adoptResume(pendingExternalResume)}>载入最新版本</button>
    </div>
  {/if}

  {#if resumeMode === 'variant' && !draft}
    <div class="panel grid min-h-[500px] place-items-center p-8">
      <div class="w-full max-w-xl text-center"><span class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-brand-soft text-brand"><BriefcaseBusiness size={27} /></span><h3 class="text-xl font-semibold">{variants.length ? '创建新的岗位定制简历' : '创建第一份岗位定制简历'}</h3><p class="mt-2 text-sm leading-6 body-muted">从当前主简历即时复制；创建过程不会调用 AI，也不会改变主简历。</p>
        {#if !$snapshot.resume}<p class="mt-5 text-sm text-warning">请先在“主简历”模式导入或创建主简历。</p>{:else}<div class="mx-auto mt-6 flex max-w-lg gap-2"><input class="input min-w-0 flex-1" bind:value={jobQuery} on:input={() => void loadVariantJobs()} placeholder="搜索岗位或公司" aria-label="搜索岗位版本目标" /><select class="select min-w-0 flex-1" bind:value={selectedVariantJobId} aria-label="选择岗位版本目标"><option value="">选择岗位</option>{#each jobOptions as option}<option value={option.id}>{option.company} · {option.title}</option>{/each}</select><button class="btn-primary shrink-0" disabled={!selectedVariantJobId || creatingVariant} on:click={createVariant}>{creatingVariant ? '创建中…' : '创建岗位版本'}</button></div>{/if}
      </div>
    </div>
  {:else if !draft}
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
    <div class="grid min-h-0 flex-1 grid-cols-[minmax(460px,1fr)_minmax(380px,.82fr)] gap-5">
      <section class="panel flex min-h-0 flex-col overflow-hidden">
        <nav class="flex shrink-0 gap-6 border-b px-6 pt-4" style="border-color: var(--line);">
          <button class:active={activeSection === 'content'} class="tab" on:click={() => activeSection = 'content'}>简历内容</button>
          {#if resumeMode === 'master'}
            <button class:active={activeSection === 'preferences'} class="tab" on:click={() => activeSection = 'preferences'}>求职偏好</button>
            <button class:active={activeSection === 'facts'} class="tab" on:click={() => activeSection = 'facts'}>事实清单 <span class="ml-1 text-brand">{draft.facts.length}</span></button>
          {:else}
            <button class:active={activeSection === 'coverage'} class="tab" on:click={() => activeSection = 'coverage'}>岗位覆盖</button>
            <button class:active={activeSection === 'preferences'} class="tab" on:click={() => activeSection = 'preferences'}>求职偏好 · 只读</button>
            <button class:active={activeSection === 'facts'} class="tab" on:click={() => activeSection = 'facts'}>事实清单 · 只读 <span class="ml-1 text-brand">{draft.facts.length}</span></button>
          {/if}
        </nav>
        <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6">
          {#if activeSection === 'content'}
            <div class="space-y-7 animate-lift">
              <div>
                <div class="mb-3 flex items-center gap-2"><UserRound size={17} class="text-brand" /><h3 class="section-title">基本信息</h3></div>
                <div class="grid grid-cols-2 gap-4">
                  <label><span class="label">姓名</span><input class="input" data-resume-path="/name" aria-label="姓名" bind:value={draft.name} /></label>
                  <label><span class="label">职业标题</span><input class="input" data-resume-path="/headline" aria-label="职业标题" bind:value={draft.headline} /></label>
                  <label><span class="label">邮箱</span><input class="input" data-resume-path="/email" aria-label="邮箱" bind:value={draft.email} /></label>
                  <label><span class="label">电话</span><input class="input" data-resume-path="/phone" aria-label="电话" bind:value={draft.phone} /></label>
                  <label><span class="label">所在城市</span><input class="input" data-resume-path="/location" aria-label="所在城市" bind:value={draft.location} /></label>
                  <label><span class="label">个人主页</span><input class="input" data-resume-path="/website" aria-label="个人主页" bind:value={draft.website} /></label>
                  <label class="col-span-2"><span class="label">简历结构模板</span><div class="flex items-center gap-2"><select class="select" data-resume-path="/templateId" bind:value={draft.templateId}>{#each RESUME_TEMPLATES as template}<option value={template.id}>{template.label} · {template.description}</option>{/each}</select>{#if resumeTemplate(draft.templateId).sample}<button class="btn shrink-0" type="button" on:click={() => openTemplatePreview(draft!.templateId)}>查看示例</button>{/if}</div><span class="mt-1 block text-[11px] body-muted">切换模板只调整章节顺序，不会改写已有内容。</span></label>
                </div>
              </div>
              <div class="divider"></div>
              <div><div class="mb-3 flex items-center justify-between"><div class="flex items-center gap-2"><Sparkles size={17} class="text-brand" /><h3 class="section-title">个人简介</h3></div><button class="btn-ghost h-8 text-xs text-brand" on:click={() => openAssistant(null)}><WandSparkles size={14} />AI 优化</button></div><textarea class="textarea min-h-[120px] leading-6" data-resume-path="/summary" aria-label="个人简介" bind:value={draft.summary}></textarea><p class="mt-2 text-[11px] body-muted">建议保持 3–4 行；AI 只会提出待审核修改，不会直接覆盖。</p></div>
              <div class="divider"></div>
              <div data-resume-path="/professionalSkills">
                <div class="mb-3 flex items-center justify-between"><div><h3 class="section-title">专业技能</h3><p class="mt-1 text-[11px] body-muted">按岗位相关能力分组；分组标题不是个人事实。</p></div><div class="flex gap-2"><button class="btn-ghost h-8 text-xs" on:click={applySuggestedGroups}>使用模板分组</button><button class="btn-ghost h-8 text-xs" on:click={addSkillGroup}><Plus size={14} />添加分组</button></div></div>
                <div class="space-y-3">
                  {#each draft.professionalSkills as group, groupIndex (group.id)}
                    <article class="rounded-xl border p-4" data-resume-path={`/professionalSkills/${groupIndex}`} style="border-color: var(--line);" on:dragover|preventDefault on:drop={(event) => dropItem(event, 'professionalSkills', groupIndex)}>
                      <div class="flex items-center gap-2">
                        <span class="cursor-grab body-muted" role="presentation" draggable="true" aria-label={`拖动技能分组 ${groupIndex + 1}`} on:dragstart={(event) => beginDrag(event, 'professionalSkills', groupIndex)}><GripVertical size={15} /></span>
                        <input class="input h-9 font-semibold" data-resume-path={`/professionalSkills/${groupIndex}/label`} aria-label={`技能分组 ${groupIndex + 1}`} bind:value={group.label} />
                        <button class="btn-icon h-8 w-8" disabled={groupIndex === 0} aria-label={`上移技能分组 ${groupIndex + 1}`} on:click={() => moveResumeItem('professionalSkills', groupIndex, groupIndex - 1)}><ArrowUp size={13} /></button>
                        <button class="btn-icon h-8 w-8" disabled={groupIndex === draft.professionalSkills.length - 1} aria-label={`下移技能分组 ${groupIndex + 1}`} on:click={() => moveResumeItem('professionalSkills', groupIndex, groupIndex + 1)}><ArrowDown size={13} /></button>
                        <button type="button" class="btn-icon h-8 w-8" aria-label={`添加技能到${group.label || `分组 ${groupIndex + 1}`}`} title="添加技能" on:click={() => addSkill(groupIndex)}><Plus size={14} /></button>
                        <button class="btn-ghost h-8 text-xs" aria-label={`删除专业技能分组 ${groupIndex + 1}`} on:click={() => removeSkillGroup(groupIndex)}>×</button>
                      </div>
                      <div class="mt-3 flex flex-wrap gap-2">{#each group.items as skill, skillIndex}<label class="chip-brand group cursor-text" data-resume-path={`/professionalSkills/${groupIndex}/items/${skillIndex}`}><input class="w-[96px] bg-transparent outline-none" aria-label={`技能 ${groupIndex + 1}.${skillIndex + 1}`} bind:value={group.items[skillIndex]} /><button type="button" class="ml-1 opacity-60 transition hover:opacity-100 focus:opacity-100" aria-label={`删除技能：${skill || skillIndex + 1}`} title="删除技能" on:click|preventDefault|stopPropagation={() => removeSkill(groupIndex, skillIndex)}>×</button></label>{/each}</div>
                    </article>
                  {/each}
                </div>
              </div>
              <div class="divider"></div>
              <div>
                <div class="mb-4 flex items-center justify-between"><h3 class="section-title">工作经历</h3><button class="btn-ghost h-8 text-xs" on:click={addExperience}><Plus size={14} />添加经历</button></div>
                <div class="space-y-4" data-resume-path="/experiences">
                  {#each draft.experiences as experience, experienceIndex (experience.id)}
                    <article class="rounded-xl border p-4" data-resume-path={`/experiences/${experienceIndex}`} style="border-color: var(--line);" on:dragover|preventDefault on:drop={(event) => dropItem(event, 'experiences', experienceIndex)}>
                      <div class="mb-3 flex items-center justify-between gap-3">
                        <span class="flex cursor-grab items-center gap-1 text-xs body-muted" role="presentation" draggable="true" aria-label={`拖动工作经历 ${experienceIndex + 1}`} on:dragstart={(event) => beginDrag(event, 'experiences', experienceIndex)}><GripVertical size={15} />经历 {experienceIndex + 1}</span>
                        <div class="flex items-center gap-1"><button class="btn-icon h-8 w-8" disabled={experienceIndex === 0} aria-label={`上移工作经历 ${experienceIndex + 1}`} on:click={() => moveResumeItem('experiences', experienceIndex, experienceIndex - 1)}><ArrowUp size={13} /></button><button class="btn-icon h-8 w-8" disabled={experienceIndex === draft.experiences.length - 1} aria-label={`下移工作经历 ${experienceIndex + 1}`} on:click={() => moveResumeItem('experiences', experienceIndex, experienceIndex + 1)}><ArrowDown size={13} /></button><button class="btn-ghost h-8" aria-label={`删除工作经历：${experience.company || experience.position || experienceIndex + 1}`} on:click={() => removeExperience(experienceIndex)}>×</button></div>
                      </div>
                      <div class="grid grid-cols-2 gap-3">
                        <input class="input font-semibold" data-resume-path={`/experiences/${experienceIndex}/company`} aria-label="公司名称" placeholder="公司名称" bind:value={experience.company} />
                        <input class="input" data-resume-path={`/experiences/${experienceIndex}/position`} aria-label="职位名称" placeholder="职位名称" bind:value={experience.position} />
                        <input class="input" data-resume-path={`/experiences/${experienceIndex}/location`} aria-label={`工作地点 ${experienceIndex + 1}`} placeholder="工作地点" bind:value={experience.location} />
                        <div class="grid grid-cols-2 gap-2"><input class="input" data-resume-path={`/experiences/${experienceIndex}/startDate`} aria-label={`工作开始时间 ${experienceIndex + 1}`} placeholder="开始时间" bind:value={experience.startDate} /><input class="input" data-resume-path={`/experiences/${experienceIndex}/endDate`} aria-label={`工作结束时间 ${experienceIndex + 1}`} placeholder="结束时间 / 至今" bind:value={experience.endDate} /></div>
                      </div>
                      <div class="mt-3 space-y-2">
                        {#each experience.highlights as highlight, index}
                          <div class="flex gap-2" role="group" data-resume-path={`/experiences/${experienceIndex}/highlights/${index}`} on:dragover|preventDefault on:drop={(event) => dropItem(event, 'highlight:experiences', index, experienceIndex)}>
                            <span class="mt-3 cursor-grab body-muted" role="presentation" draggable="true" aria-label={`拖动经历成果 ${index + 1}`} on:dragstart={(event) => beginDrag(event, 'highlight:experiences', index, experienceIndex)}><GripVertical size={14} /></span>
                            <textarea class="textarea min-h-[66px]" aria-label={`经历成果 ${index + 1}`} placeholder="经历成果" bind:value={experience.highlights[index]}></textarea>
                            <div class="flex flex-col"><button class="btn-icon h-7 w-7" disabled={index === 0} aria-label={`上移经历成果 ${index + 1}`} on:click={() => moveHighlight('experiences', experienceIndex, index, index - 1)}><ArrowUp size={12} /></button><button class="btn-icon h-7 w-7" disabled={index === experience.highlights.length - 1} aria-label={`下移经历成果 ${index + 1}`} on:click={() => moveHighlight('experiences', experienceIndex, index, index + 1)}><ArrowDown size={12} /></button><button class="btn-icon h-7 w-7" aria-label={`删除经历成果 ${index + 1}`} on:click={() => removeHighlight('experiences', experienceIndex, index)}>×</button></div>
                          </div>
                        {/each}
                      </div>
                      <button type="button" class="btn-icon mt-3 h-8 w-8" aria-label={`添加经历成果 ${experienceIndex + 1}`} title="添加成果" on:click={() => addHighlight('experiences', experienceIndex)}><Plus size={14} /></button>
                    </article>
                  {/each}
                </div>
              </div>
              <div class="divider"></div>
              <div data-resume-path="/projects">
                <div class="mb-4 flex items-center justify-between"><h3 class="section-title">项目经历</h3><button class="btn-ghost h-8 text-xs" on:click={addProject}><Plus size={14} />添加项目</button></div>
                <div class="space-y-4">
                  {#each draft.projects as project, projectIndex (project.id)}
                    <article class="rounded-xl border p-4" data-resume-path={`/projects/${projectIndex}`} style="border-color: var(--line);" on:dragover|preventDefault on:drop={(event) => dropItem(event, 'projects', projectIndex)}>
                      <div class="mb-3 flex items-center justify-between"><span class="flex cursor-grab items-center gap-1 text-xs body-muted" role="presentation" draggable="true" aria-label={`拖动项目 ${projectIndex + 1}`} on:dragstart={(event) => beginDrag(event, 'projects', projectIndex)}><GripVertical size={15} />项目 {projectIndex + 1}</span><div class="flex gap-1"><button class="btn-icon h-8 w-8" disabled={projectIndex === 0} aria-label={`上移项目 ${projectIndex + 1}`} on:click={() => moveResumeItem('projects', projectIndex, projectIndex - 1)}><ArrowUp size={13} /></button><button class="btn-icon h-8 w-8" disabled={projectIndex === draft.projects.length - 1} aria-label={`下移项目 ${projectIndex + 1}`} on:click={() => moveResumeItem('projects', projectIndex, projectIndex + 1)}><ArrowDown size={13} /></button><button class="btn-ghost h-8" aria-label={`删除项目：${project.name || projectIndex + 1}`} on:click={() => removeProject(projectIndex)}>×</button></div></div>
                      <input class="input font-semibold" data-resume-path={`/projects/${projectIndex}/name`} aria-label={`项目名称 ${projectIndex + 1}`} bind:value={project.name} placeholder="项目名称" />
                      <textarea class="textarea mt-3" data-resume-path={`/projects/${projectIndex}/summary`} aria-label={`项目简介 ${projectIndex + 1}`} bind:value={project.summary} placeholder="项目简介"></textarea>
                      <div class="mt-3 grid grid-cols-2 gap-3"><input class="input" data-resume-path={`/projects/${projectIndex}/startDate`} aria-label={`项目开始时间 ${projectIndex + 1}`} bind:value={project.startDate} placeholder="开始时间" /><input class="input" data-resume-path={`/projects/${projectIndex}/endDate`} aria-label={`项目结束时间 ${projectIndex + 1}`} bind:value={project.endDate} placeholder="结束时间" /></div>
                      <div class="mt-3 space-y-2">{#each project.highlights as highlight, index}<div class="flex gap-2" role="group" data-resume-path={`/projects/${projectIndex}/highlights/${index}`} on:dragover|preventDefault on:drop={(event) => dropItem(event, 'highlight:projects', index, projectIndex)}><span class="mt-3 cursor-grab body-muted" role="presentation" draggable="true" aria-label={`拖动项目成果 ${index + 1}`} on:dragstart={(event) => beginDrag(event, 'highlight:projects', index, projectIndex)}><GripVertical size={14} /></span><textarea class="textarea min-h-[60px]" aria-label={`项目成果 ${index + 1}`} bind:value={project.highlights[index]} placeholder="项目成果"></textarea><div class="flex flex-col"><button class="btn-icon h-7 w-7" disabled={index === 0} aria-label={`上移项目成果 ${index + 1}`} on:click={() => moveHighlight('projects', projectIndex, index, index - 1)}><ArrowUp size={12} /></button><button class="btn-icon h-7 w-7" disabled={index === project.highlights.length - 1} aria-label={`下移项目成果 ${index + 1}`} on:click={() => moveHighlight('projects', projectIndex, index, index + 1)}><ArrowDown size={12} /></button><button class="btn-icon h-7 w-7" aria-label={`删除项目成果 ${index + 1}`} on:click={() => removeHighlight('projects', projectIndex, index)}>×</button></div></div>{/each}</div>
                      <button type="button" class="btn-icon mt-3 h-8 w-8" aria-label={`添加项目成果 ${projectIndex + 1}`} title="添加成果" on:click={() => addHighlight('projects', projectIndex)}><Plus size={14} /></button>
                    </article>
                  {/each}
                </div>
              </div>
              <div class="divider"></div>
              <div data-resume-path="/certifications">
                <div class="mb-4 flex items-center justify-between"><h3 class="section-title">证书 / 专业资质</h3><button class="btn-ghost h-8 text-xs" on:click={addCertification}><Plus size={14} />添加证书</button></div>
                <div class="space-y-3">{#each draft.certifications as certification, certificationIndex (certification.id)}<article class="flex items-center gap-2 rounded-xl border p-4" data-resume-path={`/certifications/${certificationIndex}`} style="border-color: var(--line);" on:dragover|preventDefault on:drop={(event) => dropItem(event, 'certifications', certificationIndex)}><span class="cursor-grab body-muted" role="presentation" draggable="true" aria-label={`拖动证书 ${certificationIndex + 1}`} on:dragstart={(event) => beginDrag(event, 'certifications', certificationIndex)}><GripVertical size={15} /></span><input class="input min-w-0 flex-1 font-semibold" data-resume-path={`/certifications/${certificationIndex}/name`} aria-label={`证书名称 ${certificationIndex + 1}`} bind:value={certification.name} placeholder="证书名称" /><input class="input min-w-0 flex-1" aria-label={`证书颁发机构 ${certificationIndex + 1}`} bind:value={certification.issuer} placeholder="颁发机构" /><input class="input w-[130px]" aria-label={`证书日期 ${certificationIndex + 1}`} bind:value={certification.date} placeholder="取得日期" /><button class="btn-icon h-8 w-8" disabled={certificationIndex === 0} aria-label={`上移证书 ${certificationIndex + 1}`} on:click={() => moveResumeItem('certifications', certificationIndex, certificationIndex - 1)}><ArrowUp size={13} /></button><button class="btn-icon h-8 w-8" disabled={certificationIndex === draft.certifications.length - 1} aria-label={`下移证书 ${certificationIndex + 1}`} on:click={() => moveResumeItem('certifications', certificationIndex, certificationIndex + 1)}><ArrowDown size={13} /></button><button class="btn-ghost h-8" aria-label={`删除证书：${certification.name || certificationIndex + 1}`} on:click={() => removeCertification(certificationIndex)}>×</button></article>{/each}</div>
              </div>
              <div class="divider"></div>
              <div data-resume-path="/education">
                <div class="mb-4 flex items-center justify-between"><h3 class="section-title">教育经历</h3><button class="btn-ghost h-8 text-xs" on:click={addEducation}><Plus size={14} />添加教育经历</button></div>
                <div class="space-y-4">
                  {#each draft.education as education, educationIndex (education.id)}
                    <article class="rounded-xl border p-4" data-resume-path={`/education/${educationIndex}`} style="border-color: var(--line);" on:dragover|preventDefault on:drop={(event) => dropItem(event, 'education', educationIndex)}>
                      <div class="mb-3 flex items-center justify-between"><span class="flex cursor-grab items-center gap-1 text-xs body-muted" role="presentation" draggable="true" aria-label={`拖动教育经历 ${educationIndex + 1}`} on:dragstart={(event) => beginDrag(event, 'education', educationIndex)}><GripVertical size={15} />教育 {educationIndex + 1}</span><div class="flex gap-1"><button class="btn-icon h-8 w-8" disabled={educationIndex === 0} aria-label={`上移教育经历 ${educationIndex + 1}`} on:click={() => moveResumeItem('education', educationIndex, educationIndex - 1)}><ArrowUp size={13} /></button><button class="btn-icon h-8 w-8" disabled={educationIndex === draft.education.length - 1} aria-label={`下移教育经历 ${educationIndex + 1}`} on:click={() => moveResumeItem('education', educationIndex, educationIndex + 1)}><ArrowDown size={13} /></button><button class="btn-ghost h-8" aria-label={`删除教育经历：${education.institution || educationIndex + 1}`} on:click={() => removeEducation(educationIndex)}>×</button></div></div>
                      <div class="grid grid-cols-2 gap-3"><input class="input font-semibold" data-resume-path={`/education/${educationIndex}/institution`} aria-label={`学校 ${educationIndex + 1}`} bind:value={education.institution} placeholder="学校" /><input class="input" aria-label={`专业 ${educationIndex + 1}`} bind:value={education.area} placeholder="专业" /></div>
                      <div class="mt-3 grid grid-cols-2 gap-3"><label><span class="label">学历</span><select class="select" aria-label="学历" bind:value={education.degree}><option value="">请选择</option><option value="本科">本科</option><option value="硕士">硕士</option><option value="博士">博士</option><option value="其他">其他</option></select></label>{#if education.degree === '其他'}<label><span class="label">学历原文</span><input class="input" bind:value={education.degreeDetail} placeholder="例如：大专、Bachelor of Science" /></label>{:else}<div class="grid grid-cols-2 gap-2"><label><span class="label">开始时间</span><input class="input" data-resume-path={`/education/${educationIndex}/startDate`} bind:value={education.startDate} placeholder="2019.09" /></label><label><span class="label">结束时间</span><input class="input" data-resume-path={`/education/${educationIndex}/endDate`} bind:value={education.endDate} placeholder="2023.06" /></label></div>{/if}</div>
                      {#if education.degree === '其他'}<div class="mt-3 grid grid-cols-2 gap-3"><input class="input" data-resume-path={`/education/${educationIndex}/startDate`} bind:value={education.startDate} placeholder="开始时间" /><input class="input" data-resume-path={`/education/${educationIndex}/endDate`} bind:value={education.endDate} placeholder="结束时间" /></div>{/if}
                      <div class="mt-3 space-y-2">{#each education.highlights as highlight, index}<div class="flex gap-2" role="group" data-resume-path={`/education/${educationIndex}/highlights/${index}`} on:dragover|preventDefault on:drop={(event) => dropItem(event, 'highlight:education', index, educationIndex)}><span class="mt-3 cursor-grab body-muted" role="presentation" draggable="true" aria-label={`拖动教育成果 ${index + 1}`} on:dragstart={(event) => beginDrag(event, 'highlight:education', index, educationIndex)}><GripVertical size={14} /></span><textarea class="textarea min-h-[60px]" aria-label={`教育成果 ${index + 1}`} bind:value={education.highlights[index]} placeholder="课程、荣誉或研究成果"></textarea><div class="flex flex-col"><button class="btn-icon h-7 w-7" disabled={index === 0} aria-label={`上移教育成果 ${index + 1}`} on:click={() => moveHighlight('education', educationIndex, index, index - 1)}><ArrowUp size={12} /></button><button class="btn-icon h-7 w-7" disabled={index === education.highlights.length - 1} aria-label={`下移教育成果 ${index + 1}`} on:click={() => moveHighlight('education', educationIndex, index, index + 1)}><ArrowDown size={12} /></button><button class="btn-icon h-7 w-7" aria-label={`删除教育成果 ${index + 1}`} on:click={() => removeHighlight('education', educationIndex, index)}>×</button></div></div>{/each}</div>
                      <button type="button" class="btn-icon mt-3 h-8 w-8" aria-label={`添加教育成果 ${educationIndex + 1}`} title="添加教育成果" on:click={() => addHighlight('education', educationIndex)}><Plus size={14} /></button>
                    </article>
                  {/each}
                </div>
              </div>
            </div>
          {:else if activeSection === 'preferences'}
            {#if resumeMode === 'variant'}
              <div class="animate-lift space-y-4">
                <div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--brand-faint);">
                  <div class="flex gap-3"><ShieldCheck size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">来自主简历基线的只读偏好</p><p class="mt-1 text-xs leading-5 body-muted">岗位版本不维护独立偏好；同步主简历后会刷新这里的快照。</p></div></div>
                </div>
                <dl class="grid grid-cols-2 gap-3 text-sm">
                  <div class="readonly-card"><dt>目标岗位</dt><dd>{draft.preferences.targetRoles.join('、') || '未填写'}</dd></div>
                  <div class="readonly-card"><dt>目标城市</dt><dd>{draft.preferences.cities.join('、') || '未填写'}</dd></div>
                  <div class="readonly-card"><dt>工作方式</dt><dd>{draft.preferences.remotePreference || '未填写'}</dd></div>
                  <div class="readonly-card"><dt>让你有动力的任务</dt><dd>{draft.preferences.energizingTasks.join('、') || '未填写'}</dd></div>
                  <div class="readonly-card"><dt>希望减少的任务</dt><dd>{draft.preferences.drainingTasks.join('、') || '未填写'}</dd></div>
                  <div class="readonly-card"><dt>不可妥协的条件</dt><dd>{draft.preferences.hardConstraints.join('、') || '未填写'}</dd></div>
                </dl>
              </div>
            {:else}
            <div class="animate-lift"><div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--brand-faint);"><div class="flex gap-3"><Sparkles size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">约 60 秒，提高匹配结果的可信度</p><p class="mt-1 text-xs leading-5 body-muted">不知道的维度不会被 AI 硬猜。未填写项会降低置信度，而不会直接扣分。</p></div></div></div><div class="mt-6 space-y-5"><label><span class="label">目标岗位</span><input class="input" bind:value={targetRolesText} placeholder="AI Agent、大模型应用" /><span class="mt-1 block text-[11px] body-muted">使用顿号分隔多个方向</span></label><label><span class="label">目标城市</span><input class="input" bind:value={citiesText} placeholder="上海、杭州" /></label><label><span class="label">工作方式</span><select class="select" bind:value={draft.preferences.remotePreference}><option value="onsite">到岗办公</option><option value="hybrid">混合办公</option><option value="remote">远程办公</option><option value="flexible">均可</option></select></label><label><span class="label">让你有动力的任务</span><textarea class="textarea" bind:value={energizingText} placeholder="从 0 到 1、系统设计、Agent 工作流"></textarea></label><label><span class="label">希望减少的任务</span><textarea class="textarea" bind:value={drainingText} placeholder="长期纯维护、高频汇报"></textarea></label><label><span class="label">不可妥协的条件</span><textarea class="textarea" bind:value={constraintsText} placeholder="不接受长期出差"></textarea></label><div class="flex justify-end"><button class="btn-primary" on:click={savePrefs}><Check size={15} />保存求职偏好</button></div></div></div>
            {/if}
          {:else if activeSection === 'facts'}
            {#if resumeMode === 'variant'}
              <div class="animate-lift space-y-4">
                <div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--brand-faint);">
                  <div class="flex gap-3"><ShieldCheck size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">来自主简历基线的只读事实</p><p class="mt-1 text-xs leading-5 body-muted">新增事实需要先回到主简历确认，再手动同步到岗位版本。</p></div></div>
                </div>
                {#if draft.facts.length}
                  <div class="space-y-2">{#each draft.facts as fact}<article class="readonly-card"><div class="flex items-center justify-between gap-3"><span class="text-xs font-semibold text-brand">{fact.category}</span><span class="text-[11px] body-muted">{fact.confirmed ? '已确认' : '未确认'}</span></div><p class="mt-2 text-sm leading-6">{fact.value}</p><p class="mt-1 text-[11px] body-muted">{fact.source}</p></article>{/each}</div>
                {:else}
                  <p class="rounded-xl border p-5 text-sm body-muted" style="border-color: var(--line);">基线主简历还没有事实记录。</p>
                {/if}
              </div>
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
          {:else}
            <ResumeCoveragePanel
              report={coverageReport}
              aiReady={aiReady && Boolean($snapshot.resume)}
              analyzing={coverageAnalyzing}
              on:analyze={() => void analyzeCoverage()}
              on:select={(event) => focusCoverageItem(event.detail.item)}
              on:optimize={(event) => optimizeCoverageItem(event.detail.item)}
            />
          {/if}
        </div>
      </section>

      <section class="panel flex min-h-0 flex-col overflow-hidden">
        <div class="flex h-[53px] shrink-0 items-center justify-between border-b px-5" style="border-color: var(--line);"><div class="flex items-center gap-2"><FileText size={16} class="text-brand" /><span class="text-sm font-semibold">RenderCV 预览</span></div><div class="flex items-center gap-3 text-[11px] body-muted"><span>{hasUnsavedChanges ? '有未保存修改' : '内容已保存'}</span><button class="btn-ghost h-8" on:click={openVersionHistory}><History size={14} />版本 {draft.version}</button></div></div>
        <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto p-6" style="background: var(--panel-soft);">
          <ResumePaper resume={draft} sections={previewSections} coverageKeywords={previewCoverageKeywords} />
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
  aiReady={aiReady && (resumeMode === 'master' || Boolean($snapshot.resume))}
  initialJobId={initialChatJobId}
  initialMarketContext={initialChatMarketContext}
  initialPrompt={healthAssistantPrompt}
  target={draft ? { kind: resumeMode, id: resumeMode === 'variant' ? activeVariant?.id ?? draft.id : draft.id } : null}
  on:applied={(event) => handleResumeCommit(event, 'applied')}
  on:requestimport={requestImport}
/>
<ResumeHealthDrawer
  bind:open={healthDrawerOpen}
  report={healthReport}
  {aiReady}
  on:select={(event) => void focusHealthIssue(event.detail.issue)}
  on:ai={(event) => optimizeHealthIssues(event.detail.issues)}
/>
<ResumeRebaseDialog
  bind:open={rebaseDialogOpen}
  preview={rebasePreview}
  applying={rebaseLoading && Boolean(rebasePreview)}
  on:apply={(event) => void applyRebase(event.detail.resolutions)}
/>
<ResumeVersionDrawer
  bind:open={versionDrawerOpen}
  resume={draft}
  {hasUnsavedChanges}
  variantId={resumeMode === 'variant' ? activeVariant?.id ?? null : null}
  on:restored={(event) => handleResumeCommit(event, 'restored')}
/>
<ResumeDeleteDialog
  open={Boolean(pendingRemoval)}
  itemLabel={pendingRemoval?.label ?? ''}
  onCancel={cancelItemRemoval}
  onConfirm={confirmItemRemoval}
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
  .mode-button { display: inline-flex; height: 30px; align-items: center; gap: 6px; border-radius: 9px; padding: 0 10px; font-size: 12px; font-weight: 600; color: var(--muted); }
  .mode-button.active-mode { background: var(--panel); color: var(--brand); box-shadow: 0 1px 4px rgba(0,0,0,.08); }
  .tab { position: relative; padding: 0 0 13px; font-size: 13px; font-weight: 600; color: var(--muted); }
  .tab.active { color: var(--ink); }
  .tab.active::after { content: ''; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; background: var(--brand); }
  .readonly-card { border: 1px solid var(--line); border-radius: 12px; background: var(--panel-soft); padding: 14px; }
  .readonly-card dt { color: var(--muted); font-size: 11px; font-weight: 600; }
  .readonly-card dd { margin-top: 6px; line-height: 1.6; }
  .export-theme-card { border-color: var(--line); background: var(--panel-soft); }
  .export-theme-card:hover, .export-theme-card.selected-export { border-color: var(--swatch-accent); transform: translateY(-2px); }
  .export-theme-card.selected-export { box-shadow: 0 0 0 2px color-mix(in srgb, var(--swatch-accent) 24%, transparent); }
  .export-swatch { display: flex; height: 104px; flex-direction: column; gap: 9px; border-radius: 10px; background: white; padding: 18px 16px; box-shadow: inset 0 0 0 1px rgba(0,0,0,.08); }
  .export-swatch span { display: block; height: 5px; border-radius: 999px; background: #cbd3cf; }
  .export-swatch span:first-child { width: 58%; height: 9px; background: var(--swatch-accent); }
  .export-swatch span:nth-child(2) { background: var(--swatch-accent); opacity: .72; }
</style>
