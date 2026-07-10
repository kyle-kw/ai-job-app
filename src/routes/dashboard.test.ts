import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import Dashboard from './+page.svelte';
import { mockSnapshot } from '$lib/mock-data';
import { loading, snapshot } from '$lib/stores/app';

describe('dashboard', () => {
  beforeEach(() => {
    snapshot.set(structuredClone(mockSnapshot));
    loading.set(false);
  });

  it('keeps the main workflow visible without technical controls', () => {
    render(Dashboard);
    expect(screen.getByText('两项准备，抓取时自动连接 BOSS')).toBeInTheDocument();
    expect(screen.getByText('已合并到“开始抓取”')).toBeInTheDocument();
    expect(screen.getByText('开始一轮岗位搜索')).toBeInTheDocument();
    expect(screen.getByText('与你最接近的机会')).toBeInTheDocument();
    expect(screen.queryByText('运行日志')).not.toBeInTheDocument();
  });
});
