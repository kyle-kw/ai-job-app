import { describe, expect, it } from 'vitest';
import { mockJobs } from '$lib/mock-data';
import { buildClientJobDataReport, parseMonthlySalary } from '$lib/report';

describe('job data report', () => {
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
});
