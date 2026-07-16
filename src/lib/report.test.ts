import { describe, expect, it } from 'vitest';
import { mockJobs } from '$lib/mock-data';
import { buildClientJobDataReport, buildScrapeSampleSummary, classifyJobRole, parseMonthlySalary } from '$lib/report';
import type { ScrapeRun, SearchSpec } from '$lib/types';

describe('job data report', () => {
  it('classifies Chinese AI titles without matching English substrings', () => {
    expect(classifyJobRole('AI工程师')).toBe('AI / Agent 开发');
    expect(classifyJobRole('AI 应用开发')).toBe('AI / Agent 开发');
    expect(classifyJobRole('Paid Media Specialist')).toBe('其他岗位');
    expect(classifyJobRole('Rail Platform Engineer')).toBe('其他岗位');
  });
  it('parses BOSS monthly salary and bonus months', () => {
    expect(parseMonthlySalary('20-30K·15薪')).toEqual({ low: 20, mid: 25, high: 30, months: 15 });
    expect(parseMonthlySalary('薪资面议')).toBeNull();
  });

  it('aggregates all deduplicated jobs for browser preview', () => {
    const report = buildClientJobDataReport(mockJobs);
    expect(report.totalJobs).toBe(mockJobs.length);
    expect(report.totalCompanies).toBeGreaterThan(0);
    expect(report.topSkills[0].count).toBeGreaterThan(0);
    expect(report.skillPairs.length).toBeGreaterThan(0);
    expect(report.insights).toHaveLength(4);
  });

  it('compares the latest two identical cross-day scrape batches', () => {
    const spec: SearchSpec = { keyword: 'AI Agent', city: '上海', pages: 3, salary: '20-40K', experience: '3-5年' };
    const previousJobs = [
      { ...mockJobs[0], id: 'shared', salary: '20-30K', skills: ['Python'] },
      { ...mockJobs[1], id: 'previous-only', salary: '10-20K', skills: ['Python'] }
    ];
    const currentJobs = [
      { ...mockJobs[0], id: 'shared', salary: '30-40K', skills: ['Python', 'RAG'] },
      { ...mockJobs[1], id: 'current-new-1', salary: '40-50K', skills: ['RAG'] },
      { ...mockJobs[2], id: 'current-new-2', salary: '50-60K', skills: ['RAG'] }
    ];
    const run = (id: string, completedAt: string, jobs: typeof currentJobs): ScrapeRun => ({
      id, keyword: 'AI Agent', city: '上海', totalSeen: jobs.length, inserted: jobs.length, updated: 0,
      startedAt: completedAt, completedAt, reportMarkdown: null, searchSpec: spec, resolvedCity: '上海',
      sample: buildScrapeSampleSummary(jobs)
    });
    const runs = [
      run('current', '2026-07-16T10:00:00+08:00', currentJobs),
      run('previous', '2026-07-15T10:00:00+08:00', previousJobs)
    ];
    const report = buildClientJobDataReport(currentJobs, [
      { key: 'ai agent', label: 'AI Agent', jobCount: 3, lastSeen: '2026-07-16T10:00:00+08:00' }
    ], runs, new Date('2026-07-16T04:00:00Z'));

    expect(report.batchComparison.status).toBe('available');
    expect(report.batchComparison.jobCountChangePercentage).toBe(50);
    expect(report.batchComparison.newlyObservedJobs).toBe(2);
    expect(report.batchComparison.notObservedJobs).toBe(1);
    expect(report.batchComparison.salaryMedianDeltaK).toBe(25);
    expect(report.batchComparison.skillChanges.find((item) => item.label === 'RAG')?.deltaPercentagePoints).toBe(100);
  });

  it('reports sample limitations and refuses non-comparable ranges', () => {
    const sparse = { ...mockJobs[0], description: '', salary: '面议', skills: [], experience: '', degree: '' };
    const report = buildClientJobDataReport([sparse], [
      { key: 'ai-agent', label: 'AI Agent', jobCount: 1, lastSeen: sparse.lastSeen },
      { key: 'data-analysis', label: '数据分析', jobCount: 1, lastSeen: sparse.lastSeen }
    ]);

    expect(report.batchComparison).toMatchObject({ status: 'unavailable', reason: 'multi_keyword' });
    expect(report.sampleQuality.detail.coverage).toBe(0);
    expect(report.sampleQuality.limitations).toEqual(expect.arrayContaining([
      expect.stringContaining('有限页 BOSS'), expect.stringContaining('少于 20'), expect.stringContaining('薪资覆盖不足')
    ]));
  });
});
