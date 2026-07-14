<script lang="ts">
  import { ShieldCheck } from 'lucide-svelte';

  export let accepting = false;
  export let onAccept: () => void;
  export let onExit: () => void;
</script>

<div class="fixed inset-0 z-[100] grid place-items-center bg-[var(--app-bg)] p-4" role="dialog" aria-modal="true" aria-labelledby="privacy-title">
  <section class="panel flex max-h-[calc(100vh-2rem)] w-full max-w-[760px] flex-col overflow-hidden shadow-2xl">
    <header class="border-b px-7 py-6" style="border-color: var(--line);">
      <div class="flex items-center gap-3">
        <span class="grid h-11 w-11 place-items-center rounded-xl bg-brand-soft text-brand"><ShieldCheck size={21} /></span>
        <div><p class="eyebrow">PRIVACY · 2026-07-14</p><h1 id="privacy-title" class="mt-1 text-xl font-semibold">隐私与使用说明</h1></div>
      </div>
      <p class="mt-4 text-sm leading-6 body-muted">请完整阅读后再继续。确认前，求职舱不会检查更新、访问 BOSS 直聘、测试模型或发起其他网络请求。</p>
    </header>

    <div class="scrollbar-thin overflow-y-auto px-7 py-6 text-sm leading-7">
      <h2 class="font-semibold">1. 本地数据</h2>
      <p class="body-muted">简历、岗位、分析结果、配置和运行记录默认保存在你的设备中。普通卸载不会主动删除这些数据；彻底卸载前请先在设置中执行“清除全部数据”。</p>

      <h2 class="mt-5 font-semibold">2. 何时访问网络</h2>
      <p class="body-muted">只有你使用对应功能时才会联网：BOSS 功能访问 zhipin.com；模型功能把完成当前请求所需的内容发送到你配置的模型服务；更新功能访问 GitHub Releases。应用不内置共享 API Key。</p>

      <h2 class="mt-5 font-semibold">3. 敏感信息</h2>
      <p class="body-muted">模型 API Key 保存在系统钥匙串中。BOSS 登录状态保存在独立的 Chrome Profile 中。日志不得记录 API Key、Cookie、完整 Prompt、简历正文或岗位正文。</p>

      <h2 class="mt-5 font-semibold">4. 备份与诊断</h2>
      <p class="body-muted">你导出的 <code>.aijobbackup</code> 是未加密的 SQLite 备份，包含简历和岗位数据，但不包含 API Key 或 BOSS Cookie，请像保护简历文件一样妥善保管。诊断 ZIP 只包含脱敏日志和环境清单，不包含数据库、简历、岗位、密钥、Cookie 或用户名路径。</p>

      <h2 class="mt-5 font-semibold">5. 遥测与崩溃报告</h2>
      <p class="body-muted">求职舱不收集遥测，不自动上传崩溃报告。你可以自主决定是否通过 GitHub Issues 提交问题；请勿上传简历、密钥或 Cookie。</p>

      <h2 class="mt-5 font-semibold">6. 风险与责任</h2>
      <p class="body-muted">本软件按 MIT 许可提供，不保证岗位信息、AI 输出或自动化流程完全准确。请在发送简历、联系招聘方或作出求职决定前自行核验。使用 BOSS 功能时，你仍需遵守相关网站规则与适用法律。</p>

      <p class="mt-5 rounded-xl border p-4 text-xs leading-6 body-muted" style="border-color: var(--line); background: var(--panel-soft);">完整政策随应用离线提供于 <strong>PRIVACY.md</strong> 与 <strong>TERMS.md</strong>。政策发生实质变化时，应用会提升确认版本并再次征得你的同意。</p>
    </div>

    <footer class="flex justify-end gap-3 border-t px-7 py-5" style="border-color: var(--line);">
      <button class="btn" type="button" on:click={onExit} disabled={accepting}>退出应用</button>
      <button class="btn-primary" type="button" on:click={onAccept} disabled={accepting}>{accepting ? '正在保存…' : '同意并继续'}</button>
    </footer>
  </section>
</div>
