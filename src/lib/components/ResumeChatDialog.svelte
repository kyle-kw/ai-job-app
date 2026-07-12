<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { AlertCircle, BriefcaseBusiness, Check, LoaderCircle, Send, Sparkles, X } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import type {
    Job,
    ResumeChatMessage,
    ResumeChatProposal,
    ResumeCommitResult,
    ResumeFactCandidate,
    ResumeProfile
  } from '$lib/types';

  export let open = false;
  export let resume: ResumeProfile | null = null;
  export let jobs: Job[] = [];
  export let aiReady = false;
  export let initialJobId: string | null = null;

  const dispatch = createEventDispatcher<{
    applied: ResumeCommitResult;
    requestimport: void;
  }>();

  let selectedJobId = '';
  let lastInitialJobId: string | null = null;
  let input = '';
  let messages: ResumeChatMessage[] = [];
  let proposal: ResumeChatProposal | null = null;
  let selectedEditIds = new Set<string>();
  let confirmedFactCandidateIds = new Set<string>();
  let sending = false;
  let applying = false;
  let error = '';

  $: if (open && initialJobId !== lastInitialJobId) {
    selectedJobId = initialJobId && jobs.some((job) => job.id === initialJobId) ? initialJobId : '';
    lastInitialJobId = initialJobId;
  }
  $: requiredFactCandidateIds = proposal
    ? proposal.edits
        .filter((edit) => selectedEditIds.has(edit.id))
        .flatMap((edit) => edit.requiredFactCandidateIds)
    : [];
  $: missingRequiredFacts = requiredFactCandidateIds.filter((id) => !confirmedFactCandidateIds.has(id));
  $: canApply = Boolean(proposal && selectedEditIds.size > 0 && missingRequiredFacts.length === 0 && !applying && !sending);

  function close() {
    if (sending || applying) return;
    open = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') close();
  }

  function formatValue(value: unknown) {
    if (value === null || value === undefined || value === '') return '（空）';
    if (Array.isArray(value)) return value.length ? value.map((item) => typeof item === 'string' ? item : JSON.stringify(item)).join('\n') : '（空列表）';
    if (typeof value === 'object') return JSON.stringify(value, null, 2);
    return String(value);
  }

  function factCategoryLabel(candidate: ResumeFactCandidate) {
    return ({
      identity: '基本信息', experience: '工作经历', education: '教育经历', skill: '专业技能', project: '项目', certification: '证书资质', other: '其他'
    }[candidate.category]);
  }

  function toggleEdit(id: string, checked: boolean) {
    const next = new Set(selectedEditIds);
    if (checked) next.add(id); else next.delete(id);
    selectedEditIds = next;
  }

  function toggleFact(id: string, checked: boolean) {
    const next = new Set(confirmedFactCandidateIds);
    if (checked) next.add(id); else next.delete(id);
    confirmedFactCandidateIds = next;
  }

  function selectAllEdits() {
    if (!proposal) return;
    selectedEditIds = selectedEditIds.size === proposal.edits.length
      ? new Set<string>()
      : new Set(proposal.edits.map((edit) => edit.id));
  }

  function startImport() {
    open = false;
    dispatch('requestimport');
  }

  async function sendMessage() {
    const content = input.trim();
    if (!content || !resume || !aiReady || sending || applying) return;

    const userMessage: ResumeChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content
    };
    const requestMessages = [...messages, userMessage];
    messages = requestMessages;
    input = '';
    proposal = null;
    selectedEditIds = new Set<string>();
    confirmedFactCandidateIds = new Set<string>();
    error = '';
    sending = true;

    try {
      const nextProposal = await backend.proposeResumeChatEdits({
        resumeId: resume.id,
        expectedVersion: resume.version,
        jobId: selectedJobId || null,
        messages: requestMessages
      });
      proposal = nextProposal;
      selectedEditIds = new Set(nextProposal.edits.map((edit) => edit.id));
      messages = [...requestMessages, {
        id: `${nextProposal.proposalId}-assistant`,
        role: 'assistant',
        content: nextProposal.assistantMessage
      }];
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      sending = false;
    }
  }

  async function applySelectedEdits() {
    if (!proposal || !canApply) return;
    applying = true;
    error = '';
    try {
      const result = await backend.applyResumeChatEdits({
        proposal,
        selectedEditIds: [...selectedEditIds],
        confirmedFactCandidateIds: [...confirmedFactCandidateIds],
        expectedVersion: proposal.baseVersion
      });
      messages = [...messages, {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `已应用 ${selectedEditIds.size} 项修改，并创建简历版本 ${result.resume.version}。`
      }];
      proposal = null;
      selectedEditIds = new Set<string>();
      confirmedFactCandidateIds = new Set<string>();
      dispatch('applied', result);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      applying = false;
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <button class="fixed inset-0 z-[70] bg-black/30 backdrop-blur-sm" aria-label="关闭简历 AI 对话" on:click={close}></button>
  <div class="fixed bottom-5 right-5 top-5 z-[80] flex w-[min(720px,calc(100vw-40px))] flex-col overflow-hidden rounded-2xl border bg-panel shadow-2xl" style="border-color: var(--line);" role="dialog" aria-modal="true" aria-labelledby="resume-chat-title">
    <header class="flex shrink-0 items-start justify-between gap-5 border-b px-6 py-5" style="border-color: var(--line);">
      <div>
        <div class="flex items-center gap-2 text-brand"><Sparkles size={17} /><p class="eyebrow">RESUME ASSISTANT</p></div>
        <h2 id="resume-chat-title" class="mt-1 text-xl font-semibold">和 AI 一起修改主简历</h2>
        <p class="mt-1 text-xs leading-5 body-muted">AI 只提出待审核修改；勾选并确认事实后才会写入新的本地版本。</p>
      </div>
      <button class="btn-icon shrink-0" aria-label="关闭" disabled={sending || applying} on:click={close}><X size={18} /></button>
    </header>

    {#if !resume}
      <div class="grid min-h-0 flex-1 place-items-center p-8 text-center">
        <div class="max-w-sm">
          <span class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft text-brand"><Sparkles size={23} /></span>
          <h3 class="section-title">先导入一份主简历</h3>
          <p class="mt-2 text-sm leading-6 body-muted">AI 对话会基于结构化主简历提出修改，不会创建另一份 YAML 真源。</p>
          <button class="btn-primary mt-5" on:click={startImport}>选择简历文件</button>
        </div>
      </div>
    {:else if !aiReady}
      <div class="grid min-h-0 flex-1 place-items-center p-8 text-center">
        <div class="max-w-sm">
          <span class="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-brand-soft text-brand"><Sparkles size={23} /></span>
          <h3 class="section-title">先配置并验证 AI 模型</h3>
          <p class="mt-2 text-sm leading-6 body-muted">只有主动发送消息时才会把当前简历和所选岗位的必要内容发送给模型厂商。</p>
          <a class="btn-primary mt-5" href="/settings">前往模型设置</a>
        </div>
      </div>
    {:else}
      <div class="shrink-0 border-b px-6 py-3" style="border-color: var(--line); background: var(--panel-soft);">
        <label class="flex items-center gap-3">
          <BriefcaseBusiness size={15} class="shrink-0 body-muted" />
          <span class="shrink-0 text-xs font-semibold">关联岗位</span>
          <select class="select h-9 min-w-0 flex-1" bind:value={selectedJobId} disabled={sending || applying} aria-label="关联岗位">
            <option value="">不指定岗位，优化通用主简历</option>
            {#each jobs as job}
              <option value={job.id}>{job.title} · {job.company}</option>
            {/each}
          </select>
        </label>
      </div>

      <div class="scrollbar-thin min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {#if messages.length === 0}
          <div class="rounded-2xl border p-5" style="border-color: var(--line); background: var(--brand-faint);">
            <h3 class="text-sm font-semibold">你可以这样开始</h3>
            <div class="mt-3 flex flex-wrap gap-2">
              {#each ['让个人简介更简洁有力', '梳理专业技能分组并去掉重复项', '针对所选岗位强化相关经历'] as suggestion}
                <button class="chip cursor-pointer text-left hover:text-brand" on:click={() => input = suggestion}>{suggestion}</button>
              {/each}
            </div>
          </div>
        {/if}

        <div class="space-y-4">
          {#each messages as message}
            <article class:ml-12={message.role === 'user'} class:mr-12={message.role === 'assistant'} class="rounded-2xl px-4 py-3 text-sm leading-6" style={message.role === 'user' ? 'background: var(--brand); color: white;' : 'background: var(--panel-soft);'}>
              <p class="whitespace-pre-wrap">{message.content}</p>
            </article>
          {/each}
          {#if sending}
            <div class="mr-12 flex items-center gap-2 rounded-2xl px-4 py-3 text-sm body-muted" style="background: var(--panel-soft);"><LoaderCircle size={15} class="animate-spin" />正在分析简历并生成可审核修改…</div>
          {/if}
        </div>

        {#if proposal}
          <section class="mt-5 space-y-4" aria-label="AI 修改提案">
            {#if proposal.warnings.length}
              <div class="rounded-xl border p-3" style="border-color: var(--line); background: var(--warning-soft);">
                {#each proposal.warnings as warning}<p class="flex gap-2 text-xs leading-5"><AlertCircle size={14} class="mt-0.5 shrink-0 text-warning" />{warning}</p>{/each}
              </div>
            {/if}

            {#if proposal.factCandidates.length}
              <div class="rounded-2xl border p-4" style="border-color: var(--line);">
                <div class="mb-3"><h3 class="text-sm font-semibold">待确认的新事实</h3><p class="mt-1 text-xs body-muted">只有你在对话中明确提供的事实才能确认；岗位描述不能成为个人事实。</p></div>
                <div class="space-y-2">
                  {#each proposal.factCandidates as candidate}
                    <label class="flex cursor-pointer items-start gap-3 rounded-xl p-3 hover:surface-soft">
                      <input class="mt-1 h-4 w-4 accent-[var(--brand)]" type="checkbox" checked={confirmedFactCandidateIds.has(candidate.id)} on:change={(event) => toggleFact(candidate.id, event.currentTarget.checked)} />
                      <span class="min-w-0"><span class="chip mb-1 px-2 py-0.5">{factCategoryLabel(candidate)}</span><span class="block text-sm leading-6">{candidate.value}</span></span>
                    </label>
                  {/each}
                </div>
              </div>
            {/if}

            <div class="rounded-2xl border" style="border-color: var(--line);">
              <div class="flex items-center justify-between border-b px-4 py-3" style="border-color: var(--line);">
                <div><h3 class="text-sm font-semibold">字段修改</h3><p class="mt-0.5 text-[11px] body-muted">已选择 {selectedEditIds.size} / {proposal.edits.length} 项</p></div>
                {#if proposal.edits.length}<button class="btn-ghost h-8 text-xs" on:click={selectAllEdits}>{selectedEditIds.size === proposal.edits.length ? '取消全选' : '全选'}</button>{/if}
              </div>
              {#if proposal.edits.length === 0}
                <p class="px-4 py-6 text-center text-sm body-muted">这轮对话没有产生可应用的字段修改。</p>
              {:else}
                <div class="divide-y" style="border-color: var(--line);">
                  {#each proposal.edits as edit}
                    <article class="p-4">
                      <label class="flex cursor-pointer items-start gap-3">
                        <input class="mt-1 h-4 w-4 shrink-0 accent-[var(--brand)]" type="checkbox" checked={selectedEditIds.has(edit.id)} on:change={(event) => toggleEdit(edit.id, event.currentTarget.checked)} />
                        <span class="min-w-0 flex-1">
                          <span class="flex flex-wrap items-center gap-2"><strong class="text-sm">{edit.label}</strong><code class="text-[10px] body-muted">{edit.path}</code></span>
                          <span class="mt-2 grid grid-cols-2 gap-3">
                            <span class="min-w-0 rounded-xl p-3" style="background: var(--panel-soft);"><span class="label">修改前</span><pre class="whitespace-pre-wrap break-words font-sans text-xs leading-5">{formatValue(edit.before)}</pre></span>
                            <span class="min-w-0 rounded-xl p-3" style="background: var(--brand-faint);"><span class="label text-brand">修改后</span><pre class="whitespace-pre-wrap break-words font-sans text-xs leading-5">{formatValue(edit.after)}</pre></span>
                          </span>
                          <span class="mt-2 block text-xs leading-5 body-muted">{edit.rationale}</span>
                          {#if edit.evidenceFactIds.length}<span class="mt-2 block text-[11px] body-muted">引用事实：{edit.evidenceFactIds.join('、')}</span>{/if}
                        </span>
                      </label>
                    </article>
                  {/each}
                </div>
              {/if}
            </div>

            <div class="sticky bottom-0 flex items-center justify-between gap-4 rounded-2xl border p-4 shadow-lg" style="border-color: var(--line); background: var(--panel);">
              <p class="text-xs leading-5 body-muted">{missingRequiredFacts.length ? `还需确认 ${missingRequiredFacts.length} 项关联事实` : '应用后会创建不可变的新版本，可随时从历史中恢复。'}</p>
              <button class="btn-primary shrink-0" disabled={!canApply} on:click={applySelectedEdits}>
                {#if applying}<LoaderCircle size={15} class="animate-spin" />正在应用…{:else}<Check size={15} />应用所选修改{/if}
              </button>
            </div>
          </section>
        {/if}

        {#if error}
          <div class="mt-4 flex gap-2 rounded-xl border p-3 text-xs leading-5" style="border-color: var(--line); background: var(--danger-soft);"><AlertCircle size={14} class="mt-0.5 shrink-0 text-danger" /><span><strong>操作失败：</strong>{error}</span></div>
        {/if}
      </div>

      <footer class="shrink-0 border-t px-6 py-4" style="border-color: var(--line);">
        <form class="flex items-end gap-3" on:submit|preventDefault={sendMessage}>
          <label class="min-w-0 flex-1"><span class="sr-only">发送给简历 AI 的消息</span><textarea class="textarea min-h-[72px] resize-none" bind:value={input} disabled={sending || applying} placeholder="描述希望修改的内容，或补充需要写入简历的真实事实…"></textarea></label>
          <button class="btn-primary h-[42px] shrink-0" type="submit" disabled={!input.trim() || sending || applying}><Send size={15} />发送</button>
        </form>
        <p class="mt-2 text-[11px] body-muted">仅在发送时调用当前模型；请勿把岗位要求当作自己的经历或成果。</p>
      </footer>
    {/if}
  </div>
{/if}
