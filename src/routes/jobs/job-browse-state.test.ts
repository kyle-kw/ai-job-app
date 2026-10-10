import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import type { BeforeNavigate } from '@sveltejs/kit';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { jobsBrowseHref } from '$lib/jobs-browse-cache';
import { mockJobs, mockSnapshot } from '$lib/mock-data';
import { backend } from '$lib/services/backend';
import { snapshot } from '$lib/stores/app';
import type { FitAnalysisResult, Job, JobPage, TaskRun } from '$lib/types';
import JobPageComponent from './+page.svelte';

const navigationMocks = vi.hoisted(() => ({
  callbacks: [] as Array<(navigation: BeforeNavigate) => void>,
  replaceState: vi.fn((url: string | URL, state: object) => {
    window.history.replaceState(state, '', url);
  })
}));

vi.mock('$app/navigation', () => ({
  replaceState: navigationMocks.replaceState,
  beforeNavigate: vi.fn((callback: (navigation: BeforeNavigate) => void) => {
    navigationMocks.callbacks.push(callback);
  })
}));

const observers: Array<{ callback: IntersectionObserverCallback; connected: boolean }> = [];

function makeJob(index: number): Job {
  return {
    ...structuredClone(mockJobs[index % mockJobs.length]),
    id: `browse-job-${index}`,
    title: `浏览岗位 ${index}`,
    description: `浏览岗位 ${index} 的完整描述`
  };
}

function page(items: Job[], nextCursor: string | null = null, total = items.length): JobPage {
  return { items, total, pendingDetailCount: 0, nextCursor };
}

function renderJobs() {
  const shell = document.createElement('main');
  shell.className = 'page-shell';
  shell.dataset.testid = 'browse-test-shell';
  document.body.append(shell);
  return { ...render(JobPageComponent, { target: shell }), shell };
}

function leaveJobs(rendered: ReturnType<typeof renderJobs>, destination = '/resume') {
  const destinationUrl = new URL(destination, window.location.origin);
  for (const callback of navigationMocks.callbacks) {
    callback({
      from: {
        url: new URL(window.location.href),
        params: {},
        route: { id: '/jobs' },
        scroll: { x: 0, y: 0 }
      },
      to: {
        url: destinationUrl,
        params: {},
        route: { id: destinationUrl.pathname === '/resume' ? '/resume' : null },
        scroll: null
      },
      type: 'link',
      event: new MouseEvent('click') as PointerEvent,
      willUnload: false,
      complete: Promise.resolve(),
      cancel: vi.fn()
    });
  }
  // SvelteKit changes history before destroying the previous route component.
  window.history.replaceState({}, '', destination);
  rendered.unmount();
  rendered.shell.remove();
  navigationMocks.callbacks.length = 0;
}

function returnToJobs(href = get(jobsBrowseHref)) {
  window.history.replaceState({}, '', href);
  return renderJobs();
}

function listElement(): HTMLElement {
  return document.querySelector('.job-list') as HTMLElement;
}

function detailElement(): HTMLElement {
  return document.querySelector('.job-detail') as HTMLElement;
}

function jobRow(job: Job) {
  return within(listElement()).getByRole('button', { name: new RegExp(job.title) });
}

function loadMore() {
  const observer = observers.findLast((entry) => entry.connected);
  expect(observer).toBeDefined();
  observer!.callback(
    [{ isIntersecting: true } as IntersectionObserverEntry],
    {} as IntersectionObserver
  );
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

beforeEach(() => {
  window.history.replaceState({}, '', '/jobs');
  snapshot.set(structuredClone(mockSnapshot));
  navigationMocks.callbacks.length = 0;
  observers.length = 0;
  vi.spyOn(backend, 'listJobFilterOptions').mockResolvedValue({
    cities: ['上海', '杭州', '北京'],
    experiences: ['3-5年'],
    skills: [
      { label: 'Python', count: 4 },
      { label: 'SQL', count: 3 }
    ]
  });
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      entry: (typeof observers)[number];

      constructor(callback: IntersectionObserverCallback) {
        this.entry = { callback, connected: true };
        observers.push(this.entry);
      }

      observe() {}

      disconnect() {
        this.entry.connected = false;
      }
    }
  );
});

