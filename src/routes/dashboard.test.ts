import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import Dashboard from './+page.svelte';
import { mockSnapshot } from '$lib/mock-data';
import { loading, snapshot } from '$lib/stores/app';

describe('dashboard', () => {
  afterEach(cleanup);

  beforeEach(() => {
    const state = structuredClone(mockSnapshot);
    state.readiness.ai = false;
    state.readiness.boss = false;
    snapshot.set(state);
    loading.set(false);
  });

  it('keeps setup guidance visible without locking the rest of the app', () => {
    render(Dashboard);
    expect(screen.getByText('先完成两项必要配置')).toBeInTheDocument();
    expect(screen.getByText('登录 BOSS 直聘')).toBeInTheDocument();
    expect(screen.getByText('配置默认模型')).toBeInTheDocument();
    expect(screen.getByText('打开主简历')).toBeInTheDocument();
    expect(screen.getByText('配置未完成时，其他页面仍然可以正常查看。', { exact: false })).toBeInTheDocument();
    expect(screen.queryByText('开始一轮岗位搜索')).not.toBeInTheDocument();
    expect(screen.queryByText('与你最接近的机会')).not.toBeInTheDocument();
    expect(screen.queryByText('岗位市场观察')).not.toBeInTheDocument();
    expect(screen.queryByText('OpenRouter 免费路由')).not.toBeInTheDocument();
  });

  it('offers reconfiguration after both required services are ready', () => {
    const state = structuredClone(mockSnapshot);
    state.readiness.ai = true;
    state.readiness.boss = true;
    state.configuration.boss.state = 'ready';
    state.configuration.llm.state = 'ready';
    state.providers = state.providers.map((provider) => provider.isDefault ? { ...provider, verified: true } : provider);
    snapshot.set(state);

    render(Dashboard);
    expect(screen.getByText('BOSS 登录配置已完成')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '重新配置 BOSS' })).toBeInTheDocument();
    expect(screen.getByText('小米 MiMo 已验证', { exact: false })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '管理模型配置' })).toHaveAttribute('href', '/settings');
  });
});
