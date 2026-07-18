import { describe, expect, it } from 'vitest';
import { mockJobs, mockResume } from '$lib/mock-data';
import { buildLocalResumeCoverage, coverageRequirements } from '$lib/resume-coverage';

describe('local resume coverage', () => {
  it('separates resume evidence, confirmed facts, real skill gaps and semantic unknowns', () => {
    const resume = structuredClone(mockResume);
    resume.professionalSkills = resume.professionalSkills.map((group) => ({
      ...group,
      items: group.items.filter((item) => item !== 'Docker')
    }));
    resume.experiences = resume.experiences.map((item) => ({
      ...item,
      highlights: item.highlights.map((value) => value.replace('Docker', '容器'))
    }));
    const report = buildLocalResumeCoverage(
      mockJobs[0],
      { kind: 'variant', id: 'variant' },
      resume
    );

    expect(report.items.find((item) => item.label === 'Python')?.status).toBe('covered');
    expect(report.items.find((item) => item.label === 'Docker')?.status).toBe('strengthenable');
    expect(
      report.items.some((item) => item.kind === 'requirement' && item.status === 'unknown')
    ).toBe(true);
  });

  it('does not count a longer ASCII token as an exact skill match', () => {
    const resume = structuredClone(mockResume);
    resume.summary = '负责 JavaScript 前端工程化。';
    resume.professionalSkills = [];
    resume.experiences = [];
    resume.projects = [];
    resume.education = [];
    resume.certifications = [];
    resume.facts = [];
    const job = {
      ...structuredClone(mockJobs[0]),
      structuredDetails: undefined,
      description: '',
      skills: ['Java']
    };

    const report = buildLocalResumeCoverage(job, { kind: 'variant', id: 'variant' }, resume);

    expect(report.items[0].status).toBe('gap');
  });

  it('uses the shared stable IDs, Unicode length rules and description cap', () => {
    const descriptions = Array.from({ length: 25 }, (_, index) => `第 ${index + 1} 项岗位能力要求`);
    descriptions[0] = '😀'.repeat(140);
    const job = {
      ...structuredClone(mockJobs[0]),
      structuredDetails: undefined,
      skills: ['Python', 'SQL'],
      description: descriptions.join('。')
    };

    const requirements = coverageRequirements(job);

    expect(requirements.filter((item) => item.kind === 'requirement')).toHaveLength(20);
    expect(requirements).toHaveLength(22);
    expect(requirements[0]).toMatchObject({
      id: 'requirement-512aae45ed67cf17',
      label: 'Python',
      kind: 'skill'
    });
    expect(requirements.some((item) => item.label === descriptions[0])).toBe(true);
  });
});
