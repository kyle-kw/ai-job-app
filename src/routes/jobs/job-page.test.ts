import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import JobPage from './+page.svelte';

const cities = [
  '北京', '上海', '广州', '深圳', '杭州', '天津', '西安', '苏州', '武汉', '厦门', '长沙', '成都', '郑州',
  '重庆', '佛山', '合肥', '济南', '青岛', '南京', '东莞', '昆明', '南昌', '石家庄', '宁波', '福州'
];

describe('job scraping controls', () => {
  afterEach(cleanup);

  it('uses the fixed city list, defaults to one page, and updates the estimate', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(JobPage);
    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));

    const citySelect = screen.getByLabelText('城市') as HTMLSelectElement;
    const pageSelect = screen.getByLabelText('抓取页数') as HTMLSelectElement;
    await waitFor(() => expect(screen.getByLabelText('关键词')).toHaveValue('AI Agent'));
    expect(within(citySelect).getAllByRole('option').map((option) => option.textContent)).toEqual(cities);
    expect(pageSelect).toHaveValue('1');
    expect(within(pageSelect).getAllByRole('option').map((option) => option.textContent)).toEqual([
      '1 页（推荐）', '2 页', '3 页', '4 页', '5 页'
    ]);
    expect(screen.getByText('预计耗时：20 分钟')).toBeInTheDocument();

    const estimates = [['2', '40 分钟'], ['3', '60 分钟（约 1 小时）'], ['4', '80 分钟'], ['5', '100 分钟']];
    for (const [pages, duration] of estimates) {
      await fireEvent.change(pageSelect, { target: { value: pages } });
      await waitFor(() => expect(screen.getByText(`预计耗时：${duration}`)).toBeInTheDocument());
    }
    expect(screen.getByText(/抓取期间请勿关闭应用/)).toBeInTheDocument();
  });

  it('reads the latest completed keyword each time the dialog opens', async () => {
    const state = structuredClone(mockSnapshot);
    state.scrapeRuns = [
      { ...state.scrapeRuns[0], id: 'older', keyword: '财务会计', startedAt: '2026-07-10T08:00:00.000Z' },
      { ...state.scrapeRuns[0], id: 'failed', keyword: '失败尝试', startedAt: '2026-07-13T08:00:00.000Z', completedAt: null },
      { ...state.scrapeRuns[0], id: 'newer', keyword: '数据分析', startedAt: '2026-07-12T08:00:00.000Z' }
    ];
    snapshot.set(state);
    render(JobPage);

    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));
    await waitFor(() => expect(screen.getByLabelText('关键词')).toHaveValue('数据分析'));
    await fireEvent.input(screen.getByLabelText('关键词'), { target: { value: '正在编辑' } });

    snapshot.update((value) => ({
      ...value,
      scrapeRuns: [{ ...value.scrapeRuns[0], id: 'latest', keyword: '商业分析', startedAt: '2026-07-14T08:00:00.000Z', completedAt: '2026-07-14T08:10:00.000Z' }, ...value.scrapeRuns]
    }));
    expect(screen.getByLabelText('关键词')).toHaveValue('正在编辑');

    await fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));
    await waitFor(() => expect(screen.getByLabelText('关键词')).toHaveValue('商业分析'));
  });

  it('prevents opening another scrape while one is queued or running', () => {
    const state = structuredClone(mockSnapshot);
    state.tasks = [{
      id: 'scrape-running',
      kind: 'scrape',
      title: '抓取 上海 · AI Agent',
      state: 'running',
      progress: 35,
      message: '正在抓取岗位',
      createdAt: '2026-07-11T08:00:00.000Z',
      updatedAt: '2026-07-11T08:01:00.000Z',
      logs: []
    }];
    snapshot.set(state);

    render(JobPage);

    expect(screen.getByRole('button', { name: '岗位抓取中…' })).toBeDisabled();
    expect(screen.queryByRole('heading', { name: '抓取新岗位' })).not.toBeInTheDocument();
  });
});
