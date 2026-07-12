import { describe, expect, it } from 'vitest';
import {
  filterJobs,
  matchesCompanyScaleFilter,
  matchesSalaryFilter,
  normalizeCompanyScale,
  parseSalaryRange
} from './job-filters';

describe('job salary filters', () => {
  it('parses BOSS monthly salary ranges and ignores bonus months', () => {
    expect(parseSalaryRange('25-40K·15薪')).toEqual({ min: 25, max: 40 });
    expect(parseSalaryRange('50K以上')).toEqual({ min: 50, max: Number.POSITIVE_INFINITY });
  });

  it('matches intersecting ranges and excludes negotiable or unsupported salaries', () => {
    expect(matchesSalaryFilter('25-40K·15薪', '406')).toBe(true);
    expect(matchesSalaryFilter('20-25K', '405')).toBe(true);
    expect(matchesSalaryFilter('25-40K', '405')).toBe(false);
    expect(matchesSalaryFilter('薪资面议', '406')).toBe(false);
    expect(matchesSalaryFilter('20-30元/时', '406')).toBe(false);
    expect(matchesSalaryFilter('薪资面议', '')).toBe(true);
  });
});

describe('company scale filters', () => {
  it('normalizes common BOSS aliases', () => {
    expect(normalizeCompanyScale('20人以下')).toBe('301');
    expect(normalizeCompanyScale('20–99 人')).toBe('302');
    expect(normalizeCompanyScale('100-500人')).toBe('303');
    expect(normalizeCompanyScale('1万人以上')).toBe('306');
    expect(matchesCompanyScaleFilter('1000-9999人', '305')).toBe(true);
  });
});

describe('combined local job filtering', () => {
  const jobs = [
    { title: 'AI Agent 工程师', company: '甲公司', skills: ['RAG'], salary: '25-40K', companyScale: '100-499人', isNew: true, fit: { overallScore: 82 } },
    { title: 'Java 工程师', company: '乙公司', skills: ['Java'], salary: '15-20K', companyScale: '1000-9999人', isNew: false, fit: { overallScore: 70 } }
  ];

  it('combines text, score, freshness, salary, and company scale conditions', () => {
    expect(filterJobs(jobs, { query: 'rag', minScore: 80, onlyNew: true, salary: '406', companyScale: '303' })).toEqual([jobs[0]]);
    expect(filterJobs(jobs, { query: '', minScore: 0, onlyNew: false, salary: '407', companyScale: '' })).toEqual([]);
  });
});
