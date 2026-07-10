<script lang="ts">
  import { Bot, Check, CheckCircle2, ChevronRight, CircleHelp, Code2, Eye, EyeOff, Globe2, KeyRound, Languages, LockKeyhole, Moon, Palette, PlugZap, Save, ShieldCheck, Sparkles, Sun, TerminalSquare, TestTube2, XCircle } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import { locale } from '$lib/i18n';
  import { refresh, saveSettings, snapshot } from '$lib/stores/app';
  import type { AiProviderConfig, AppSettings, ProviderTestResult } from '$lib/types';

  let selectedId = '';
  let draft: AiProviderConfig | null = null;
  let apiKey = '';
  let revealKey = false;
  let testing = false;
  let saving = false;
  let result: ProviderTestResult | null = null;
  let toast = '';
  let localSettings: AppSettings | null = null;
  let settingsInitialized = false;

  $: if (!selectedId && $snapshot.providers.length) selectProvider($snapshot.providers.find((item) => item.isDefault)?.id ?? $snapshot.providers[0].id);
  $: if (!settingsInitialized && $snapshot.settings) { localSettings = structuredClone($snapshot.settings); settingsInitialized = true; }

  function selectProvider(id: string) { selectedId = id; const found = $snapshot.providers.find((item) => item.id === id); draft = found ? structuredClone(found) : null; apiKey = ''; result = null; }
  function showToast(message: string) { toast = message; window.setTimeout(() => toast === message && (toast = ''), 2400); }
  async function test() { if (!draft) return; testing = true; result = null; try { result = await backend.testProvider({ ...draft, apiKey }); if (result.ok) { await refresh(); const verified = $snapshot.providers.find((item) => item.id === selectedId); if (verified) draft = structuredClone(verified); apiKey = ''; showToast('模型连接已验证'); } } finally { testing = false; } }
  async function saveProvider() { if (!draft) return; saving = true; try { await backend.saveProvider({ ...draft, apiKey: apiKey || undefined }); await refresh(); apiKey = ''; showToast('模型配置已保存'); } finally { saving = false; } }
  async function updateSettings() { if (!localSettings) return; await saveSettings(localSettings); locale.set(localSettings.locale); showToast('偏好设置已保存'); }

  const providerIcon = (kind: string) => kind === 'xiaomi' ? Sparkles : kind === 'openrouter' ? Globe2 : Code2;
  const providerNote = (kind: string) => kind === 'xiaomi' ? '默认推荐，OpenAI 兼容' : kind === 'openrouter' ? '免费模型有额度与可用性限制' : '连接任意 OpenAI 兼容服务';
  const skills = [
    ['resume-extraction', '1.0.0', '简历结构化与事实提取'], ['job-market-analysis', '1.0.0', '岗位市场聚合报告'], ['job-fit', '1.0.0', '岗位匹配与证据分析'], ['resume-tailor', '1.0.0', '可审核的专岗简历改写'], ['greeting-message', '1.0.0', '不超过 60 字的招呼语']
  ];
  const themes: Array<{ value: AppSettings['theme']; icon: typeof Sun; label: string }> = [
    { value: 'light', icon: Sun, label: '浅色' },
    { value: 'dark', icon: Moon, label: '深色' },
    { value: 'system', icon: Palette, label: '跟随系统' }
  ];
</script>

