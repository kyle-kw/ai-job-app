import { describe, expect, it } from 'vitest';
import { mockResume } from '$lib/mock-data';
import { analyzeResumeHealth } from '$lib/resume-health';

describe('resume health', () => {
  it('finds missing identity, invalid dates, duplicate skills and empty bullets locally', () => {
    const resume = structuredClone(mockResume);
    resume.name = '';
    resume.email = 'broken';
    resume.phone = '';
    resume.professionalSkills[1].items.push(' python ');
    resume.professionalSkills.push({ id: 'empty-group', label: '', items: [''] });
    resume.experiences[0].startDate = '2025-13';
    resume.experiences[0].highlights.push('');

    const report = analyzeResumeHealth(resume);
    expect(report.issues.map((item) => item.code)).toEqual(
      expect.arrayContaining([
        'missing-name',
        'invalid-email',
        'duplicate-skill',
        'invalid-date',
        'empty-highlight',
        'empty-record'
      ])
    );
    expect(report.errorCount).toBeGreaterThan(0);
  });

  it('accepts supported dates and confirmed numeric claims', () => {
    const resume = structuredClone(mockResume);
    resume.summary =
      '具备多年人工智能应用研发与后端工程经验，专注于可信的生产交付、跨团队协作和持续优化。';
    const report = analyzeResumeHealth(resume);
    expect(report.issues.find((item) => item.path === '/experiences/0/startDate')).toBeUndefined();
    expect(report.issues.find((item) => item.message.includes('23%'))).toBeUndefined();
  });
});
