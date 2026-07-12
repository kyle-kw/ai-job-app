<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { AlertCircle, Check, ChevronRight, Clock3, FileText, LoaderCircle, RotateCcw, X } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import { resumeTemplate } from '$lib/resume-templates';
  import { displayDegree, formatDateRange } from '$lib/resume-format';
  import type { ResumeCommitResult, ResumeProfile, ResumeVersionDetail, ResumeVersionSource, ResumeVersionSummary } from '$lib/types';

  export let open = false;
  export let resume: ResumeProfile | null = null;
  export let hasUnsavedChanges = false;

  const dispatch = createEventDispatcher<{ restored: ResumeCommitResult }>();

  let versions: ResumeVersionSummary[] = [];
  let selectedId = '';
  let detail: ResumeVersionDetail | null = null;
  let loading = false;
  let loadingDetail = false;
  let restoring = false;
  let error = '';
  let wasOpen = false;
  let detailRequest = 0;
  $: detailSections = detail ? resumeTemplate(detail.profile.templateId).sectionOrder : [];

  $: if (open && !wasOpen) {
    wasOpen = true;
    void loadVersions();
  }
  $: if (!open && wasOpen) wasOpen = false;

  function sourceLabel(source: ResumeVersionSource) {
    return ({
      legacy: '历史版本', import: '导入', template: '空白模板', manual: '手工保存', 'ai-chat': 'AI 对话', rollback: '版本恢复'
    }[source]);
  }

  function formatTime(value: string) {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
  }

  function close() {
    if (restoring) return;
    open = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') close();
  }

  async function loadVersions(preferredId = '') {
    if (!resume) {
      versions = [];
      detail = null;
      return;
    }
    loading = true;
    error = '';
    try {
      versions = (await backend.listResumeVersions(resume.id)).sort((a, b) => b.version - a.version);
      const nextId = preferredId || selectedId || versions[0]?.id || '';
      if (nextId) await selectVersion(nextId);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      loading = false;
    }
  }

  async function selectVersion(versionId: string) {
    selectedId = versionId;
    detail = null;
    error = '';
    const request = ++detailRequest;
    loadingDetail = true;
    try {
      const nextDetail = await backend.getResumeVersion(versionId);
      if (request === detailRequest) detail = nextDetail;
    } catch (reason) {
      if (request === detailRequest) error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      if (request === detailRequest) loadingDetail = false;
    }
  }

  async function restoreSelected() {
    if (!resume || !detail || restoring || detail.version === resume.version) return;
    if (hasUnsavedChanges && !window.confirm('当前表单有未保存修改，恢复历史版本会放弃这些本地修改。是否继续？')) return;
    if (!window.confirm(`恢复版本 ${detail.version}？系统会基于它创建一个新的版本，不会删除任何历史记录。`)) return;

    restoring = true;
    error = '';
    try {
      const result = await backend.restoreResumeVersion(detail.id, resume.version);
      dispatch('restored', result);
      await loadVersions(result.version.id);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      restoring = false;
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <button class="fixed inset-0 z-[70] bg-black/25 backdrop-blur-[1px]" aria-label="关闭简历版本历史" on:click={close}></button>
  <aside class="fixed bottom-0 right-0 top-0 z-[80] flex w-[min(900px,calc(100vw-28px))] flex-col border-l bg-panel shadow-2xl" style="border-color: var(--line); animation: slide-in .22s ease-out;" aria-labelledby="resume-version-title">
    <header class="flex h-[74px] shrink-0 items-center justify-between border-b px-6" style="border-color: var(--line);">
      <div><h2 id="resume-version-title" class="text-base font-semibold">主简历版本历史</h2><p class="mt-0.5 text-xs body-muted">每次保存、导入、AI 应用和恢复都会创建不可变版本。</p></div>
      <button class="btn-icon" aria-label="关闭" disabled={restoring} on:click={close}><X size={18} /></button>
    </header>

    {#if !resume}
      <div class="grid min-h-0 flex-1 place-items-center p-8 text-center"><div><FileText size={25} class="mx-auto mb-3 body-muted" /><p class="text-sm font-semibold">导入主简历后即可查看版本历史</p></div></div>
    {:else}
      <div class="grid min-h-0 flex-1 grid-cols-[300px_minmax(0,1fr)]">
        <section class="scrollbar-thin min-h-0 overflow-y-auto border-r" style="border-color: var(--line);">
          {#if loading && versions.length === 0}
            <div class="flex items-center justify-center gap-2 p-8 text-sm body-muted"><LoaderCircle size={16} class="animate-spin" />正在加载版本…</div>
          {:else if versions.length === 0}
            <div class="p-8 text-center"><Clock3 size={22} class="mx-auto mb-3 body-muted" /><p class="text-sm font-semibold">还没有版本记录</p></div>
          {:else}
            {#each versions as version}
              <button class:selected={selectedId === version.id} class="version-row w-full border-b px-5 py-4 text-left transition" style="border-color: var(--line);" on:click={() => selectVersion(version.id)}>
                <div class="flex items-start gap-3">
                  <span class="mt-0.5 grid h-8 w-8 shrink-0 place-items-center rounded-lg" style={version.version === resume.version ? 'background: var(--brand-soft); color: var(--brand);' : 'background: var(--panel-soft);'}>{#if version.version === resume.version}<Check size={15} />{:else}<Clock3 size={15} />{/if}</span>
                  <span class="min-w-0 flex-1">
                    <span class="flex items-center justify-between gap-2"><strong class="text-sm">版本 {version.version}</strong>{#if version.version === resume.version}<span class="chip-brand px-2 py-0.5">当前</span>{/if}</span>
                    <span class="mt-1 block truncate text-xs body-muted">{sourceLabel(version.source)} · {version.summary}</span>
                    <span class="mt-1 block text-[10px] body-muted">{formatTime(version.createdAt)}</span>
                  </span>
                  <ChevronRight size={14} class="mt-2 shrink-0 body-muted" />
                </div>
              </button>
            {/each}
          {/if}
        </section>

        <section class="scrollbar-thin min-h-0 overflow-y-auto p-6" style="background: var(--panel-soft);">
          {#if loadingDetail}
            <div class="flex min-h-72 items-center justify-center gap-2 text-sm body-muted"><LoaderCircle size={16} class="animate-spin" />正在读取版本详情…</div>
          {:else if detail}
            <div class="mx-auto max-w-[560px]">
              <div class="mb-5 flex flex-wrap items-start justify-between gap-4">
                <div><p class="eyebrow">VERSION {detail.version}</p><h3 class="mt-1 text-xl font-semibold">{detail.summary}</h3><p class="mt-1 text-xs body-muted">{sourceLabel(detail.source)} · {formatTime(detail.createdAt)}</p></div>
                <button class="btn-primary" disabled={detail.version === resume.version || restoring} on:click={restoreSelected}>
                  {#if restoring}<LoaderCircle size={15} class="animate-spin" />正在恢复…{:else}<RotateCcw size={15} />{detail.version === resume.version ? '当前版本' : '恢复为新版本'}{/if}
                </button>
              </div>

              {#if detail.restoredFromVersion}<div class="mb-4 rounded-xl border p-3 text-xs body-muted" style="border-color: var(--line); background: var(--brand-faint);">此版本由版本 {detail.restoredFromVersion} 恢复创建。</div>{/if}
              {#if detail.jobId}<div class="mb-4 rounded-xl border p-3 text-xs body-muted" style="border-color: var(--line);">关联岗位 ID：{detail.jobId}</div>{/if}

              <article class="rounded-2xl bg-white p-8 text-[#17201d] shadow-lg">
                <header class="border-b-2 border-[#176b57] pb-4"><h1 class="text-2xl font-bold text-[#176b57]">{detail.profile.name}</h1><p class="mt-1 text-sm font-semibold">{detail.profile.headline}</p><p class="mt-2 text-[10px] text-[#5c6863]">{[detail.profile.email, detail.profile.phone, detail.profile.location].filter(Boolean).join(' · ')}</p></header>
                {#each detailSections as section}
                  {#if section === 'summary'}
                    <section class="version-section"><h4>个人简介</h4><p>{detail.profile.summary || '（空）'}</p></section>
                  {:else if section === 'professionalSkills'}
                    <section class="version-section"><h4>专业技能</h4>{#each detail.profile.professionalSkills as group}<p><strong>{group.label}：</strong>{group.items.join('、') || '（空）'}</p>{/each}</section>
                  {:else if section === 'experiences'}
                    <section class="version-section"><h4>工作经历</h4>{#each detail.profile.experiences as experience}<div class="mb-3"><strong>{experience.position} · {experience.company}</strong><p class="text-[10px] text-[#5c6863]">{formatDateRange(experience.startDate, experience.endDate)}</p>{#if experience.highlights.length}<ul>{#each experience.highlights as highlight}<li>{highlight}</li>{/each}</ul>{/if}</div>{/each}</section>
                  {:else if section === 'projects' && detail.profile.projects.length}
                    <section class="version-section"><h4>项目经历</h4>{#each detail.profile.projects as project}<div class="mb-3"><strong>{project.name}</strong><p>{project.summary}</p>{#if project.highlights.length}<ul>{#each project.highlights as highlight}<li>{highlight}</li>{/each}</ul>{/if}</div>{/each}</section>
                  {:else if section === 'certifications' && detail.profile.certifications.length}
                    <section class="version-section"><h4>证书 / 专业资质</h4>{#each detail.profile.certifications as certification}<p><strong>{certification.name}</strong>{certification.issuer ? ` · ${certification.issuer}` : ''}{certification.date ? ` · ${certification.date}` : ''}</p>{/each}</section>
                  {:else if section === 'education'}
                    <section class="version-section"><h4>教育经历</h4>{#each detail.profile.education as education}<div class="mb-2"><strong>{education.institution} · {education.area}</strong><p>{displayDegree(education)}{formatDateRange(education.startDate, education.endDate) ? ` · ${formatDateRange(education.startDate, education.endDate)}` : ''}</p></div>{/each}</section>
                  {/if}
                {/each}
              </article>
            </div>
          {:else}
            <div class="grid min-h-72 place-items-center text-sm body-muted">从左侧选择一个版本查看详情</div>
          {/if}
        </section>
      </div>
    {/if}

    {#if error}
      <div class="absolute bottom-5 left-[320px] right-5 flex gap-2 rounded-xl border p-3 text-xs shadow-lg" style="border-color: var(--line); background: var(--danger-soft);"><AlertCircle size={14} class="shrink-0 text-danger" /><span><strong>版本操作失败：</strong>{error}</span></div>
    {/if}
  </aside>
{/if}

<style>
  .version-row:hover, .version-row.selected { background: var(--brand-faint); }
  .version-row.selected { box-shadow: inset 3px 0 0 var(--brand); }
  .version-section { margin-top: 16px; font-size: 11px; line-height: 1.55; }
  .version-section h4 { margin-bottom: 6px; border-bottom: 1px solid #c4cec9; padding-bottom: 3px; color: #176b57; font-size: 12px; font-weight: 700; }
  .version-section ul { margin-top: 4px; list-style: disc; padding-left: 16px; }
</style>
