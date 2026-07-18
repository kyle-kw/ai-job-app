<script lang="ts">
  import { BarChart3, BriefcaseBusiness, FileText, Home, Settings2, Sparkles } from 'lucide-svelte';
  import { page } from '$app/stores';

  const nav = [
    { href: '/', label: '首页', icon: Home },
    { href: '/jobs', label: '岗位', icon: BriefcaseBusiness },
    { href: '/reports', label: '数据报告', icon: BarChart3 },
    { href: '/resume', label: '简历', icon: FileText }
  ];

  const active = (href: string) =>
    href === '/' ? $page.url.pathname === '/' : $page.url.pathname.startsWith(href);
</script>

<aside
  class="sidebar flex w-[224px] shrink-0 flex-col border-r px-3 py-4"
  style="background: var(--sidebar); border-color: var(--line);"
>
  <a href="/" class="mb-7 flex items-center gap-3 px-2.5" aria-label="求职舱首页">
    <span
      class="grid h-10 w-10 place-items-center rounded-[14px] text-white shadow-sm"
      style="background: var(--brand);"
    >
      <Sparkles size={20} strokeWidth={2.2} />
    </span>
    <span class="brand-copy">
      <span class="block text-[17px] font-semibold tracking-[-0.02em]">求职舱</span>
      <span class="block text-[11px] body-muted">AI Job Companion</span>
    </span>
  </a>

  <nav class="space-y-1" aria-label="主导航">
    {#each nav as item}
      <a
        href={item.href}
        aria-label={item.label}
        aria-current={active(item.href) ? 'page' : undefined}
        title={item.label}
        class:active={active(item.href)}
        class="nav-item flex h-11 items-center gap-3 rounded-xl px-3 text-sm font-medium transition"
      >
        <svelte:component this={item.icon} size={18} strokeWidth={active(item.href) ? 2.25 : 1.8} />
        <span class="nav-copy">{item.label}</span>
      </a>
    {/each}
  </nav>

  <div class="mt-auto">
    <div
      class="local-note mx-2 mb-3 rounded-xl border p-3"
      style="border-color: var(--line); background: var(--brand-faint);"
    >
      <div class="mb-1 flex items-center gap-2 text-xs font-semibold" style="color: var(--brand);">
        <span class="h-1.5 w-1.5 rounded-full" style="background: var(--brand);"></span>
        本地优先
      </div>
      <p class="text-[11px] leading-5 body-muted">
        岗位与简历保存在本机，仅主动使用 AI 时发送必要上下文。
      </p>
    </div>
    <a
      href="/settings"
      aria-label="设置"
      aria-current={active('/settings') ? 'page' : undefined}
      title="设置"
      class:active={active('/settings')}
      class="nav-item flex h-11 items-center gap-3 rounded-xl px-3 text-sm font-medium transition"
    >
      <Settings2 size={18} strokeWidth={active('/settings') ? 2.25 : 1.8} />
      <span class="nav-copy">设置</span>
    </a>
  </div>
</aside>

<style>
  .nav-item {
    color: var(--muted);
  }
  .nav-item:hover {
    background: color-mix(in srgb, var(--panel) 62%, transparent);
    color: var(--ink);
  }
  .nav-item.active {
    background: var(--panel);
    color: var(--brand);
    box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04);
  }
  @media (max-width: 1180px) {
    .sidebar {
      width: 80px;
      padding-left: 10px;
      padding-right: 10px;
    }
    .brand-copy,
    .nav-copy,
    .local-note {
      display: none;
    }
    .sidebar > a {
      justify-content: center;
      padding-left: 0;
      padding-right: 0;
    }
    .nav-item {
      justify-content: center;
      padding-left: 0;
      padding-right: 0;
    }
  }
</style>
