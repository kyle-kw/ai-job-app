import { describe, expect, it } from 'vitest';
import { mockJobs } from '$lib/mock-data';
import { buildClientJobDataReport, classifyJobRole, parseMonthlySalary } from '$lib/report';

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

  it('builds zero-filled Shanghai-time trend windows with comparable salary and skills', () => {
    const jobs = [
      {
        ...mockJobs[0], id: 'recent-boundary', firstSeen: '2026-07-09T16:30:00Z',
        lastSeen: '2026-07-10T01:00:00+08:00', salary: '20-30K', skills: ['Python']
      },
      {
        ...mockJobs[1], id: 'recent-second', firstSeen: '2026-07-12T08:00:00+08:00',
        lastSeen: '2026-07-12T08:00:00+08:00', salary: '30-40K', skills: ['RAG']
      },
      {
        ...mockJobs[2], id: 'previous', firstSeen: '2026-07-05T08:00:00+08:00',
        lastSeen: '2026-07-15T08:00:00+08:00', salary: '10-20K', skills: ['Python']
      },
      {
        ...mockJobs[3], id: 'invalid-date', firstSeen: 'not-a-date',
        lastSeen: '2026-07-16T08:00:00+08:00', salary: '50-60K', skills: ['Java']
      }
    ];

    const report = buildClientJobDataReport(jobs, [], new Date('2026-07-16T04:00:00Z'));
    const trend = report.trends.sevenDays;
    expect(trend.dailyNewJobs).toHaveLength(7);
    expect(trend.dailyNewJobs[0]).toEqual({ date: '2026-07-10', count: 1 });
    expect(trend.dailyNewJobs.some((point) => point.count === 0)).toBe(true);
    expect(trend.recentNewJobs).toBe(2);
    expect(trend.previousNewJobs).toBe(1);
    expect(trend.newJobsChangePercentage).toBe(100);
    expect(trend.recentlySeenExistingJobs).toBe(1);
    expect(trend.recentSalaryMedianK).toBe(30);
    expect(trend.previousSalaryMedianK).toBe(15);
    expect(trend.salaryMedianDeltaK).toBe(15);
    expect(trend.dateSampleCount).toBe(3);
    expect(trend.dateCoverage).toBe(75);
    expect(trend.skillChanges.find((item) => item.label === 'Python')?.deltaPercentagePoints).toBe(-50);
    expect(trend.skillChanges.find((item) => item.label === 'RAG')?.deltaPercentagePoints).toBe(50);
    expect(report.trends.thirtyDays.dailyNewJobs).toHaveLength(30);
  });

  it('marks a current-only trend as having no comparable previous sample', () => {
    const report = buildClientJobDataReport([
      { ...mockJobs[0], firstSeen: '2026-07-16T00:00:00+08:00', lastSeen: '2026-07-16T00:00:00+08:00' }
    ], [], new Date('2026-07-16T04:00:00Z'));

    expect(report.trends.sevenDays.previousNewJobs).toBe(0);
    expect(report.trends.sevenDays.newJobsChangePercentage).toBeNull();
  });
});
