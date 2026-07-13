<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { CheckCircle2, PencilLine, Plus, RefreshCw, Save, Trash2, X } from 'lucide-svelte';
  import {
    RESUME_FACT_CATEGORIES,
    factsFromResumeContent,
    mergeResumeFacts,
    resumeFactCategoryLabel,
    resumeFactGuidance
  } from '$lib/resume-facts';
  import type { ResumeFact, ResumeFactCategory, ResumeProfile } from '$lib/types';
  import { modalFocus } from '$lib/modal-focus';

  export let resume: ResumeProfile;
  export let saving = false;
  export let hasUnsavedChanges = false;

  const dispatch = createEventDispatcher<{
    factschange: { facts: ResumeFact[] };
    save: void;
    notice: { message: string };
  }>();

  let statusFilter: 'all' | 'confirmed' | 'pending' = 'all';
  let categoryFilter: 'all' | ResumeFactCategory = 'all';
  let editorOpen = false;
  let editingFact: ResumeFact | null = null;
  let formCategory: ResumeFactCategory = 'experience';
  let formValue = '';
  let formConfirmed = false;
  let formError = '';

  $: guidance = resumeFactGuidance(resume.templateId);
  $: confirmedCount = resume.facts.filter((fact) => fact.confirmed).length;
  $: pendingCount = resume.facts.length - confirmedCount;
  $: filteredFacts = resume.facts.filter((fact) =>
    (statusFilter === 'all' || (statusFilter === 'confirmed' ? fact.confirmed : !fact.confirmed))
    && (categoryFilter === 'all' || fact.category === categoryFilter)
  );

  function emitFacts(facts: ResumeFact[]) {
    dispatch('factschange', { facts });
  }

  function openAdd() {
    editingFact = null;
    formCategory = 'experience';
    formValue = '';
    formConfirmed = false;
    formError = '';
    editorOpen = true;
  }

  function openEdit(fact: ResumeFact) {
    editingFact = fact;
    formCategory = fact.category;
    formValue = fact.value;
    formConfirmed = fact.confirmed;
    formError = '';
    editorOpen = true;
  }

  function semanticChanged(category = formCategory, value = formValue) {
    return !editingFact || category !== editingFact.category || value.trim() !== editingFact.value.trim();
  }

  function updateCategory(category: ResumeFactCategory) {
    formCategory = category;
    if (semanticChanged(category, formValue)) formConfirmed = false;
    else if (editingFact) formConfirmed = editingFact.confirmed;
  }

  function updateValue(value: string) {
    formValue = value;
    if (semanticChanged(formCategory, value)) formConfirmed = false;
    else if (editingFact) formConfirmed = editingFact.confirmed;
  }

  function saveFact() {
    const value = formValue.trim().replace(/\s+/g, ' ');
    if (!value) {
      formError = '事实内容不能为空。';
      return;
    }
    const changed = semanticChanged(formCategory, value);
    const next: ResumeFact = {
      id: editingFact?.id ?? crypto.randomUUID(),
      category: formCategory,
      value,
      source: changed ? '用户手工维护' : editingFact?.source ?? '用户手工维护',
      confidence: changed ? 1 : editingFact?.confidence ?? 1,
      confirmed: formConfirmed
    };
    emitFacts(editingFact
      ? resume.facts.map((fact) => fact.id === editingFact?.id ? next : fact)
      : [...resume.facts, next]);
    editorOpen = false;
    dispatch('notice', { message: changed && !formConfirmed ? '事实已保存为待确认' : '事实已更新' });
  }

  function toggleConfirmed(fact: ResumeFact, confirmed: boolean) {
    emitFacts(resume.facts.map((item) => item.id === fact.id ? { ...item, confirmed } : item));
  }

  function removeFact(fact: ResumeFact) {
    if (!window.confirm(`删除这条事实？\n\n${fact.value}\n\n删除不会修改简历正文，保存后仍可通过版本历史恢复。`)) return;
    emitFacts(resume.facts.filter((item) => item.id !== fact.id));
    dispatch('notice', { message: '事实已从草稿删除' });
  }

  function syncFromResume() {
    const result = mergeResumeFacts(resume.facts, factsFromResumeContent(resume));
    if (result.added > 0) emitFacts(result.facts);
    dispatch('notice', { message: result.added ? `已补全 ${result.added} 条待确认事实` : '当前简历没有新的可补全事实' });
  }
</script>

