<script lang="ts">
  import { Bot, Check, CheckCircle2, ChevronRight, CircleHelp, Code2, Eye, EyeOff, KeyRound, LockKeyhole, PlugZap, Save, ShieldCheck, Sparkles, TerminalSquare, TestTube2, XCircle } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import { refresh, saveSettings, snapshot } from '$lib/stores/app';
  import type { AiProviderConfig, AppSettings, ProviderTestResult } from '$lib/types';

  let selectedId = '';
  let draft: AiProviderConfig | null = null;
  let apiKey = '';
  let revealKey = false;
  let testing = false;
  let saving = false;
  let result: ProviderTestResult | null = null;
  let errorMessage = '';
  let toast = '';
  let localSettings: AppSettings | null = null;
  let settingsInitialized = false;

  $: visibleProviders = $snapshot.providers.filter((provider) => (provider.kind as string) !== 'openrouter');
  $: if (visibleProviders.length && (!selectedId || !visibleProviders.some((provider) => provider.id === selectedId))) {
    selectProvider(visibleProviders.find((provider) => provider.isDefault)?.id ?? visibleProviders.find((provider) => provider.kind === 'xiaomi')?.id ?? visibleProviders[0].id);
  }
  $: if (!settingsInitialized && $snapshot.settings) {
    localSettings = structuredClone($snapshot.settings);
    localSettings.locale = 'zh-CN';
    localSettings.theme = 'system';
    settingsInitialized = true;
  }

  function selectProvider(id: string) {
    selectedId = id;
    const found = $snapshot.providers.find((provider) => provider.id === id);
    draft = found ? structuredClone(found) : null;
    apiKey = '';
    result = null;
    errorMessage = '';
  }

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => toast === message && (toast = ''), 2400);
  }

  function syncDraft() {
    const saved = $snapshot.providers.find((provider) => provider.id === selectedId);
    if (saved) draft = structuredClone(saved);
  }

  async function test() {
    if (!draft) return;
    testing = true;
    result = null;
    errorMessage = '';
    try {
      result = await backend.testProvider({ ...draft, apiKey: apiKey || undefined });
      if (result.ok) {
        await refresh();
        syncDraft();
        apiKey = '';
        showToast('模型连接已验证');
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      testing = false;
    }
  }

  async function saveProvider() {
    if (!draft) return;
    saving = true;
    errorMessage = '';
    try {
      await backend.saveProvider({ ...draft, apiKey: apiKey || undefined });
      await refresh();
      syncDraft();
      apiKey = '';
      result = null;
      showToast('模型配置已保存，请重新测试连接');
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      saving = false;
    }
  }

  async function updateSettings() {
    if (!localSettings) return;
    const next = { ...localSettings, locale: 'zh-CN' as const, theme: 'system' as const };
    await saveSettings(next);
    localSettings = next;
    showToast('高级模式设置已保存');
  }

  const providerNote = (kind: string) => kind === 'xiaomi'
    ? '内置默认模型预设，使用你自己的 API Key'
    : '连接任意 OpenAI 兼容服务';
  const skills = [
    ['resume-extraction', '1.2.0', '文本、DOCX 表格与扫描 PDF 的可追溯提取'],
    ['job-detail-extraction', '1.0.0', '岗位、公司与工商信息结构化提取'],
    ['job-fit', '1.1.0', '简体中文岗位匹配与证据分析'],
    ['interview-preparation', '1.0.0', '基于本地统计生成 AI 面试准备'],
    ['greeting-message', '1.0.0', '不超过 60 字的招呼语']
  ];
</script>

<div class="page-content max-w-[1240px]">
  <div class="mb-7">
    <p class="eyebrow">SETTINGS</p>
    <h2 class="page-title mt-1">模型与高级能力</h2>
    <p class="mt-1 text-sm body-muted">日常使用只需要验证一个默认模型；界面固定为简体中文并跟随系统主题。</p>
  </div>

  <section class="panel overflow-hidden">
    <div class="flex items-center justify-between border-b px-6 py-5" style="border-color: var(--line);">
      <div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"><Bot size={19} /></span><div><h3 class="section-title">AI 模型服务</h3><p class="mt-0.5 text-xs body-muted">只有主动使用 AI 功能时，必要上下文才会发送到当前默认服务。</p></div></div>
      {#if $snapshot.readiness.ai}<span class="chip-brand"><CheckCircle2 size={13} />连接已验证</span>{:else}<span class="chip">尚未连接</span>{/if}
    </div>

    <div class="provider-layout grid grid-cols-[330px_1fr]">
      <div class="border-r p-4" style="border-color: var(--line); background: var(--panel-soft);">
        <p class="eyebrow mb-3 px-2">可用模型</p>
        <div class="space-y-2">
          {#each visibleProviders as provider}
            <button class:selected={selectedId === provider.id} class="provider-row flex w-full items-center gap-3 rounded-xl border p-3 text-left transition" style="border-color: var(--line);" on:click={() => selectProvider(provider.id)}>
              <span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-panel">{#if provider.kind === 'xiaomi'}<Sparkles size={17} />{:else}<Code2 size={17} />{/if}</span>
              <div class="min-w-0 flex-1"><div class="flex items-center gap-2"><p class="truncate text-sm font-semibold">{provider.kind === 'xiaomi' ? '默认模型' : '自定义模型'}</p>{#if provider.verified}<Check size={13} class="text-success" />{/if}</div><p class="mt-0.5 truncate text-[11px] body-muted">{provider.name}{provider.isDefault ? ' · 当前默认' : ''}{provider.visionVerified ? ' · 支持扫描件' : ''}</p></div>
              <ChevronRight size={15} class="body-muted" />
            </button>
          {/each}
          {#if visibleProviders.length === 0}<div class="rounded-xl border p-3 text-xs leading-5 body-muted" style="border-color: var(--line);">正在等待模型预设初始化。</div>{/if}
        </div>
        <div class="mt-4 rounded-xl border p-3" style="border-color: var(--line); background: var(--warning-soft);"><div class="flex gap-2"><CircleHelp size={15} class="mt-0.5 shrink-0 text-warning" /><p class="text-[11px] leading-5 body-muted">安装包不会内置共享 Key。你填写的 Key 只保存到系统钥匙串，不写入 SQLite 或日志。</p></div></div>
      </div>

      {#if draft}
        <div class="p-6">
          <div class="mb-6 flex items-start justify-between"><div><h4 class="text-lg font-semibold">{draft.name}</h4><p class="mt-1 text-xs body-muted">{providerNote(draft.kind)}</p></div>{#if draft.verified}<span class="chip-brand"><ShieldCheck size={13} />已通过结构化输出测试</span>{/if}</div>
          <div class="max-w-[690px] space-y-5">
            <label><span class="label">API Key</span><div class="relative"><KeyRound size={15} class="absolute left-3 top-3 body-muted" /><input class="input pl-9 pr-11" type={revealKey ? 'text' : 'password'} bind:value={apiKey} placeholder={draft.apiKeyRef ? '已安全保存；留空则保持不变' : '粘贴你的 API Key'} /><button type="button" class="absolute right-1.5 top-1.5 btn-ghost h-7 px-2" on:click={() => revealKey = !revealKey} aria-label="显示或隐藏密钥">{#if revealKey}<EyeOff size={14} />{:else}<Eye size={14} />{/if}</button></div><p class="mt-1.5 flex items-center gap-1.5 text-[11px] body-muted"><LockKeyhole size={12} />测试失败时，本次输入的 Key 不会被保存。</p></label>
            <label><span class="label">Base URL</span><input class="input" bind:value={draft.baseUrl} placeholder="https://token-plan-sgp.xiaomimimo.com/v1" /><p class="mt-1.5 text-[11px] body-muted">请求发送到 <code>{draft.baseUrl || 'Base URL'}/chat/completions</code></p></label>
            <div class="grid grid-cols-2 gap-4"><label><span class="label">模型</span><input class="input" bind:value={draft.model} placeholder="mimo-v2.5-pro" /></label><label><span class="label">设为默认</span><select class="select" bind:value={draft.isDefault}><option value={true}>是</option><option value={false}>否</option></select></label></div>

            {#if result}<div class="flex items-start gap-3 rounded-xl border p-3" style={`border-color:${result.ok ? 'var(--success)' : 'var(--danger)'}; background:${result.ok ? 'var(--brand-faint)' : 'var(--danger-soft)'}`}><svelte:component this={result.ok ? CheckCircle2 : XCircle} size={17} class={result.ok ? 'text-success' : 'text-danger'} /><div><p class="text-sm font-semibold">{result.message}</p><p class="mt-0.5 text-[11px] body-muted">延迟 {result.latencyMs} ms · 结构化输出 {result.structuredOutput ? '正常' : '未通过'} · {result.visionMessage}</p></div></div>{/if}
            {#if errorMessage}<div class="rounded-xl border p-3 text-xs leading-5 text-danger" style="border-color: var(--danger); background: var(--danger-soft);">{errorMessage}</div>{/if}

            <div class="flex justify-end gap-2"><button class="btn" disabled={testing || !draft.baseUrl || !draft.model || (!apiKey && !draft.apiKeyRef)} on:click={test}><TestTube2 size={15} />{testing ? '正在测试…' : '测试连接'}</button><button class="btn-primary" disabled={saving || !draft.baseUrl || !draft.model} on:click={saveProvider}><Save size={15} />{saving ? '正在保存…' : '保存配置'}</button></div>
          </div>
        </div>
      {/if}
    </div>
  </section>

  {#if localSettings}
    <section class="mt-6 panel overflow-hidden">
      <div class="flex items-center justify-between border-b px-6 py-5" style="border-color: var(--line);"><div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl surface-soft"><TerminalSquare size={18} /></span><div><h3 class="section-title">高级模式</h3><p class="mt-0.5 text-xs body-muted">显示运行日志、原始 YAML 和内置 Skill。</p></div></div><label class="relative inline-flex cursor-pointer items-center"><input class="peer sr-only" type="checkbox" bind:checked={localSettings.advancedMode} /><span class="h-6 w-11 rounded-full bg-[var(--line-strong)] transition peer-checked:bg-brand after:absolute after:left-1 after:top-1 after:h-4 after:w-4 after:rounded-full after:bg-white after:transition peer-checked:after:translate-x-5"></span></label></div>
      {#if localSettings.advancedMode}<div class="p-6 animate-lift"><div class="mb-4 flex items-center justify-between"><div><h4 class="text-sm font-semibold">内置 Skills</h4><p class="mt-1 text-xs body-muted">内置版本保持只读；仅展示当前产品仍在使用的能力。</p></div><span class="chip"><Code2 size={13} />{skills.length} 个已启用</span></div><div class="divide-y rounded-xl border" style="border-color: var(--line);">{#each skills as skill}<div class="flex items-center gap-4 px-4 py-3"><span class="grid h-8 w-8 place-items-center rounded-lg bg-brand-soft text-brand"><PlugZap size={15} /></span><div class="min-w-0 flex-1"><p class="font-mono text-xs font-semibold">{skill[0]}</p><p class="mt-0.5 text-[11px] body-muted">{skill[2]}</p></div><span class="chip">v{skill[1]}</span></div>{/each}</div></div>{/if}
    </section>
    <div class="mt-6 flex justify-end"><button class="btn-primary" on:click={updateSettings}><Save size={15} />保存高级模式设置</button></div>
  {/if}
</div>

{#if toast}<div class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift">{toast}</div>{/if}

<style>
  .provider-row { background: var(--panel); }
  .provider-row:hover, .provider-row.selected { border-color: var(--brand) !important; }
  .provider-row.selected { box-shadow: 0 0 0 2px var(--focus); }
  @media (max-width: 980px) { .provider-layout { grid-template-columns: minmax(0, 1fr); } .provider-layout > :first-child { border-right: 0; border-bottom: 1px solid var(--line); } }
</style>
