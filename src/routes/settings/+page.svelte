<script lang="ts">
  import { onMount } from 'svelte';
  import { open, save, confirm } from '@tauri-apps/plugin-dialog';
  import { Bot, Check, CheckCircle2, ChevronRight, CircleHelp, Code2, DatabaseBackup, Download, Eye, EyeOff, HardDrive, KeyRound, LockKeyhole, PlugZap, RefreshCw, RotateCcw, Save, ShieldCheck, Sparkles, TerminalSquare, TestTube2, Trash2, XCircle } from 'lucide-svelte';
  import { backend } from '$lib/services/backend';
  import BossSetupCard from '$lib/components/BossSetupCard.svelte';
  import ClearDataDialog from '$lib/components/ClearDataDialog.svelte';
  import { formatLocalDateTime } from '$lib/date-time';
  import { availableProviderConfigs } from '$lib/provider-policy';
  import { shouldReloadAfterClear } from '$lib/clear-data';
  import { checkForUpdate, updateCheckError, updateChecking } from '$lib/stores/distribution';
  import { refresh, saveSettings, snapshot } from '$lib/stores/app';
  import type { AiProviderConfig, AppInfo, AppSettings, BackupInfo, ClearDataResult, ClearDataScope, ProviderTestResult } from '$lib/types';

  let selectedId = '';
  let draft: AiProviderConfig | null = null;
  let apiKey = '';
  let revealKey = false;
  let testing = false;
  let saving = false;
  let result: ProviderTestResult | null = null;
  let resultMode: 'test' | 'saved' | null = null;
  let errorMessage = '';
  let toast = '';
  let localSettings: AppSettings | null = null;
  let settingsInitialized = false;
  let settingsSaving: 'automaticUpdateChecks' | 'advancedMode' | null = null;
  let appInfo: AppInfo | null = null;
  let automaticBackups: BackupInfo[] = [];
  let maintenanceError = '';
  let maintenanceBusy = false;
  let clearResult: ClearDataResult | null = null;
  let clearConfirmation: ClearDataScope | null = null;

  onMount(() => {
    void loadDistributionInfo();
  });

  async function loadDistributionInfo() {
    try {
      [appInfo, automaticBackups] = await Promise.all([
        backend.getAppInfo(),
        backend.listAutomaticBackups()
      ]);
    } catch (error) {
      maintenanceError = error instanceof Error ? error.message : String(error);
    }
  }

  $: visibleProviders = availableProviderConfigs($snapshot.providers);
  $: if (visibleProviders.length && (!selectedId || !visibleProviders.some((provider) => provider.id === selectedId))) {
    selectProvider(visibleProviders.find((provider) => provider.isDefault)?.id ?? visibleProviders.find((provider) => provider.kind === 'xiaomi')?.id ?? visibleProviders[0].id);
  }
  $: if (!settingsInitialized && $snapshot.settings) {
    localSettings = structuredClone($snapshot.settings);
    settingsInitialized = true;
  }

  function selectProvider(id: string) {
    selectedId = id;
    const found = visibleProviders.find((provider) => provider.id === id);
    draft = found ? structuredClone(found) : null;
    apiKey = '';
    result = null;
    resultMode = null;
    errorMessage = '';
  }

  function showToast(message: string) {
    toast = message;
    window.setTimeout(() => toast === message && (toast = ''), 2400);
  }

  function syncDraft() {
    const saved = availableProviderConfigs($snapshot.providers).find((provider) => provider.id === selectedId);
    if (saved) draft = structuredClone(saved);
  }

  async function test() {
    if (!draft) return;
    testing = true;
    result = null;
    resultMode = null;
    errorMessage = '';
    try {
      result = await backend.testProvider({ ...draft, apiKey: apiKey || undefined });
      resultMode = 'test';
      if (result.ok) {
        showToast('连接测试通过，配置尚未保存');
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
    result = null;
    resultMode = null;
    errorMessage = '';
    try {
      const saved = await backend.saveProvider({ ...draft, apiKey: apiKey || undefined });
      result = saved.testResult;
      resultMode = 'saved';
      await refresh();
      syncDraft();
      apiKey = '';
      showToast('模型配置已验证并保存');
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      saving = false;
    }
  }

  async function persistBooleanSetting(
    key: 'automaticUpdateChecks' | 'advancedMode',
    value: boolean
  ) {
    if (!localSettings || settingsSaving) return;
    const previous = localSettings;
    const next = { ...localSettings, [key]: value };
    localSettings = next;
    settingsSaving = key;
    try {
      await saveSettings(next);
      showToast('设置已自动保存');
    } catch (error) {
      localSettings = previous;
      const message = error instanceof Error ? error.message : String(error);
      showToast(`自动保存失败：${message}`);
    } finally {
      settingsSaving = null;
    }
  }

  const formatBytes = (bytes: number) => bytes < 1024 * 1024
    ? `${Math.max(1, Math.round(bytes / 1024))} KiB`
    : `${(bytes / 1024 / 1024).toFixed(1)} MiB`;

  async function manualUpdateCheck() {
    maintenanceError = '';
    const update = await checkForUpdate(true);
    if (!$updateCheckError) {
      await loadDistributionInfo();
      if (!update) showToast('当前已是最新版本');
    }
  }

  async function exportBackup() {
    const accepted = await confirm('备份包含简历和岗位数据，且不加密；不包含 API Key 或 BOSS Cookie。请妥善保管。', { title: '导出明文备份', kind: 'warning' });
    if (!accepted) return;
    const output = await save({ defaultPath: `求职舱-${new Date().toISOString().slice(0, 10)}.aijobbackup`, filters: [{ name: '求职舱备份', extensions: ['aijobbackup'] }] });
    if (!output) return;
    maintenanceBusy = true;
    try {
      await backend.createBackup(output);
      showToast('备份已导出');
    } catch (error) {
      maintenanceError = error instanceof Error ? error.message : String(error);
    } finally {
      maintenanceBusy = false;
    }
  }

  async function restoreBackup() {
    const selected = await open({ multiple: false, filters: [{ name: '求职舱备份', extensions: ['aijobbackup'] }] });
    if (!selected || Array.isArray(selected)) return;
    const accepted = await confirm('恢复前会自动保存当前数据快照。恢复成功后应用将重启。', { title: '恢复备份', kind: 'warning' });
    if (!accepted) return;
    maintenanceBusy = true;
    try {
      await backend.restoreBackup(selected);
      await backend.restartApp();
    } catch (error) {
      maintenanceError = error instanceof Error ? error.message : String(error);
      maintenanceBusy = false;
    }
  }

  async function exportDiagnostics() {
    const output = await save({ defaultPath: `求职舱-诊断-${new Date().toISOString().slice(0, 10)}.zip`, filters: [{ name: '诊断 ZIP', extensions: ['zip'] }] });
    if (!output) return;
    maintenanceBusy = true;
    try {
      await backend.exportDiagnostics(output);
      showToast('脱敏诊断已导出');
    } catch (error) {
      maintenanceError = error instanceof Error ? error.message : String(error);
    } finally {
      maintenanceBusy = false;
    }
  }

  async function openIssueSupport() {
    maintenanceError = '';
    try {
      await backend.openGitHubIssues();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      maintenanceError = `无法打开 GitHub Issues：${message}`;
    }
  }

  function requestClearData(scope: ClearDataScope) {
    if (maintenanceBusy) return;
    maintenanceError = '';
    clearConfirmation = scope;
  }

  function closeClearConfirmation() {
    if (!maintenanceBusy) clearConfirmation = null;
  }

  async function confirmClearData() {
    const scope = clearConfirmation;
    if (!scope) return;
    maintenanceBusy = true;
    maintenanceError = '';
    clearResult = null;
    try {
      clearResult = await backend.clearData(scope);
      clearConfirmation = null;
      if (shouldReloadAfterClear(clearResult)) {
        await refresh();
        await loadDistributionInfo();
      }
    } catch (error) {
      maintenanceError = error instanceof Error ? error.message : String(error);
    } finally {
      maintenanceBusy = false;
    }
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
    <h2 class="page-title mt-1">连接、模型与高级能力</h2>
    <p class="mt-1 text-sm body-muted">管理 BOSS 专用浏览器、默认模型和本地数据；界面固定为简体中文并跟随系统主题。</p>
  </div>

  <div id="boss" class="mb-6 scroll-mt-24"><BossSetupCard eyebrow="浏览器连接" title="BOSS 专用浏览器" /></div>

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
            <label><span class="label">Base URL</span><input class="input" bind:value={draft.baseUrl} placeholder="https://api.example.com/v1" /><p class="mt-1.5 text-[11px] body-muted">请求发送到 <code>{draft.baseUrl || 'Base URL'}/chat/completions</code></p></label>
            {#if draft.baseUrl.trim().toLowerCase().startsWith('http://')}
              <label class="flex items-start gap-3 rounded-xl border p-3 text-xs leading-5 text-warning" style="border-color: var(--warning); background: var(--warning-soft);">
                <input class="mt-1 h-4 w-4" type="checkbox" bind:checked={draft.allowInsecureHttp} />
                <span><strong>允许不安全 HTTP</strong><br />API Key 和请求内容将以明文发送到该地址。仅在你信任此服务和网络时启用。</span>
              </label>
            {/if}
            <div class="grid grid-cols-2 gap-4"><label><span class="label">模型</span><input class="input" bind:value={draft.model} placeholder="模型名称" /></label><label><span class="label">设为默认</span><select class="select" bind:value={draft.isDefault}><option value={true}>是</option><option value={false}>否</option></select></label></div>

            {#if result}<div class="flex items-start gap-3 rounded-xl border p-3" style={`border-color:${result.ok ? 'var(--success)' : 'var(--danger)'}; background:${result.ok ? 'var(--brand-faint)' : 'var(--danger-soft)'}`}><svelte:component this={result.ok ? CheckCircle2 : XCircle} size={17} class={result.ok ? 'text-success' : 'text-danger'} /><div><p class="text-sm font-semibold">{result.message}</p><p class="mt-0.5 text-[11px] body-muted">延迟 {result.latencyMs} ms · 结构化输出 {result.structuredOutput ? '正常' : '未通过'} · {result.visionMessage}</p>{#if result.ok && resultMode === 'test'}<p class="mt-1 text-[11px] text-warning">本次仅测试连接；点击“验证并保存”后配置才会生效。</p>{:else if result.ok && resultMode === 'saved'}<p class="mt-1 text-[11px] text-success">配置已保存并生效。</p>{/if}</div></div>{/if}
            {#if errorMessage}<div class="rounded-xl border p-3 text-xs leading-5 text-danger" style="border-color: var(--danger); background: var(--danger-soft);">{errorMessage}</div>{/if}

            <div class="flex justify-end gap-2"><button class="btn" disabled={testing || saving || !draft.baseUrl || !draft.model || (!apiKey && !draft.apiKeyRef) || (draft.baseUrl.trim().toLowerCase().startsWith('http://') && !draft.allowInsecureHttp)} on:click={test}><TestTube2 size={15} />{testing ? '正在测试…' : '测试连接'}</button><button class="btn-primary" disabled={saving || testing || !draft.baseUrl || !draft.model || (!apiKey && !draft.apiKeyRef) || (draft.baseUrl.trim().toLowerCase().startsWith('http://') && !draft.allowInsecureHttp)} on:click={saveProvider}><Save size={15} />{saving ? '正在验证并保存…' : '验证并保存'}</button></div>
          </div>
        </div>
      {/if}
    </div>
  </section>

  {#if localSettings}
    <section class="mt-6 panel overflow-hidden">
      <div class="flex items-center justify-between gap-4 px-6 py-5">
        <div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"><RefreshCw size={18} /></span><div><h3 class="section-title">自动检查更新</h3><p class="mt-0.5 text-xs body-muted">开启后，隐私确认完成且距上次成功检查满 24 小时时访问 GitHub Releases。应用不会自动下载或安装更新。</p></div></div>
        <div class="flex shrink-0 items-center gap-3">
          {#if settingsSaving === 'automaticUpdateChecks'}<span class="text-xs body-muted" aria-live="polite">保存中…</span>{/if}
          <label class="relative inline-flex cursor-pointer items-center"><input class="peer sr-only" type="checkbox" aria-label="自动检查更新" checked={localSettings.automaticUpdateChecks} disabled={settingsSaving !== null} on:change={(event) => void persistBooleanSetting('automaticUpdateChecks', event.currentTarget.checked)} /><span class="h-6 w-11 rounded-full bg-[var(--line-strong)] transition peer-checked:bg-brand after:absolute after:left-1 after:top-1 after:h-4 after:w-4 after:rounded-full after:bg-white after:transition peer-checked:after:translate-x-5"></span></label>
        </div>
      </div>
      {#if !localSettings.automaticUpdateChecks}<div class="border-t px-6 py-3 text-xs text-warning" style="border-color: var(--line); background: var(--warning-soft);">自动检查已关闭。你仍可在“关于与诊断”中手动检查更新。</div>{/if}
    </section>
    <section class="settings-dynamic-panel mt-6 panel overflow-hidden">
      <div class="flex items-center justify-between border-b px-6 py-5" style="border-color: var(--line);"><div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl surface-soft"><TerminalSquare size={18} /></span><div><h3 class="section-title">高级模式</h3><p class="mt-0.5 text-xs body-muted">显示运行日志、原始 YAML 和内置 Skill；修改后自动保存。</p></div></div><div class="flex shrink-0 items-center gap-3">{#if settingsSaving === 'advancedMode'}<span class="text-xs body-muted" aria-live="polite">保存中…</span>{/if}<label class="relative inline-flex cursor-pointer items-center"><input class="peer sr-only" type="checkbox" aria-label="高级模式" checked={localSettings.advancedMode} disabled={settingsSaving !== null} on:change={(event) => void persistBooleanSetting('advancedMode', event.currentTarget.checked)} /><span class="h-6 w-11 rounded-full bg-[var(--line-strong)] transition peer-checked:bg-brand after:absolute after:left-1 after:top-1 after:h-4 after:w-4 after:rounded-full after:bg-white after:transition peer-checked:after:translate-x-5"></span></label></div></div>
      {#if localSettings.advancedMode}<div class="settings-expand-content p-6"><div class="mb-4 flex items-center justify-between"><div><h4 class="text-sm font-semibold">内置 Skills</h4><p class="mt-1 text-xs body-muted">内置版本保持只读；仅展示当前产品仍在使用的能力。</p></div><span class="chip"><Code2 size={13} />{skills.length} 个已启用</span></div><div class="divide-y rounded-xl border" style="border-color: var(--line);">{#each skills as skill}<div class="flex items-center gap-4 px-4 py-3"><span class="grid h-8 w-8 place-items-center rounded-lg bg-brand-soft text-brand"><PlugZap size={15} /></span><div class="min-w-0 flex-1"><p class="font-mono text-xs font-semibold">{skill[0]}</p><p class="mt-0.5 text-[11px] body-muted">{skill[2]}</p></div><span class="chip">v{skill[1]}</span></div>{/each}</div></div>{/if}
    </section>
  {/if}
</div>

<div class="page-content max-w-[1240px] pt-0">
  <section class="panel overflow-hidden">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b px-6 py-5" style="border-color: var(--line);">
      <div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-brand-soft text-brand"><HardDrive size={18} /></span><div><h3 class="section-title">关于与诊断</h3><p class="mt-0.5 text-xs body-muted">版本、运行环境、更新状态和可安全分享的诊断信息。</p></div></div>
      <button class="btn" type="button" on:click={manualUpdateCheck} disabled={$updateChecking}><RefreshCw size={15} class={$updateChecking ? 'animate-spin' : ''} />{$updateChecking ? '正在检查…' : '检查更新'}</button>
    </div>
    {#if appInfo}
      <dl class="grid gap-x-10 gap-y-5 px-6 py-5 text-sm md:grid-cols-2">
        <div><dt class="body-muted">应用 / sidecar</dt><dd class="mt-1 font-medium">v{appInfo.version} / protocol {appInfo.sidecarProtocol}</dd></div>
        <div><dt class="body-muted">系统 / 架构 / WebView</dt><dd class="mt-1 font-medium">{appInfo.os} · {appInfo.arch} · {appInfo.webview}</dd></div>
        <div><dt class="body-muted">Google Chrome</dt><dd class="mt-1 font-medium">{appInfo.chrome.installed ? appInfo.chrome.version || '已安装' : '未安装（BOSS 功能已禁用）'}</dd></div>
        <div><dt class="body-muted">最后更新检查</dt><dd class="mt-1 font-medium">{formatLocalDateTime(appInfo.lastUpdateCheckAt) || '尚未检查'}</dd></div>
        <div class="md:col-span-2"><dt class="body-muted">数据目录</dt><dd class="mt-1.5 rounded-lg border px-3 py-2" style="border-color: var(--line); background: var(--panel-soft);"><code class="break-all text-xs">{appInfo.dataDir}</code></dd></div>
      </dl>
      {#if !appInfo.chrome.installed}<div class="mx-6 mb-5 rounded-xl border p-3 text-xs text-warning" style="border-color: var(--warning); background: var(--warning-soft);">BOSS 功能需要 Google Chrome，应用不会替你下载浏览器。<a class="ml-1 underline" href="https://www.google.com/chrome/" target="_blank" rel="noreferrer">前往 Chrome 官方网站</a></div>{/if}
    {/if}
    <div class="flex flex-wrap gap-2 border-t px-6 py-4" style="border-color: var(--line);"><button class="btn" type="button" on:click={exportDiagnostics} disabled={maintenanceBusy}><Download size={15} />导出脱敏诊断</button><button class="btn" type="button" on:click={openIssueSupport}>GitHub Issues 支持</button></div>
  </section>

  <section class="mt-6 panel overflow-hidden">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b px-6 py-5" style="border-color: var(--line);"><div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl surface-soft"><DatabaseBackup size={18} /></span><div><h3 class="section-title">备份与恢复</h3><p class="mt-0.5 text-xs body-muted">导出备份为明文文件；自动备份最多保留最近 3 份。</p></div></div><div class="flex gap-2"><button class="btn" type="button" on:click={restoreBackup} disabled={maintenanceBusy}><RotateCcw size={15} />恢复备份</button><button class="btn-primary" type="button" on:click={exportBackup} disabled={maintenanceBusy}><Download size={15} />导出备份</button></div></div>
    <div class="px-6 py-5">
      <div class="rounded-xl border p-4 text-xs leading-6 text-warning" style="border-color: var(--warning); background: var(--warning-soft);">.aijobbackup 包含简历和岗位数据且不加密，不包含 API Key 或 BOSS Cookie。恢复会先校验完整性和 schema，并在覆盖前保存当前快照。</div>
      <h4 class="mt-5 text-sm font-semibold">自动备份</h4>
      {#if automaticBackups.length}
        <div class="mt-2 divide-y rounded-xl border text-xs" style="border-color: var(--line);">{#each automaticBackups as backup}<div class="flex flex-wrap items-center justify-between gap-2 px-4 py-3"><div><p class="font-medium">{backup.fileName}</p><p class="mt-1 body-muted">{new Date(backup.createdAt).toLocaleString()} · {formatBytes(backup.size)}</p></div><code class="max-w-[48%] truncate body-muted" title={backup.path}>{backup.path}</code></div>{/each}</div>
      {:else}<p class="mt-2 text-xs body-muted">尚无自动备份。更新、恢复或 schema 迁移前会自动创建。</p>{/if}
    </div>
  </section>

  <section class="mt-6 panel overflow-hidden">
    <div class="border-b px-6 py-5" style="border-color: var(--line);"><div class="flex items-center gap-3"><span class="grid h-10 w-10 place-items-center rounded-xl bg-[var(--danger-soft)] text-danger"><Trash2 size={18} /></span><div><h3 class="section-title">数据生命周期</h3><p class="mt-0.5 text-xs body-muted">普通卸载保留用户数据；彻底卸载前请在此清除全部数据。</p></div></div></div>
    <div class="grid gap-3 px-6 py-5 sm:grid-cols-2 lg:grid-cols-4">
      <button class="btn justify-center" type="button" on:click={() => requestClearData('modelKeys')} disabled={maintenanceBusy}>清除模型密钥</button>
      <button class="btn justify-center" type="button" on:click={() => requestClearData('bossProfile')} disabled={maintenanceBusy}>清除 BOSS 数据</button>
      <button class="btn justify-center" type="button" on:click={() => requestClearData('legacyData')} disabled={maintenanceBusy || !appInfo?.legacyDataDetected}>删除旧版遗留</button>
      <button class="btn justify-center text-danger" type="button" on:click={() => requestClearData('all')} disabled={maintenanceBusy}>清除全部数据</button>
    </div>
    {#if clearResult}<div class="mx-6 mb-5 rounded-xl border p-4 text-xs" style="border-color: var(--line);"><p class="font-semibold">{clearResult.complete ? '清理完成' : '部分项目清理失败'}</p><ul class="mt-2 space-y-1">{#each clearResult.items as item}<li class={item.ok ? 'text-success' : 'text-danger'}>{item.ok ? '✓' : '✕'} {item.message}</li>{/each}</ul>{#if clearResult.restartRequired}<button class="btn mt-3" type="button" on:click={() => void backend.restartApp()}>重启并重新初始化</button>{/if}</div>{/if}
  </section>

  {#if maintenanceError || $updateCheckError}<div class="mt-5 rounded-xl border p-3 text-xs text-danger" style="border-color: var(--danger); background: var(--danger-soft);">{maintenanceError || $updateCheckError}</div>{/if}
</div>

<ClearDataDialog
  open={Boolean(clearConfirmation)}
  scope={clearConfirmation ?? 'modelKeys'}
  busy={maintenanceBusy}
  error={maintenanceError}
  onCancel={closeClearConfirmation}
  onConfirm={confirmClearData}
/>

{#if toast}<div class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-xl bg-[#1d2824] px-4 py-2.5 text-sm font-medium text-white shadow-xl animate-lift">{toast}</div>{/if}

<style>
  .provider-row { background: var(--panel); }
  .provider-row:hover, .provider-row.selected { border-color: var(--brand) !important; }
  .provider-row.selected { box-shadow: 0 0 0 2px var(--focus); }
  .settings-dynamic-panel { overflow-anchor: none; }
  .settings-expand-content { display: flow-root; contain: layout paint; }
  @media (max-width: 980px) { .provider-layout { grid-template-columns: minmax(0, 1fr); } .provider-layout > :first-child { border-right: 0; border-bottom: 1px solid var(--line); } }
</style>