<div class="animate-lift">
  <div class="rounded-xl border p-4" style="border-color: var(--line); background: var(--brand-faint);">
    <div class="flex items-start gap-3"><CheckCircle2 size={18} class="mt-0.5 shrink-0 text-brand" /><div><p class="text-sm font-semibold">{guidance.title}</p><p class="mt-1 text-xs leading-5 body-muted">{guidance.examples.join(' · ')}</p><p class="mt-2 text-[11px] body-muted">事实清单只决定 AI 可以引用什么，不会反向改写简历正文。</p></div></div>
  </div>

  <div class="mt-5 flex flex-wrap items-end justify-between gap-4">
    <div><h3 class="section-title">AI 可以使用的事实</h3><p class="mt-1 text-xs body-muted">共 {resume.facts.length} 条 · {confirmedCount} 条已确认 · {pendingCount} 条待确认</p></div>
    <div class="flex flex-wrap gap-2"><button class="btn" on:click={syncFromResume}><RefreshCw size={14} />从简历补全</button><button class="btn" on:click={openAdd}><Plus size={14} />新增事实</button><button class="btn-primary" disabled={!hasUnsavedChanges || saving} on:click={() => dispatch('save') }><Save size={14} />{saving ? '正在保存…' : hasUnsavedChanges ? '保存事实清单' : '已保存'}</button></div>
  </div>

  <div class="mt-4 flex flex-wrap gap-3">
    <div class="inline-flex rounded-xl border p-1" style="border-color: var(--line);">
      {#each [['all', '全部'], ['pending', '待确认'], ['confirmed', '已确认']] as option}
        <button class:chip-brand={statusFilter === option[0]} class="rounded-lg px-3 py-1.5 text-xs font-semibold" on:click={() => statusFilter = option[0] as typeof statusFilter}>{option[1]}</button>
      {/each}
    </div>
    <label class="min-w-[160px]"><span class="sr-only">事实类别</span><select class="select h-9 py-1 text-xs" bind:value={categoryFilter}><option value="all">全部类别</option>{#each RESUME_FACT_CATEGORIES as category}<option value={category.value}>{category.label}</option>{/each}</select></label>
  </div>

  {#if filteredFacts.length}
    <div class="mt-4 space-y-3">
      {#each filteredFacts as fact (fact.id)}
        <article class="rounded-xl border p-4" style="border-color: var(--line);">
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1"><div class="flex flex-wrap items-center gap-2"><span class="chip px-2 py-0.5">{resumeFactCategoryLabel(fact.category)}</span><span class={fact.confirmed ? 'text-success text-[11px] font-semibold' : 'text-warning text-[11px] font-semibold'}>{fact.confirmed ? 'AI 可使用' : '待确认'}</span></div><p class="mt-2 text-sm font-medium leading-6">{fact.value}</p><div class="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[11px] body-muted"><span>来源：{fact.source}</span><span>来源可靠度 {Math.round(fact.confidence * 100)}%</span></div></div>
            <div class="flex shrink-0 items-center gap-1"><button class="btn-ghost h-8 text-xs" on:click={() => openEdit(fact)}><PencilLine size={13} />编辑</button><button class="btn-ghost h-8 px-2 text-warning" aria-label={`删除事实：${fact.value}`} on:click={() => removeFact(fact)}><Trash2 size={13} /></button></div>
          </div>
          <label class="mt-3 flex cursor-pointer items-center gap-2 border-t pt-3 text-xs font-medium" style="border-color: var(--line);"><input class="h-4 w-4 accent-[var(--brand)]" type="checkbox" checked={fact.confirmed} on:change={(event) => toggleConfirmed(fact, event.currentTarget.checked)} />我确认这条事实真实，允许 AI 在匹配、面试准备和招呼语中使用</label>
        </article>
      {/each}
    </div>
  {:else}
    <div class="mt-4 rounded-xl border border-dashed p-8 text-center" style="border-color: var(--line);"><p class="text-sm font-semibold">{resume.facts.length ? '当前筛选下没有事实' : '还没有事实记录'}</p><p class="mt-1 text-xs body-muted">可以从当前简历补全，再逐条核对并确认。</p>{#if !resume.facts.length}<button class="btn-primary mt-4" on:click={syncFromResume}><RefreshCw size={14} />从简历补全</button>{/if}</div>
  {/if}
</div>

{#if editorOpen}
  <button class="fixed inset-0 z-[80] bg-black/30 backdrop-blur-sm" tabindex="-1" on:click={() => editorOpen = false} aria-label="关闭事实编辑器"></button>
  <div class="fixed left-1/2 top-1/2 z-[81] w-[560px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 panel p-6" role="dialog" aria-modal="true" aria-labelledby="fact-editor-title" tabindex="-1" use:modalFocus={{ close: () => editorOpen = false }}>
    <div class="flex items-start justify-between gap-4"><div><p class="eyebrow">事实清单</p><h3 id="fact-editor-title" class="mt-1 text-xl font-semibold">{editingFact ? '编辑事实' : '新增事实'}</h3></div><button class="btn-icon" on:click={() => editorOpen = false} aria-label="关闭"><X size={16} /></button></div>
    <div class="mt-5 space-y-4">
      <label><span class="label">类别</span><select class="select" value={formCategory} on:change={(event) => updateCategory(event.currentTarget.value as ResumeFactCategory)}>{#each RESUME_FACT_CATEGORIES as category}<option value={category.value}>{category.label}</option>{/each}</select></label>
      <label><span class="label">事实内容</span><textarea class="textarea min-h-[120px]" value={formValue} on:input={(event) => updateValue(event.currentTarget.value)} placeholder="只填写能够确认真实、可被引用的候选人事实"></textarea></label>
      <div class="rounded-xl p-3 text-xs body-muted" style="background: var(--panel-soft);"><span class="font-semibold text-ink">来源：</span>{semanticChanged() ? '用户手工维护' : editingFact?.source ?? '用户手工维护'}</div>
      <label class="flex cursor-pointer items-start gap-3 rounded-xl border p-3 text-sm" style="border-color: var(--line);"><input class="mt-0.5 h-4 w-4 accent-[var(--brand)]" type="checkbox" bind:checked={formConfirmed} /><span><span class="block font-semibold">我确认内容真实</span><span class="mt-1 block text-xs body-muted">只有确认后，AI 才能引用这条事实。</span></span></label>
      {#if formError}<p class="text-xs text-warning">{formError}</p>{/if}
    </div>
    <div class="mt-6 flex justify-end gap-2"><button class="btn" on:click={() => editorOpen = false}>取消</button><button class="btn-primary" on:click={saveFact}>{editingFact ? '保存修改' : '添加事实'}</button></div>
  </div>
{/if}
