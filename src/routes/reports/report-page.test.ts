import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { replaceState } from '$app/navigation';
import { mockJobs } from '$lib/mock-data';
import { buildClientJobDataReport } from '$lib/report';
import { backend } from '$lib/services/backend';
import type {
  InterviewPreparationState,
  JobDataReport,
  RenderResult,
  ReportCompetitivenessState
} from '$lib/types';
import ReportPage from './+page.svelte';

const navigationMocks = vi.hoisted(() => ({
  replaceState: vi.fn((url: string | URL, state: object) => {
    window.history.replaceState(state, '', url);
  })
}));
vi.mock('$app/navigation', () => ({ replaceState: navigationMocks.replaceState }));

const exportFileMocks = vi.hoisted(() => ({ choosePath: vi.fn<() => Promise<string | null>>() }));
vi.mock('$lib/export-file', () => ({
  chooseLocalExportPath: exportFileMocks.choosePath,
  localExportStamp: () => '20260713_120000'
}));

const localReport = buildClientJobDataReport(mockJobs);
const reportKeywords = [
  {
    key: '__historical_unclassified__',
    label: '历史未分类',
    jobCount: 5,
    lastSeen: '2026-07-12T08:00:00.000Z'
  },
  { key: 'ai-agent', label: 'AI Agent', jobCount: 18, lastSeen: '2026-07-10T08:00:00.000Z' },
  { key: 'data-analysis', label: '数据分析', jobCount: 12, lastSeen: '2026-07-11T08:00:00.000Z' }
];

const missingGeneralState: InterviewPreparationState = {
  status: 'missing',
  reason: 'no_resume',
  hasProvider: true,
  hasResume: false,
  preparation: null
};

const freshState: InterviewPreparationState = {
  status: 'fresh',
  reason: null,
  hasProvider: true,
  hasResume: true,
  generatedAt: '2026-07-11T08:00:00.000Z',
  preparation: {
    summary: '优先补齐 RAG 评测与系统设计表达，并准备一个可量化的落地案例。',
    skills: [
      {
        name: 'RAG 评测',
        gap: '缺少离线评测方法的完整说明',
        action: '整理一套指标、数据集与误差分析流程。',
        jobCount: 2
      },
      {
        name: '系统设计',
        gap: '需要更清晰地解释取舍',
        action: '按容量、延迟、成本和可靠性演练架构题。',
        jobCount: 3
      }
    ],
    projectIdeas: ['准备一个从检索基线到上线监控的完整项目案例。'],
    practiceQuestions: ['如何定位 RAG 系统中召回率下降的原因？']
  }
};

const staleState: InterviewPreparationState = {
  ...freshState,
  status: 'stale',
  preparation: {
    ...freshState.preparation!,
    summary: '这是岗位数据变化前生成的准备建议。'
  }
};

const localCompetitivenessState: ReportCompetitivenessState = {
  status: 'missing',
  reason: 'no_provider',
  hasProvider: false,
  hasResume: true,
  local: {
    source: 'local',
    resumeId: 'resume-master',
    resumeVersion: 3,
    generatedAt: '2026-07-16T08:00:00.000Z',
    items: [
      {
        id: 'report-skill-1',
        label: 'Python',
        jobCount: 4,
        percentage: 80,
        status: 'covered',
        resumePaths: ['/summary'],
        evidenceFactIds: [],
        rationale: '主简历正文中已有明确表达。'
      }
    ]
  },
  ai: null,
  effectiveSource: 'local'
};

const freshCompetitivenessState: ReportCompetitivenessState = {
  ...localCompetitivenessState,
  status: 'fresh',
  reason: null,
  hasProvider: true,
  generatedAt: '2026-07-16T09:00:00.000Z',
  ai: {
    ...localCompetitivenessState.local!,
    source: 'ai',
    generatedAt: '2026-07-16T09:00:00.000Z',
    items: [
      {
        ...localCompetitivenessState.local!.items[0],
        rationale: 'AI 语义复核确认了项目中的 Python 证据。'
      }
    ]
  },
  effectiveSource: 'ai'
};

const listReportKeywords = vi.fn<() => Promise<typeof reportKeywords>>();
const getJobDataReport = vi.fn<(keywordKeys: string[]) => Promise<JobDataReport>>();
const exportJobDataReport =
  vi.fn<(keywordKeys: string[], outputPath: string) => Promise<RenderResult>>();
const getInterviewPreparationState =
  vi.fn<(keywordKeys: string[]) => Promise<InterviewPreparationState>>();
const generateInterviewPreparation =
  vi.fn<(keywordKeys: string[], force?: boolean) => Promise<InterviewPreparationState>>();
const getReportCompetitivenessState =
  vi.fn<(keywordKeys: string[]) => Promise<ReportCompetitivenessState>>();