afterEach(() => {
  cleanup();
  document.querySelectorAll('[data-testid="browse-test-shell"]').forEach((node) => node.remove());
  navigationMocks.callbacks.length = 0;
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  window.history.replaceState({}, '', '/');
});

describe('jobs navigation browsing state', () => {
  it('restores filters, selected job, detail tab, and all scroll containers without reloading jobs', async () => {
    const jobs = [makeJob(0), makeJob(1)];
    const listJobs = vi.spyOn(backend, 'listJobsPage').mockResolvedValue(page(jobs));
    window.history.replaceState(
      {},
      '',
      '/jobs?city=杭州&skill=Python&experience=3-5%E5%B9%B4&salaryBand=25-35'
    );
    const rendered = renderJobs();
    await screen.findByRole('heading', { level: 2, name: jobs[0].title });

    await fireEvent.input(screen.getByPlaceholderText('搜索岗位或公司'), {
      target: { value: '数据' }
    });
    await fireEvent.input(screen.getByRole('slider'), { target: { value: '50' } });
    await fireEvent.change(screen.getByLabelText('公司规模'), { target: { value: '303' } });
    await fireEvent.change(screen.getByLabelText('岗位排序'), { target: { value: 'salary-desc' } });
    await fireEvent.click(screen.getByRole('checkbox', { name: '最近一次抓取新增' }));
    await fireEvent.click(screen.getByRole('checkbox', { name: '只看无原始详情' }));
    await waitFor(() =>
      expect(listJobs).toHaveBeenLastCalledWith(
        expect.objectContaining({
          query: '数据',
          minScore: 50,
          companyScale: '303',
          onlyNew: true,
          missingDescription: true,
          sort: 'salary-desc'
        })
      )
    );
    await fireEvent.click(jobRow(jobs[1]));
    await fireEvent.click(screen.getByRole('button', { name: /^匹配分析/ }));
    listElement().scrollTop = 840;
    detailElement().scrollTop = 430;
    rendered.shell.scrollTop = 120;
    await fireEvent.scroll(listElement());
    await fireEvent.scroll(detailElement());
    const callsBeforeLeaving = listJobs.mock.calls.length;

    leaveJobs(rendered, `/resume?job=${jobs[1].id}&assistant=1`);
    const returnHref = new URL(get(jobsBrowseHref), window.location.origin);
    expect(returnHref.pathname).toBe('/jobs');
    expect(returnHref.searchParams.get('city')).toBe('杭州');
    expect(returnHref.searchParams.getAll('skill')).toEqual(['Python']);
    expect(returnHref.searchParams.has('assistant')).toBe(false);
    expect(returnHref.searchParams.has('job')).toBe(false);

    const returned = returnToJobs();
    await screen.findByRole('heading', { level: 2, name: jobs[1].title });
    expect(await screen.findByRole('heading', { name: '岗位匹配分析' })).toBeInTheDocument();
    await waitFor(() => {
      expect(listElement().scrollTop).toBe(840);
      expect(detailElement().scrollTop).toBe(430);
      expect(returned.shell.scrollTop).toBe(120);
    });
    expect(screen.getByPlaceholderText('搜索岗位或公司')).toHaveValue('数据');
    expect(screen.getByRole('slider')).toHaveValue('50');
    await waitFor(() => {
      expect(screen.getByLabelText('城市')).toHaveValue('杭州');
      expect(screen.getByLabelText('经验要求')).toHaveValue('3-5年');
    });
    expect(screen.getByLabelText('薪资范围')).toHaveValue('25-35');
    expect(screen.getByLabelText('公司规模')).toHaveValue('303');
    expect(screen.getByLabelText('岗位排序')).toHaveValue('salary-desc');
    expect(screen.getByRole('checkbox', { name: '最近一次抓取新增' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: '只看无原始详情' })).toBeChecked();
    expect(listJobs).toHaveBeenCalledTimes(callsBeforeLeaving);
  });

  it('retains loaded pages and continues with the saved pagination cursor after returning', async () => {
    const jobs = Array.from({ length: 6 }, (_, index) => makeJob(index));
    const listJobs = vi.spyOn(backend, 'listJobsPage').mockImplementation(async (query) => {
      if (query.cursor === 'page-2') return page(jobs.slice(2, 4), 'page-3', 6);
      if (query.cursor === 'page-3') return page(jobs.slice(4), null, 6);
      return page(jobs.slice(0, 2), 'page-2', 6);
    });
    const rendered = renderJobs();
    await screen.findByRole('heading', { level: 2, name: jobs[0].title });
    loadMore();
    await waitFor(() => expect(jobRow(jobs[3])).toBeInTheDocument());
    await fireEvent.click(jobRow(jobs[3]));
    listElement().scrollTop = 1400;
    await fireEvent.scroll(listElement());

    leaveJobs(rendered);
    returnToJobs();
    await screen.findByRole('heading', { level: 2, name: jobs[3].title });
    expect(listElement().querySelectorAll('.job-row')).toHaveLength(4);
    await waitFor(() => expect(listElement().scrollTop).toBe(1400));
    expect(listJobs).toHaveBeenCalledTimes(2);

    loadMore();
    await waitFor(() => expect(jobRow(jobs[5])).toBeInTheDocument());
    expect(listJobs).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: 'page-3' }));
  });

  it('opens an explicit job deep link instead of restoring another selected job or old search', async () => {
    const jobs = [makeJob(0), makeJob(1)];
    const linkedJob = makeJob(7);
    const listJobs = vi.spyOn(backend, 'listJobsPage').mockResolvedValue(page(jobs));
    const getJob = vi.spyOn(backend, 'getJob').mockResolvedValue(linkedJob);
    const rendered = renderJobs();
    await screen.findByRole('heading', { level: 2, name: jobs[0].title });
    await fireEvent.input(screen.getByPlaceholderText('搜索岗位或公司'), {
      target: { value: '旧筛选' }
    });
    await waitFor(() =>
      expect(listJobs).toHaveBeenLastCalledWith(expect.objectContaining({ query: '旧筛选' }))
    );
    await fireEvent.click(jobRow(jobs[1]));
    listElement().scrollTop = 600;
    leaveJobs(rendered);

    returnToJobs(`/jobs?job=${linkedJob.id}`);
    expect(
      await screen.findByRole('heading', { level: 2, name: linkedJob.title })
    ).toBeInTheDocument();
    expect(getJob).toHaveBeenCalledWith(linkedJob.id);
    expect(listJobs).toHaveBeenLastCalledWith(expect.objectContaining({ query: '', cursor: null }));
    expect(screen.getByPlaceholderText('搜索岗位或公司')).toHaveValue('');
    expect(listElement().scrollTop).toBe(0);
  });

  it('applies a new report drilldown instead of the cached browsing context', async () => {
    const jobs = [makeJob(0), makeJob(1)];
    const listJobs = vi.spyOn(backend, 'listJobsPage').mockResolvedValue(page(jobs));
    const rendered = renderJobs();
    await screen.findByRole('heading', { level: 2, name: jobs[0].title });
    await fireEvent.change(screen.getByLabelText('城市'), { target: { value: '杭州' } });
    await waitFor(() =>
      expect(listJobs).toHaveBeenLastCalledWith(expect.objectContaining({ city: '杭州' }))
    );
    leaveJobs(rendered);

    returnToJobs('/jobs?from=report&keyword=data-analysis&skill=SQL&city=北京');
    expect(await screen.findByText('来自数据报告')).toBeInTheDocument();
    await waitFor(() =>
      expect(listJobs).toHaveBeenLastCalledWith(
        expect.objectContaining({
          city: '北京',
          keywordKeys: ['data-analysis'],
          skills: ['SQL'],
          cursor: null
        })
      )
    );
    expect(screen.getByLabelText('城市')).toHaveValue('北京');
    expect(listElement().scrollTop).toBe(0);
  });

  it('loads the new filter when navigating away before its debounce finishes', async () => {
    const oldJob = makeJob(0);
    const filteredJob = makeJob(1);
    const listJobs = vi
      .spyOn(backend, 'listJobsPage')
      .mockImplementation(async (query) =>
        page(query.query === '新筛选' ? [filteredJob] : [oldJob])
      );
    const rendered = renderJobs();
    await screen.findByRole('heading', { level: 2, name: oldJob.title });
    await fireEvent.input(screen.getByPlaceholderText('搜索岗位或公司'), {
      target: { value: '新筛选' }
    });
    leaveJobs(rendered);
    expect(listJobs).toHaveBeenCalledTimes(1);

    returnToJobs();
    expect(
      await screen.findByRole('heading', { level: 2, name: filteredJob.title })
    ).toBeInTheDocument();
    expect(listJobs).toHaveBeenLastCalledWith(expect.objectContaining({ query: '新筛选' }));
    expect(screen.getByPlaceholderText('搜索岗位或公司')).toHaveValue('新筛选');
    expect(listElement().querySelectorAll('.job-row')).toHaveLength(1);
    expect(within(listElement()).queryByText(oldJob.title)).not.toBeInTheDocument();
  });

  it('refreshes an unfinished query on return and ignores its response after the old page is destroyed', async () => {
    const oldJob = makeJob(0);
    const currentJob = makeJob(1);
    const lateJob = makeJob(2);
    const pending = deferred<JobPage>();
    const listJobs = vi
      .spyOn(backend, 'listJobsPage')
      .mockResolvedValueOnce(page([oldJob]))
      .mockImplementationOnce(() => pending.promise)
      .mockResolvedValue(page([currentJob]));
    const rendered = renderJobs();
    await screen.findByRole('heading', { level: 2, name: oldJob.title });
    await fireEvent.input(screen.getByPlaceholderText('搜索岗位或公司'), {
      target: { value: '新筛选' }
    });
    await waitFor(() => expect(listJobs).toHaveBeenCalledTimes(2));
    leaveJobs(rendered);

    const returned = returnToJobs();
    expect(
      await screen.findByRole('heading', { level: 2, name: currentJob.title })
    ).toBeInTheDocument();
    expect(listJobs).toHaveBeenCalledTimes(3);
    expect(listJobs).toHaveBeenLastCalledWith(expect.objectContaining({ query: '新筛选' }));
    pending.resolve(page([lateJob]));
    await tick();
    await tick();
    expect(screen.getByRole('heading', { level: 2, name: currentJob.title })).toBeInTheDocument();

    leaveJobs(returned);
    returnToJobs();
    expect(
      await screen.findByRole('heading', { level: 2, name: currentJob.title })
    ).toBeInTheDocument();
    expect(listJobs).toHaveBeenCalledTimes(3);
  });

  it.each(['before returning', 'after returning'] as const)(
    'shows an analysis completed %s after leaving the page, preserving loaded pages, selection, tab, and scroll',
    async (completionTiming) => {
      const jobs = Array.from({ length: 4 }, (_, index) => makeJob(index));
      jobs[3] = { ...jobs[3], fit: null };
      const analyzedJob: Job = {
        ...jobs[3],
        fit: {
          ...structuredClone(mockJobs[0].fit!),
          summary: '导航期间完成的匹配分析',
          analysisSource: 'llm',
          cacheStatus: 'fresh'
        }
      };
      let analysisCompleted = false;
      const listJobs = vi.spyOn(backend, 'listJobsPage').mockImplementation(async (query) => {
        const items = query.cursor === 'page-2' ? jobs.slice(2) : jobs.slice(0, 2);
        return page(
          items.map((job) => (analysisCompleted && job.id === analyzedJob.id ? analyzedJob : job)),
          query.cursor === 'page-2' ? null : 'page-2',
          jobs.length
        );
      });
      const pending = deferred<FitAnalysisResult>();
      const analyzeJob = vi.spyOn(backend, 'analyzeJob').mockImplementation(() => pending.promise);
      const bootstrap = vi
        .spyOn(backend, 'bootstrap')
        .mockResolvedValue(structuredClone(mockSnapshot));
      const rendered = renderJobs();
      await screen.findByRole('heading', { level: 2, name: jobs[0].title });
      loadMore();
      await waitFor(() => expect(jobRow(jobs[3])).toBeInTheDocument());
      await fireEvent.click(jobRow(jobs[3]));
      await fireEvent.click(screen.getByRole('button', { name: /^匹配分析/ }));
      const analysisToolbar = screen.getByRole('heading', { name: '岗位匹配分析' }).parentElement!
        .parentElement!;
      await fireEvent.click(within(analysisToolbar).getByRole('button', { name: '分析当前岗位' }));
      expect(analyzeJob).toHaveBeenCalledWith(jobs[3].id, false);
      listElement().scrollTop = 1400;
      detailElement().scrollTop = 320;
      await fireEvent.scroll(listElement());
      await fireEvent.scroll(detailElement());
      leaveJobs(rendered);

      if (completionTiming === 'after returning') {
        returnToJobs();
        await screen.findByRole('heading', { level: 2, name: jobs[3].title });
        expect(screen.queryByText(analyzedJob.fit!.summary)).not.toBeInTheDocument();
        expect(listJobs).toHaveBeenCalledTimes(2);
      }

      analysisCompleted = true;
      pending.resolve({ job: analyzedJob, cacheHit: false, source: 'llm' });
      await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(1));
      await tick();
      if (completionTiming === 'before returning') {
        expect(listJobs).toHaveBeenCalledTimes(2);
        returnToJobs();
      }

      expect(await screen.findByText(analyzedJob.fit!.summary)).toBeInTheDocument();
      expect(screen.getByRole('heading', { level: 2, name: jobs[3].title })).toBeInTheDocument();
      expect(screen.getByRole('heading', { name: '岗位匹配分析' })).toBeInTheDocument();
      expect(listElement().querySelectorAll('.job-row')).toHaveLength(4);
      expect(listJobs).toHaveBeenCalledTimes(4);
      expect(listJobs.mock.calls.slice(2).map(([query]) => query.cursor)).toEqual([null, 'page-2']);
      expect(get(snapshot).tasks).toEqual(mockSnapshot.tasks);
      await waitFor(() => {
        expect(listElement().scrollTop).toBe(1400);
        expect(detailElement().scrollTop).toBe(320);
      });
    }
  );

  it.each(['completed task', 'resume revision'] as const)(
    'refreshes all previously loaded pages after a %s changes while away, preserving the selected job and position',
    async (change) => {
      const jobs = Array.from({ length: 4 }, (_, index) => makeJob(index));
      let refreshed = false;
      const listJobs = vi.spyOn(backend, 'listJobsPage').mockImplementation(async (query) => {
        const items = query.cursor === 'page-2' ? jobs.slice(2) : jobs.slice(0, 2);
        return page(
          items.map((job) => ({
            ...job,
            description: refreshed ? '后台更新后的岗位详情' : job.description
          })),
          query.cursor === 'page-2' ? null : 'page-2',
          jobs.length
        );
      });
      const rendered = renderJobs();
      await screen.findByRole('heading', { level: 2, name: jobs[0].title });
      loadMore();
      await waitFor(() => expect(jobRow(jobs[3])).toBeInTheDocument());
      await fireEvent.click(jobRow(jobs[3]));
      listElement().scrollTop = 1400;
      detailElement().scrollTop = 320;
      await fireEvent.scroll(listElement());
      await fireEvent.scroll(detailElement());
      leaveJobs(rendered);

      refreshed = true;
      snapshot.update((state) => {
        if (change === 'resume revision') {
          return {
            ...state,
            resume: { ...state.resume!, version: state.resume!.version + 1 }
          };
        }
        const task: TaskRun = {
          id: 'browse-background-scrape',
          kind: 'scrape',
          title: '后台抓取',
          state: 'completed',
          progress: 100,
          message: '已完成',
          createdAt: '2026-10-10T02:00:00.000Z',
          updatedAt: '2026-10-10T02:05:00.000Z',
          logs: []
        };
        return { ...state, tasks: [...state.tasks, task] };
      });

      returnToJobs();
      await waitFor(() => expect(listJobs).toHaveBeenCalledTimes(4));
      expect(listJobs.mock.calls.slice(2).map(([query]) => query.cursor)).toEqual([null, 'page-2']);
      expect(
        await screen.findByRole('heading', { level: 2, name: jobs[3].title })
      ).toBeInTheDocument();
      expect(screen.getByText('后台更新后的岗位详情')).toBeInTheDocument();
      expect(listElement().querySelectorAll('.job-row')).toHaveLength(4);
      await waitFor(() => {
        expect(listElement().scrollTop).toBe(1400);
        expect(detailElement().scrollTop).toBe(320);
      });
    }
  );
});
