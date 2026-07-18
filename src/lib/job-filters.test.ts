import { describe, expect, it } from 'vitest';
import {
  filterJobs,
  matchesCompanyScaleFilter,
  matchesReportSalaryBand,
  matchesSalaryFilter,
  normalizeCompanyScale,
  parseSalaryRange,
  sortJobs
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
    expect(matchesReportSalaryBand('50K以上', '50-plus')).toBe(true);
  });
});

describe('job sorting', () => {
  const jobs = [
    {
      id: 'low-new',
      title: 'A',
      company: 'A',
      skills: [],
      salary: '20-30K',
      companyScale: '',
      location: '',
      description: '',
      isNew: true,
      lastSeen: '2026-07-18',
      fit: { overallScore: 70 }
    },
    {
      id: 'high-old',
      title: 'B',
      company: 'B',
      skills: [],
      salary: '30-60K',
      companyScale: '',
      location: '',
      description: '',
      isNew: false,
      lastSeen: '2026-07-17',
      fit: { overallScore: 90 }
    },
    {
      id: 'unknown',
      title: 'C',
      company: 'C',
      skills: [],
      salary: '面议',
      companyScale: '',
      location: '',
      description: '',
      isNew: false,
      lastSeen: '2026-07-19',
      fit: { overallScore: 95 }
    }
  ];

  it('uses stable recommendation, recency, and salary midpoint orders', () => {
    expect(sortJobs(jobs, 'recommended').map((job) => job.id)).toEqual([
      'unknown',
      'high-old',
      'low-new'
    ]);
    expect(sortJobs(jobs, 'recent').map((job) => job.id)).toEqual([
      'unknown',
      'low-new',
      'high-old'
    ]);
    expect(sortJobs(jobs, 'salary-desc').map((job) => job.id)).toEqual([
      'high-old',
      'low-new',
      'unknown'
    ]);
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
    {
      title: 'AI Agent 工程师',
      company: '甲公司',
      skills: ['RAG', 'Python'],
      salary: '25-45K',
      companyScale: '100-499人',
      location: '上海·浦东新区',
      experience: '3-5年',
      description: '',
      isNew: true,
      fit: { overallScore: 82 }
    },
    {
      title: 'Java 工程师',
      company: '乙公司',
      skills: ['Java'],
      salary: '15-20K',
      companyScale: '1000-9999人',
      location: '杭州·余杭区',
      experience: '5-10年',
      description: '负责 Java 服务开发',
      isNew: false,
      fit: { overallScore: 70 }
    }
  ];

  it('combines text, score, freshness, salary, company scale, city, and missing description conditions', () => {
    expect(
      filterJobs(jobs, {
        query: 'rag',
        minScore: 80,
        onlyNew: true,
        salary: '406',
        companyScale: '303',
        city: '上海',
        missingDescription: true
      })
    ).toEqual([jobs[0]]);
    expect(
      filterJobs(jobs, {
        query: '',
        minScore: 0,
        onlyNew: false,
        salary: '',
        companyScale: '',
        city: '杭州',
        missingDescription: true
      })
    ).toEqual([]);
    expect(
      filterJobs(jobs, {
        query: '',
        minScore: 0,
        onlyNew: false,
        salary: '407',
        companyScale: '',
        city: '',
        missingDescription: false
      })
    ).toEqual([]);
  });

  it('uses AND for skills and combines exact experience with report salary midpoint bands', () => {
    const base = {
      query: '',
      minScore: 0,
      onlyNew: false,
      salary: '' as const,
      companyScale: '' as const,
      city: '',
      missingDescription: false
    };
    expect(
      filterJobs(jobs, {
        ...base,
        skills: ['RAG', 'Python'],
        experience: '3-5年',
        salaryBand: '35-50'
      })
    ).toEqual([jobs[0]]);
    expect(filterJobs(jobs, { ...base, skills: ['RAG', 'Java'] })).toEqual([]);
    expect(filterJobs(jobs, { ...base, salaryBand: '25-35' })).toEqual([]);
    expect(filterJobs(jobs, { ...base, salaryBand: '15-25' })).toEqual([jobs[1]]);
  });
});