const generateReportCompetitiveness =
  vi.fn<(keywordKeys: string[], force?: boolean) => Promise<ReportCompetitivenessState>>();

describe('full job data report page', () => {
  beforeEach(() => {
    replaceState('/', {});
    Object.assign(backend, {
      listReportKeywords,
      getJobDataReport,
      exportJobDataReport,
      getInterviewPreparationState,
      generateInterviewPreparation,
      getReportCompetitivenessState,
      generateReportCompetitiveness
    });
    listReportKeywords.mockReset().mockResolvedValue(reportKeywords);
    getJobDataReport.mockReset().mockResolvedValue(localReport);
    exportJobDataReport
      .mockReset()
      .mockResolvedValue({ path: 'C:\\tmp\\report.html', fileName: 'report.html' });
    exportFileMocks.choosePath.mockReset().mockResolvedValue('C:\\tmp\\report.html');
    getInterviewPreparationState.mockReset().mockResolvedValue(missingGeneralState);
    generateInterviewPreparation.mockReset().mockResolvedValue(freshState);
    getReportCompetitivenessState.mockReset().mockResolvedValue(localCompetitivenessState);
    generateReportCompetitiveness.mockReset().mockResolvedValue(freshCompetitivenessState);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    replaceState('/', {});
  });

  it('keeps local analytics available and offers a general plan without a resume', async () => {
    render(ReportPage);

    expect(await screen.findByText('技能需求与共现组合')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: /数据分析/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /AI Agent/ })).not.toBeChecked();
    expect(getJobDataReport).toHaveBeenCalledWith(['data-analysis']);
    expect(getInterviewPreparationState).toHaveBeenCalledWith(['data-analysis']);
    expect(screen.getByText('薪资与候选人门槛')).toBeInTheDocument();
    expect(screen.getByText('市场结构')).toBeInTheDocument();
    expect(await screen.findByText('当前为通用市场模式')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '生成 AI 面试准备' })).toBeEnabled();
    expect(screen.getByRole('button', { name: /导出 HTML/ })).toBeEnabled();
  });

  it('renders a fresh preparation with skill counts, project ideas, and questions', async () => {
    getInterviewPreparationState.mockResolvedValue(freshState);
    render(ReportPage);

    expect(await screen.findByText(freshState.preparation!.summary)).toBeInTheDocument();
    expect(screen.getByText('RAG 评测')).toBeInTheDocument();
    expect(screen.getByText('2 个岗位提及')).toBeInTheDocument();
    expect(screen.getByText('准备一个从检索基线到上线监控的完整项目案例。')).toBeInTheDocument();
    expect(screen.getByText('如何定位 RAG 系统中召回率下降的原因？')).toBeInTheDocument();
    expect(screen.getByText('内容最新')).toBeInTheDocument();
  });

  it('refreshes a stale preparation explicitly and forces regeneration', async () => {
    getInterviewPreparationState.mockResolvedValue(staleState);
    render(ReportPage);

    expect(await screen.findByText('这是岗位数据变化前生成的准备建议。')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '刷新建议' }));

    await waitFor(() =>
      expect(generateInterviewPreparation).toHaveBeenCalledWith(['data-analysis'], true)
    );
    expect(await screen.findByText(freshState.preparation!.summary)).toBeInTheDocument();
    expect(screen.queryByText('这是岗位数据变化前生成的准备建议。')).not.toBeInTheDocument();
  });

  it('keeps local analytics and the previous preparation when AI generation fails', async () => {
    getInterviewPreparationState.mockResolvedValue(staleState);
    generateInterviewPreparation.mockRejectedValue(new Error('模型服务暂时不可用'));
    render(ReportPage);

    expect(await screen.findByText('这是岗位数据变化前生成的准备建议。')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '刷新建议' }));

    expect(await screen.findByText('AI 面试准备生成失败')).toBeInTheDocument();
    expect(screen.getByText(/模型服务暂时不可用/)).toBeInTheDocument();
    expect(screen.getByText('这是岗位数据变化前生成的准备建议。')).toBeInTheDocument();
    expect(screen.getByText('技能需求与共现组合')).toBeInTheDocument();
  });

  it('directs users to model settings when no provider is available', async () => {
    getInterviewPreparationState.mockResolvedValue({
      status: 'missing',
      reason: 'no_provider',
      hasProvider: false,
      hasResume: true,
      preparation: null
    });
    render(ReportPage);

    expect(await screen.findByText('先配置并验证 AI 模型')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '前往模型设置' })).toHaveAttribute('href', '/settings');
    expect(screen.queryByRole('button', { name: '生成 AI 面试准备' })).not.toBeInTheDocument();
  });

  it('uses the same multi-keyword scope for analytics, AI generation, and export', async () => {
    render(ReportPage);
    await screen.findByText('技能需求与共现组合');

    await fireEvent.click(screen.getByRole('checkbox', { name: /AI Agent/ }));
    await waitFor(() =>
      expect(getJobDataReport).toHaveBeenLastCalledWith(['ai-agent', 'data-analysis'])
    );
    expect(getInterviewPreparationState).toHaveBeenLastCalledWith(['ai-agent', 'data-analysis']);

    await fireEvent.click(screen.getByRole('button', { name: '生成 AI 面试准备' }));
    await waitFor(() =>
      expect(generateInterviewPreparation).toHaveBeenCalledWith(
        ['ai-agent', 'data-analysis'],
        false
      )
    );

    await fireEvent.click(screen.getByRole('button', { name: '导出 HTML' }));
    await waitFor(() =>
      expect(exportJobDataReport).toHaveBeenCalledWith(
        ['ai-agent', 'data-analysis'],
        'C:\\tmp\\report.html'
      )
    );
  });

  it('does not invoke report export when the save dialog is cancelled', async () => {
    exportFileMocks.choosePath.mockResolvedValueOnce(null);
    render(ReportPage);
    await screen.findByText('技能需求与共现组合');

    await fireEvent.click(screen.getByRole('button', { name: '导出 HTML' }));

    await waitFor(() => expect(exportFileMocks.choosePath).toHaveBeenCalled());
    expect(exportJobDataReport).not.toHaveBeenCalled();
  });

  it('does not generate a report when no keyword is selected', async () => {
    render(ReportPage);
    await screen.findByText('技能需求与共现组合');
    const callsBeforeClear = getJobDataReport.mock.calls.length;

    await fireEvent.click(screen.getByRole('checkbox', { name: /数据分析/ }));

    expect(await screen.findByText('请至少选择一个关键词')).toBeInTheDocument();
    expect(getJobDataReport).toHaveBeenCalledTimes(callsBeforeClear);
    expect(screen.getByRole('button', { name: '导出 HTML' })).toBeDisabled();
  });

  it('restores valid URL state, silently removes the old window parameter, and exposes drilldowns', async () => {
    replaceState('/reports?keyword=ai-agent&keyword=missing&window=30', {});
    render(ReportPage);

    await screen.findByText('最近两次同条件样本对比');
    expect(screen.getByRole('checkbox', { name: /AI Agent/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /数据分析/ })).not.toBeChecked();
    expect(getJobDataReport).toHaveBeenCalledWith(['ai-agent']);
    expect(new URL(window.location.href).searchParams.getAll('keyword')).toEqual(['ai-agent']);
    expect(new URL(window.location.href).searchParams.has('window')).toBe(false);
    expect(screen.queryByRole('button', { name: /近 (7|30) 天/ })).not.toBeInTheDocument();
    expect(screen.getByText('样本质量与限制')).toBeInTheDocument();

    const skillLink = screen.getByRole('link', { name: /查看 Python 的 \d+ 个岗位/ });
    const drilldown = new URL(skillLink.getAttribute('href')!, 'http://localhost');
    expect(drilldown.pathname).toBe('/jobs');
    expect(drilldown.searchParams.get('from')).toBe('report');
    expect(drilldown.searchParams.has('window')).toBe(false);
    expect(drilldown.searchParams.getAll('keyword')).toEqual(['ai-agent']);
    expect(drilldown.searchParams.getAll('skill')).toEqual(['Python']);

    await fireEvent.click(screen.getByRole('button', { name: '导出 HTML' }));
    await waitFor(() =>
      expect(exportJobDataReport).toHaveBeenCalledWith(['ai-agent'], 'C:\\tmp\\report.html')
    );
  });

  it('renders comparable batch deltas without treating missing ids as removed jobs', async () => {
    const spec = { keyword: '数据分析', city: '上海', pages: 2 };
    getJobDataReport.mockResolvedValue({
      ...localReport,
      batchComparison: {
        status: 'available',
        reason: null,
        previous: {
          runId: 'previous',
          completedAt: '2026-07-15T10:00:00+08:00',
          searchSpec: spec,
          totalJobs: 4,
          detailCoverage: 50,
          salarySampleCount: 3,
          medianSalaryK: 25
        },
        current: {
          runId: 'current',
          completedAt: '2026-07-16T10:00:00+08:00',
          searchSpec: spec,
          totalJobs: 5,
          detailCoverage: 60,
          salarySampleCount: 4,
          medianSalaryK: 30
        },
        jobCountChangePercentage: 25,
        newlyObservedJobs: 2,
        notObservedJobs: 1,
        salaryMedianDeltaK: 5,
        skillChanges: [
          {
            label: 'Python',
            currentCount: 4,
            currentPercentage: 80,
            previousCount: 2,
            previousPercentage: 50,
            deltaPercentagePoints: 30
          }
        ]
      }
    });
    render(ReportPage);

    expect(await screen.findByText('本次有限结果未再次出现')).toBeInTheDocument();
    expect(screen.getByText('不代表岗位已经下架')).toBeInTheDocument();
    expect(screen.getByText('+25.0%')).toBeInTheDocument();
    expect(screen.getByText('+5.0K')).toBeInTheDocument();
    expect(screen.getByText('+30.0pp')).toBeInTheDocument();
  });

  it('runs competitiveness AI only after an explicit click', async () => {
    getReportCompetitivenessState.mockResolvedValue({
      ...localCompetitivenessState,
      hasProvider: true,
      reason: null
    });
    render(ReportPage);

    expect(await screen.findByText('主简历正文中已有明确表达。')).toBeInTheDocument();
    expect(generateReportCompetitiveness).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: 'AI 语义分析' }));

    await waitFor(() =>
      expect(generateReportCompetitiveness).toHaveBeenCalledWith(['data-analysis'], false)
    );
    expect(await screen.findByText('AI 语义复核确认了项目中的 Python 证据。')).toBeInTheDocument();
  });

  it('falls back to the local matrix when a cached AI result is stale', async () => {
    getReportCompetitivenessState.mockResolvedValue({
      ...freshCompetitivenessState,
      status: 'stale',
      reason: 'data_changed',
      local: localCompetitivenessState.local,
      effectiveSource: 'local'
    });
    render(ReportPage);

    expect(
      await screen.findByText('旧 AI 结果未用于当前矩阵，下面已自动回退为最新本地结果。')
    ).toBeInTheDocument();
    expect(screen.getByText('主简历正文中已有明确表达。')).toBeInTheDocument();
    expect(screen.queryByText('AI 语义复核确认了项目中的 Python 证据。')).not.toBeInTheDocument();
  });

  it('keeps the local matrix when competitiveness AI generation fails', async () => {
    getReportCompetitivenessState.mockResolvedValue({
      ...localCompetitivenessState,
      hasProvider: true,
      reason: null
    });
    generateReportCompetitiveness.mockRejectedValue(new Error('语义分析服务暂时不可用'));
    render(ReportPage);

    expect(await screen.findByText('主简历正文中已有明确表达。')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'AI 语义分析' }));

    expect(await screen.findByText('AI 竞争力分析失败')).toBeInTheDocument();
    expect(screen.getByText(/语义分析服务暂时不可用/)).toBeInTheDocument();
    expect(screen.getByText('主简历正文中已有明确表达。')).toBeInTheDocument();
  });

  it('links strengthenable and gap skills into a prefilled market-context resume review', async () => {
    getReportCompetitivenessState.mockResolvedValue({
      ...localCompetitivenessState,
      local: {
        ...localCompetitivenessState.local!,
        items: [
          {
            id: 'strength',
            label: 'RAG',
            jobCount: 3,
            percentage: 60,
            status: 'strengthenable',
            resumePaths: [],
            evidenceFactIds: ['fact-rag'],
            rationale: '已确认事实中有证据。'
          },
          {
            id: 'gap',
            label: 'Kubernetes',
            jobCount: 2,
            percentage: 40,
            status: 'gap',
            resumePaths: [],
            evidenceFactIds: [],
            rationale: '尚无候选人证据。'
          }
        ]
      }
    });
    render(ReportPage);

    const wholeReportLink = await screen.findByRole('link', { name: '基于当前样本优化主简历' });
    const strengthLink = screen.getByRole('link', { name: '生成表达优化' });
    const gapLink = screen.getByRole('link', { name: '核对相关经历' });
    for (const link of [wholeReportLink, strengthLink, gapLink]) {
      const url = new URL(link.getAttribute('href')!, 'http://localhost');
      expect(url.pathname).toBe('/resume');
      expect(url.searchParams.get('market')).toBe('1');
      expect(url.searchParams.getAll('keyword')).toEqual(['data-analysis']);
    }
    expect(
      new URL(strengthLink.getAttribute('href')!, 'http://localhost').searchParams.getAll(
        'focusSkill'
      )
    ).toEqual(['RAG']);
    expect(
      new URL(gapLink.getAttribute('href')!, 'http://localhost').searchParams.getAll('focusSkill')
    ).toEqual(['Kubernetes']);
  });

  it('offers a resume entry when no master resume exists', async () => {
    getReportCompetitivenessState.mockResolvedValue({
      status: 'missing',
      reason: 'no_resume',
      hasProvider: true,
      hasResume: false,
      local: null,
      ai: null,
      effectiveSource: null
    });
    render(ReportPage);

    expect(await screen.findByText('先建立可信主简历')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '前往简历' })).toHaveAttribute('href', '/resume');
  });
});