<div class="page-content max-w-[1240px]">
  <div class="mb-7"><p class="eyebrow">PREFERENCES</p><h2 class="page-title mt-1">让复杂配置保持安静</h2><p class="mt-1 text-sm body-muted">普通模式只需要选择服务并填写 API Key，其余均有可靠默认值。</p></div>

  <section class="panel overflow-hidden">
    <div class="flex items-center justify-between border-b px-6 py-5" style="border-color: var(--line);"><div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"><Bot size={19} /></span><div><h3 class="section-title">AI 模型服务</h3><p class="mt-0.5 text-xs body-muted">简历与岗位内容只发送到当前选中的服务。</p></div></div>{#if $snapshot.readiness.ai}<span class="chip-brand"><CheckCircle2 size={13} />连接已验证</span>{:else}<span class="chip">尚未连接</span>{/if}</div>
    <div class="grid grid-cols-[330px_1fr]">
      <div class="border-r p-4" style="border-color: var(--line); background: var(--panel-soft);">
        <p class="eyebrow mb-3 px-2">服务预设</p>
        <div class="space-y-2">{#each $snapshot.providers as provider}<button class:selected={selectedId === provider.id} class="provider-row flex w-full items-center gap-3 rounded-xl border p-3 text-left transition" style="border-color: var(--line);" on:click={() => selectProvider(provider.id)}><span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-panel"><svelte:component this={providerIcon(provider.kind)} size={17} /></span><div class="min-w-0 flex-1"><div class="flex items-center gap-2"><p class="truncate text-sm font-semibold">{provider.name}</p>{#if provider.verified}<Check size={13} class="text-success" />{/if}</div><p class="mt-0.5 truncate text-[11px] body-muted">{providerNote(provider.kind)}</p></div><ChevronRight size={15} class="body-muted" /></button>{/each}</div>
        <div class="mt-4 rounded-xl border p-3" style="border-color: var(--line); background: var(--warning-soft);"><div class="flex gap-2"><CircleHelp size={15} class="mt-0.5 shrink-0 text-warning" /><p class="text-[11px] leading-5 body-muted">应用不会内置共享密钥。免费预设仍需创建自己的服务账号和 API Key。</p></div></div>
      </div>

      {#if draft}
        <div class="p-6">
          <div class="mb-6 flex items-start justify-between"><div><h4 class="text-lg font-semibold">{draft.name}</h4><p class="mt-1 text-xs body-muted">{providerNote(draft.kind)}</p></div>{#if draft.verified}<span class="chip-brand"><ShieldCheck size={13} />已通过结构化输出测试</span>{/if}</div>
          <div class="max-w-[690px] space-y-5">
            <label><span class="label">API Key</span><div class="relative"><KeyRound size={15} class="absolute left-3 top-3 body-muted" /><input class="input pl-9 pr-11" type={revealKey ? 'text' : 'password'} bind:value={apiKey} placeholder={draft.apiKeyRef ? '已安全保存；留空则保持不变' : '粘贴你的 API Key'} /><button class="absolute right-1.5 top-1.5 btn-ghost h-7 px-2" on:click={() => revealKey = !revealKey} aria-label="显示或隐藏密钥">{#if revealKey}<EyeOff size={14} />{:else}<Eye size={14} />{/if}</button></div><p class="mt-1.5 flex items-center gap-1.5 text-[11px] body-muted"><LockKeyhole size={12} />保存后进入系统密钥库，不写入 SQLite 或日志。</p></label>
            <div class="grid grid-cols-2 gap-4"><label><span class="label">模型</span><input class="input" bind:value={draft.model} /></label><label><span class="label">设为默认</span><select class="select" bind:value={draft.isDefault}><option value={true}>是</option><option value={false}>否</option></select></label></div>
            <details class="rounded-xl border" style="border-color: var(--line);"><summary class="cursor-pointer px-4 py-3 text-sm font-semibold">高级连接选项</summary><div class="border-t p-4" style="border-color: var(--line);"><label><span class="label">Base URL</span><input class="input" bind:value={draft.baseUrl} placeholder="https://api.example.com/v1" /></label><p class="mt-2 text-[11px] body-muted">请求将发送到 <code>{draft.baseUrl || 'Base URL'}/chat/completions</code></p></div></details>
            {#if result}<div class="flex items-start gap-3 rounded-xl border p-3" style={`border-color:${result.ok ? 'var(--success)' : 'var(--danger)'};background:${result.ok ? 'var(--brand-faint)' : 'var(--danger-soft)'}`}><svelte:component this={result.ok ? CheckCircle2 : XCircle} size={17} class={result.ok ? 'text-success' : 'text-danger'} /><div><p class="text-sm font-semibold">{result.message}</p><p class="mt-0.5 text-[11px] body-muted">延迟 {result.latencyMs} ms · 结构化输出 {result.structuredOutput ? '正常' : '未通过'}</p></div></div>{/if}
            <div class="flex justify-end gap-2"><button class="btn" disabled={testing} on:click={test}><TestTube2 size={15} />{testing ? '正在测试…' : '测试连接'}</button><button class="btn-primary" disabled={saving} on:click={saveProvider}><Save size={15} />{saving ? '正在保存…' : '保存配置'}</button></div>
          </div>
        </div>
      {/if}
    </div>
  </section>

  {#if localSettings}
    <section class="mt-6 grid grid-cols-2 gap-5">
      <article class="panel p-6"><div class="mb-5 flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl surface-soft"><Palette size={18} /></span><div><h3 class="section-title">外观与语言</h3><p class="mt-0.5 text-xs body-muted">跟随你的工作环境。</p></div></div><div class="space-y-4"><label><span class="label">界面语言</span><select class="select" bind:value={localSettings.locale}><option value="zh-CN">简体中文</option><option value="en">English</option></select></label><label><span class="label">主题</span><div class="grid grid-cols-3 gap-2">{#each themes as theme}<button class:selected={localSettings.theme === theme.value} class="theme-option flex h-11 items-center justify-center gap-2 rounded-xl border text-xs font-semibold" style="border-color: var(--line);" on:click={() => localSettings && (localSettings.theme = theme.value)}><svelte:component this={theme.icon} size={15} />{theme.label}</button>{/each}</div></label></div></article>
      <article id="privacy" class="panel p-6"><div class="mb-5 flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"><ShieldCheck size={18} /></span><div><h3 class="section-title">隐私与数据</h3><p class="mt-0.5 text-xs body-muted">默认关闭遥测并保存在本机。</p></div></div><div class="space-y-3"><div class="flex items-center justify-between rounded-xl border p-3" style="border-color: var(--line);"><div><p class="text-sm font-semibold">匿名遥测</p><p class="mt-0.5 text-[11px] body-muted">此版本固定关闭</p></div><span class="chip">已关闭</span></div><label class="flex cursor-pointer items-center justify-between rounded-xl border p-3" style="border-color: var(--line);"><div><p class="text-sm font-semibold">已了解模型数据范围</p><p class="mt-0.5 text-[11px] body-muted">简历与 JD 会发送到所选服务</p></div><input class="h-5 w-5 accent-[var(--brand)]" type="checkbox" bind:checked={localSettings.privacyAcknowledged} /></label></div></article>
    </section>

    <section class="mt-6 panel overflow-hidden"><div class="flex items-center justify-between border-b px-6 py-5" style="border-color: var(--line);"><div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl surface-soft"><TerminalSquare size={18} /></span><div><h3 class="section-title">高级模式</h3><p class="mt-0.5 text-xs body-muted">显示运行日志、原始 YAML 和内置 Skill。</p></div></div><label class="relative inline-flex cursor-pointer items-center"><input class="peer sr-only" type="checkbox" bind:checked={localSettings.advancedMode} /><span class="h-6 w-11 rounded-full bg-[var(--line-strong)] transition peer-checked:bg-brand after:absolute after:left-1 after:top-1 after:h-4 after:w-4 after:rounded-full after:bg-white after:transition peer-checked:after:translate-x-5"></span></label></div>{#if localSettings.advancedMode}<div class="p-6 animate-lift"><div class="mb-4 flex items-center justify-between"><div><h4 class="text-sm font-semibold">内置 Skills</h4><p class="mt-1 text-xs body-muted">内置版本保持只读；复制后可创建自定义版本。</p></div><span class="chip"><Code2 size={13} />5 个已启用</span></div><div class="divide-y rounded-xl border" style="border-color: var(--line);">{#each skills as skill}<div class="flex items-center gap-4 px-4 py-3"><span class="grid h-8 w-8 place-items-center rounded-lg bg-brand-soft text-brand"><PlugZap size={15} /></span><div class="min-w-0 flex-1"><p class="font-mono text-xs font-semibold">{skill[0]}</p><p class="mt-0.5 text-[11px] body-muted">{skill[2]}</p></div><span class="chip">v{skill[1]}</span><button class="btn-ghost h-8 text-xs">查看</button></div>{/each}</div></div>{/if}</section>
    <div class="mt-6 flex justify-end"><button class="btn-primary" on:click={updateSettings}><Save size={15} />保存偏好设置</button></div>
  {/if}
</div>

{#if toast}<div class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift">{toast}</div>{/if}

<style>
  .provider-row { background: var(--panel); }
  .provider-row:hover, .provider-row.selected { border-color: var(--brand) !important; }
  .provider-row.selected { box-shadow: 0 0 0 2px var(--focus); }
  .theme-option { background: var(--panel); }
  .theme-option.selected { border-color: var(--brand) !important; color: var(--brand); background: var(--brand-faint); }
</style>
