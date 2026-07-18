import { describe, expect, it } from 'vitest';
import {
  flattenProfessionalSkills,
  resumeTemplate,
  suggestedProfessionalSkillGroups
} from './resume-templates';
import { mockResume } from './mock-data';

describe('resume templates', () => {
  it('keeps role-specific section ordering deterministic', () => {
    expect(resumeTemplate('ai-engineering').sectionOrder.slice(0, 3)).toEqual([
      'summary',
      'professionalSkills',
      'projects'
    ]);
    expect(resumeTemplate('data-analysis').sectionOrder.slice(0, 3)).toEqual([
      'summary',
      'professionalSkills',
      'experiences'
    ]);
    expect(resumeTemplate('finance-accounting').sectionOrder.slice(0, 3)).toEqual([
      'summary',
      'experiences',
      'certifications'
    ]);
    expect(resumeTemplate('general').sectionOrder.slice(0, 3)).toEqual([
      'summary',
      'experiences',
      'professionalSkills'
    ]);
  });

  it('provides complete read-only examples only for the two role templates', () => {
    const data = resumeTemplate('data-analysis');
    const finance = resumeTemplate('finance-accounting');

    expect(data.sample?.templateId).toBe('data-analysis');
    expect(data.sample?.experiences).toHaveLength(2);
    expect(data.sample?.projects).toHaveLength(1);
    expect(data.sample?.certifications).toHaveLength(1);
    expect(data.sample?.experiences[0].company).toContain('示例公司');
    expect(data.sample?.experiences[0].highlights.join(' ')).toContain('60%');

    expect(finance.sample?.templateId).toBe('finance-accounting');
    expect(finance.sample?.experiences).toHaveLength(2);
    expect(finance.sample?.projects).toHaveLength(1);
    expect(finance.sample?.certifications[0].name).toContain('示例');
    expect(finance.sample?.experiences[0].highlights.join(' ')).toContain('7 天缩短至 4 天');

    expect(resumeTemplate('ai-engineering').sample).toBeUndefined();
    expect(resumeTemplate('general').sample).toBeUndefined();
  });

  it('keeps blank template skill groups free of sample facts', () => {
    expect(
      suggestedProfessionalSkillGroups('data-analysis').every((group) => group.items.length === 0)
    ).toBe(true);
    expect(
      suggestedProfessionalSkillGroups('finance-accounting').every(
        (group) => group.items.length === 0
      )
    ).toBe(true);
  });

  it('flattens grouped professional skills without duplicate facts', () => {
    const resume = structuredClone(mockResume);
    resume.professionalSkills.push({
      id: 'duplicate',
      label: '重复',
      items: ['python', '  Docker  ']
    });
    expect(flattenProfessionalSkills(resume)).toEqual([
      'LangChain',
      'RAG',
      'Python',
      'FastAPI',
      'PostgreSQL',
      'Redis',
      'TypeScript',
      'Docker'
    ]);
  });
});
