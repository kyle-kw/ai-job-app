import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mockJobs } from '$lib/mock-data';
import { buildClientJobDataReport } from '$lib/report';
import { backend } from '$lib/services/backend';
import type { InterviewPreparationState, JobDataReport, RenderResult } from '$lib/types';
import ReportPage from './+page.svelte';

const exportFileMocks = vi.hoisted(() => ({ choosePath: vi.fn<() => Promise<string | null>>() }));
vi.mock('$lib/export-file', () => ({
  chooseLocalExportPath: exportFileMocks.choosePath,
  localExportStamp: () => '20260713_120000'
}));

const localReport = buildClientJobDataReport(mockJobs);
const reportKeywords = [
  { key: '__historical_unclassified__', label: '历史未分类', jobCount: 5, lastSeen: '2026-07-12T08:00:00.000Z' },
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
      { name: 'RAG 评测', gap: '缺少离线评测方法的完整说明', action: '整理一套指标、数据集与误差分析流程。', jobCount: 2 },
      { name: '系统设计', gap: '需要更清晰地解释取舍', action: '按容量、延迟、成本和可靠性演练架构题。', jobCount: 3 }
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

const listReportKeywords = vi.fn<() => Promise<typeof reportKeywords>>();
const getJobDataReport = vi.fn<(keywordKeys: string[]) => Promise<JobDataReport>>();
const exportJobDataReport = vi.fn<(keywordKeys: string[], outputPath: string) => Promise<RenderResult>>();
const getInterviewPreparationState = vi.fn<(keywordKeys: string[]) => Promise<InterviewPreparationState>>();
const generateInterviewPreparation = vi.fn<(keywordKeys: string[], force?: boolean) => Promise<InterviewPreparationState>>();

describe('full job data report page', () => {
  beforeEach(() => {
    Object.assign(backend, { listReportKeywords, getJobDataReport, exportJobDataReport, getInterviewPreparationState, generateInterviewPreparation });
    listReportKeywords.mockReset().mockResolvedValue(reportKeywords);
    getJobDataReport.mockReset().mockResolvedValue(localReport);
    exportJobDataReport.mockReset().mockResolvedValue({ path: 'C:\\tmp\\report.html', fileName: 'report.html' });
    exportFileMocks.choosePath.mockReset().mockResolvedValue('C:\\tmp\\report.html');
    getInterviewPreparationState.mockReset().mockResolvedValue(missingGeneralState);
    generateInterviewPreparation.mockReset().mockResolvedValue(freshState);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
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

    await waitFor(() => expect(generateInterviewPreparation).toHaveBeenCalledWith(['data-analysis'], true));
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
    await waitFor(() => expect(getJobDataReport).toHaveBeenLastCalledWith(['ai-agent', 'data-analysis']));
    expect(getInterviewPreparationState).toHaveBeenLastCalledWith(['ai-agent', 'data-analysis']);

    await fireEvent.click(screen.getByRole('button', { name: '生成 AI 面试准备' }));
    await waitFor(() => expect(generateInterviewPreparation).toHaveBeenCalledWith(['ai-agent', 'data-analysis'], false));

    await fireEvent.click(screen.getByRole('button', { name: '导出 HTML' }));
    await waitFor(() => expect(exportJobDataReport).toHaveBeenCalledWith(['ai-agent', 'data-analysis'], 'C:\\tmp\\report.html'));
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
});
