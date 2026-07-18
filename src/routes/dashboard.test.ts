import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import Dashboard from './+page.svelte';
import { backend } from '$lib/services/backend';
import { mockSnapshot } from '$lib/mock-data';
import { loading, snapshot } from '$lib/stores/app';

function readyState() {
  const state = structuredClone(mockSnapshot);
  state.readiness.ai = true;
  state.readiness.boss = true;
  state.configuration.boss.state = 'ready';
  state.configuration.llm.state = 'ready';
  state.providers = state.providers.map((provider) =>
    provider.isDefault ? { ...provider, verified: true } : provider
  );
  return state;
}

describe('dashboard', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    const state = structuredClone(mockSnapshot);
    state.readiness.ai = false;
    state.readiness.boss = false;
    snapshot.set(state);
    loading.set(false);
  });

  it('shows setup guidance until BOSS and the default model are ready', () => {
    render(Dashboard);
    expect(screen.getByText('先完成两项必要配置')).toBeInTheDocument();
    expect(screen.getByText('登录 BOSS 直聘')).toBeInTheDocument();
    expect(screen.getByText('配置默认模型')).toBeInTheDocument();
    expect(
      screen.getByText('配置未完成时，其他页面仍然可以正常查看。', { exact: false })
    ).toBeInTheDocument();
    expect(screen.queryByText('求职工作台')).not.toBeInTheDocument();
  });

  it('switches to the workbench and loads counts, matches, and the latest successful report', async () => {
    const state = readyState();
    state.scrapeRuns = [
      {
        ...state.scrapeRuns[0],
        id: 'failed',
        keyword: '失败任务',
        totalSeen: 0,
        completedAt: null
      },
      {
        ...state.scrapeRuns[0],
        id: 'successful',
        keyword: '数据分析',
        totalSeen: 42,
        reportMarkdown: '## 数据分析观察\n\n- Python 需求最多。'
      }
    ];
    snapshot.set(state);
    render(Dashboard);

    expect(screen.getByText('求职工作台')).toBeInTheDocument();
    expect(screen.queryByText('先完成两项必要配置')).not.toBeInTheDocument();
    const totalCard = screen.getByText('累计岗位').closest('article');
    await waitFor(() => expect(within(totalCard!).getByText('5')).toBeInTheDocument());
    expect(screen.getByRole('heading', { name: '与你最接近的机会' })).toBeInTheDocument();
    expect(await screen.findByText('AI Agent 开发工程师')).toBeInTheDocument();
    expect(screen.getByText(/最近抓取 · 数据分析/)).toBeInTheDocument();
    expect(screen.getByText('本次岗位样本观察')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: '数据分析观察' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '管理连接' })).toHaveAttribute(
      'href',
      '/settings#boss'
    );
  });

  it('falls back to recent jobs when no resume exists', async () => {
    const state = readyState();
    state.resume = null;
    state.readiness.resume = false;
    snapshot.set(state);
    render(Dashboard);

    expect(screen.getByRole('heading', { name: '最近岗位' })).toBeInTheDocument();
    expect(await screen.findByText('AI Agent 开发工程师')).toBeInTheDocument();
    expect(screen.queryByLabelText(/匹配度/)).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: '创建主简历' })).toHaveAttribute('href', '/resume');
  });

  it('shows an empty state when the local job library has no jobs', async () => {
    snapshot.set(readyState());
    vi.spyOn(backend, 'listJobsPage').mockResolvedValue({
      items: [],
      total: 0,
      pendingDetailCount: 0,
      nextCursor: null
    });
    render(Dashboard);

    expect(await screen.findByText('岗位库还是空的')).toBeInTheDocument();
  });

  it('restores the same persisted search controls from the dashboard', async () => {
    const state = readyState();
    state.lastSearchSpec = {
      keyword: '财务分析',
      city: '北京',
      pages: 3,
      experience: '106',
      salary: '406',
      degree: '',
      companyScale: '304'
    };
    snapshot.set(state);
    render(Dashboard);

    await fireEvent.click(screen.getByRole('button', { name: '设置搜索条件' }));

    expect(screen.getByLabelText('关键词')).toHaveValue('财务分析');
    expect(screen.getByLabelText('城市')).toHaveValue('北京');
    expect(screen.getByLabelText('抓取页数')).toHaveValue('3');
    expect(screen.getByLabelText('经验要求')).toHaveValue('106');
    expect(screen.getByLabelText('薪资范围')).toHaveValue('406');
    expect(screen.getByLabelText('公司规模')).toHaveValue('304');
  });

  it('shows a retryable dashboard error and refreshes after a scrape finishes', async () => {
    snapshot.set(readyState());
    const listJobs = vi.spyOn(backend, 'listJobsPage');
    listJobs
      .mockRejectedValueOnce(new Error('数据库暂时不可用'))
      .mockRejectedValueOnce(new Error('数据库暂时不可用'));
    render(Dashboard);
    expect(await screen.findByText(/看板数据加载失败：数据库暂时不可用/)).toBeInTheDocument();

    listJobs.mockResolvedValue({ items: [], total: 0, pendingDetailCount: 0, nextCursor: null });
    snapshot.update((state) => ({
      ...state,
      tasks: [
        {
          id: 'scrape-done',
          kind: 'scrape',
          title: '抓取完成',
          state: 'completed',
          progress: 100,
          message: '抓取完成',
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          logs: []
        },
        ...state.tasks
      ]
    }));
    await waitFor(() => expect(listJobs).toHaveBeenCalledTimes(4));
    await waitFor(() => expect(screen.queryByText(/看板数据加载失败/)).not.toBeInTheDocument());
  });
});
