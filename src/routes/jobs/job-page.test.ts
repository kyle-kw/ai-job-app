import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mockJobs, mockSnapshot } from '$lib/mock-data';
import { backend } from '$lib/services/backend';
import { snapshot } from '$lib/stores/app';
import JobPage from './+page.svelte';

const cities = [
  '北京', '上海', '广州', '深圳', '杭州', '天津', '西安', '苏州', '武汉', '厦门', '长沙', '成都', '郑州',
  '重庆', '佛山', '合肥', '济南', '青岛', '南京', '东莞', '昆明', '南昌', '石家庄', '宁波', '福州'
];

describe('job scraping controls', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('shows the popular city list, defaults to one page, and updates the estimate', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(JobPage);
    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));

    const cityInput = within(screen.getByRole('dialog')).getByLabelText('城市') as HTMLInputElement;
    const pageSelect = screen.getByLabelText('抓取页数') as HTMLSelectElement;
    await waitFor(() => expect(screen.getByLabelText('关键词')).toHaveValue('AI Agent'));
    expect(cityInput).toHaveValue('上海');
    await fireEvent.focus(cityInput);
    expect(within(screen.getByRole('listbox', { name: '城市选项' })).getAllByRole('option').map((option) => option.textContent)).toEqual(cities);
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

  it('filters every bundled city, requires selection, and supports keyboard choice', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const start = vi.spyOn(backend, 'startScrape').mockResolvedValue('scrape-city');
    render(JobPage);
    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));

    const dialog = screen.getByRole('dialog');
    const cityInput = within(dialog).getByLabelText('城市') as HTMLInputElement;
    const review = within(dialog).getByRole('button', { name: '开始抓取' });
    await fireEvent.input(cityInput, { target: { value: '赣' } });
    expect(screen.getByRole('option', { name: '赣州' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: '上海' })).not.toBeInTheDocument();
    expect(review).toBeDisabled();

    await fireEvent.click(screen.getByRole('option', { name: '赣州' }));
    expect(cityInput).toHaveValue('赣州');
    expect(review).toBeEnabled();

    await fireEvent.input(cityInput, { target: { value: '火星' } });
    expect(screen.getByText('未找到匹配城市')).toBeInTheDocument();
    expect(review).toBeDisabled();

    await fireEvent.input(cityInput, { target: { value: '洛阳' } });
    await fireEvent.keyDown(cityInput, { key: 'Enter' });
    expect(cityInput).toHaveValue('洛阳');
    expect(review).toBeEnabled();

    await fireEvent.click(review);
    await fireEvent.click(screen.getByRole('button', { name: '检查登录并开始抓取' }));
    await waitFor(() => expect(start).toHaveBeenCalledWith(expect.objectContaining({ city: '洛阳' })));
  });

  it('restores the complete persisted search each time the dialog opens', async () => {
    const state = structuredClone(mockSnapshot);
    state.lastSearchSpec = {
      keyword: '数据分析', city: '杭州', pages: 4, experience: '105', salary: '405', degree: '203', companyScale: '303'
    };
    snapshot.set(state);
    render(JobPage);

    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));
    let dialog = screen.getByRole('dialog');
    await waitFor(() => expect(within(dialog).getByLabelText('关键词')).toHaveValue('数据分析'));
    expect(within(dialog).getByLabelText('城市')).toHaveValue('杭州');
    expect(within(dialog).getByLabelText('抓取页数')).toHaveValue('4');
    expect(within(dialog).getByLabelText('经验要求')).toHaveValue('105');
    expect(within(dialog).getByLabelText('薪资范围')).toHaveValue('405');
    expect(within(dialog).getByLabelText('公司规模')).toHaveValue('303');
    await fireEvent.input(within(dialog).getByLabelText('关键词'), { target: { value: '正在编辑' } });

    snapshot.update((value) => ({
      ...value,
      lastSearchSpec: {
        keyword: '商业分析', city: '北京', pages: 2, experience: '104', salary: '404', degree: '', companyScale: '302'
      }
    }));
    expect(within(dialog).getByLabelText('关键词')).toHaveValue('正在编辑');

    await fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));
    dialog = screen.getByRole('dialog');
    await waitFor(() => expect(within(dialog).getByLabelText('关键词')).toHaveValue('商业分析'));
    expect(within(dialog).getByLabelText('城市')).toHaveValue('北京');
    expect(within(dialog).getByLabelText('抓取页数')).toHaveValue('2');
    expect(within(dialog).getByLabelText('经验要求')).toHaveValue('104');
    expect(within(dialog).getByLabelText('薪资范围')).toHaveValue('404');
    expect(within(dialog).getByLabelText('公司规模')).toHaveValue('302');
  });

  it('requires confirmation before checking login and starting a scrape', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const start = vi.spyOn(backend, 'startScrape').mockResolvedValue('scrape-confirmed');
    render(JobPage);

    await fireEvent.click(screen.getByRole('button', { name: '抓取新岗位' }));
    await fireEvent.click(screen.getByRole('button', { name: '开始抓取' }));
    expect(screen.getByRole('heading', { name: '抓取前确认' })).toBeInTheDocument();
    expect(screen.getByText(/如果出现登录界面，请在 5 分钟内完成登录/)).toBeInTheDocument();
    expect(start).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('button', { name: '返回修改' }));
    expect(screen.getByRole('heading', { name: '抓取新岗位' })).toBeInTheDocument();
    expect(start).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('button', { name: '开始抓取' }));
    await fireEvent.click(screen.getByRole('button', { name: '检查登录并开始抓取' }));
    await waitFor(() => expect(start).toHaveBeenCalledWith(expect.objectContaining({
      keyword: 'AI Agent', city: '上海', pages: 1
    })));
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

  it('passes the dynamic city and missing-description filters to the paged query', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const listJobsPage = vi.spyOn(backend, 'listJobsPage');
    vi.spyOn(backend, 'listJobCities').mockResolvedValue(['上海', '杭州']);
    render(JobPage);

    const city = screen.getByLabelText('城市');
    await waitFor(() => expect(within(city).getByRole('option', { name: '杭州' })).toBeInTheDocument());
    await fireEvent.change(city, { target: { value: '杭州' } });
    await fireEvent.click(screen.getByRole('checkbox', { name: '只看无原始详情' }));

    await waitFor(() => expect(listJobsPage).toHaveBeenLastCalledWith(expect.objectContaining({
      city: '杭州',
      missingDescription: true,
      cursor: null
    })));
  });

  it('exports all jobs with a timestamped JSON path and deletes one job after confirmation', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const exportJobsJson = vi.spyOn(backend, 'exportJobsJson').mockResolvedValue({ path: '岗位数据.json', fileName: '岗位数据.json' });
    const deleteJob = vi.spyOn(backend, 'deleteJob').mockResolvedValue({ deletedCount: 1 });
    render(JobPage);

    await fireEvent.click(screen.getByRole('button', { name: '导出全部岗位 JSON' }));
    await waitFor(() => expect(exportJobsJson).toHaveBeenCalledWith(expect.stringMatching(/^岗位数据_\d{8}_\d{6}\.json$/)));

    const deleteButton = await screen.findByRole('button', { name: '删除岗位' });
    await fireEvent.click(deleteButton);
    expect(await screen.findByRole('heading', { name: '确认删除岗位' })).toBeInTheDocument();
    expect(screen.getByText(/AI Agent 开发工程师 · 森亿智能/)).toBeInTheDocument();
    expect(deleteJob).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: '取消' }));
    expect(screen.queryByRole('heading', { name: '确认删除岗位' })).not.toBeInTheDocument();
    expect(deleteJob).not.toHaveBeenCalled();

    await fireEvent.click(deleteButton);
    await fireEvent.click(await screen.findByRole('button', { name: '确认删除' }));
    await waitFor(() => expect(deleteJob).toHaveBeenCalledWith(mockJobs[0].id));
  });

  it('only offers bulk deletion for the filtered missing-description result set', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const missingJob = { ...mockJobs[0], description: '' };
    vi.spyOn(backend, 'listJobCities').mockResolvedValue(['上海']);
    vi.spyOn(backend, 'listJobsPage').mockImplementation(async (query) => ({
      items: query.missingDescription ? [missingJob] : [missingJob, mockJobs[1]],
      total: query.missingDescription ? 1 : 2,
      pendingDetailCount: 0,
      nextCursor: null
    }));
    const deleteMissing = vi.spyOn(backend, 'deleteMissingDescriptionJobs').mockResolvedValue({ deletedCount: 1 });
    render(JobPage);

    expect(screen.queryByRole('button', { name: /删除无详情岗位/ })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('checkbox', { name: '只看无原始详情' }));
    const bulkButton = await screen.findByRole('button', { name: '删除无详情岗位（1）' });
    await fireEvent.click(bulkButton);
    expect(await screen.findByRole('heading', { name: '确认批量删除' })).toBeInTheDocument();
    expect(deleteMissing).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: '确认删除 1 个岗位' }));

    await waitFor(() => expect(deleteMissing).toHaveBeenCalledWith(expect.objectContaining({ missingDescription: true })));
  });
});
