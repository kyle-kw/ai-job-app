import { describe, expect, it } from 'vitest';
import { factsFromResumeContent, mergeResumeFacts, resumeFactGuidance } from './resume-facts';
import { mockResume } from './mock-data';

describe('resume facts', () => {
  it('extracts role-neutral facts from every structured resume section', () => {
    let id = 0;
    const resume = structuredClone(mockResume);
    resume.professionalSkills = [{ id: 'skills', label: '数据工具', items: ['SQL', 'Python'] }];
    resume.projects = [{ id: 'project', name: '增长分析', summary: '建立用户漏斗', startDate: '', endDate: '', highlights: ['留存提升 8%'] }];
    resume.certifications = [{ id: 'cert', name: '初级会计资格', issuer: '示例机构', date: '2024.01' }];
    const facts = factsFromResumeContent(resume, () => `fact-${++id}`);

    expect(facts.some((fact) => fact.category === 'skill' && fact.value === 'SQL')).toBe(true);
    expect(facts.some((fact) => fact.category === 'experience' && fact.value.includes('云帆科技'))).toBe(true);
    expect(facts.some((fact) => fact.category === 'project' && fact.value === '留存提升 8%')).toBe(true);
    expect(facts.some((fact) => fact.category === 'education' && fact.value.includes('浙江工业大学'))).toBe(true);
    expect(facts.some((fact) => fact.category === 'certification' && fact.value.includes('初级会计资格'))).toBe(true);
    expect(facts.every((fact) => !fact.confirmed && fact.confidence === 1)).toBe(true);
  });

  it('adds only normalized non-duplicates and preserves existing facts', () => {
    const existing = [{ id: 'existing', category: 'skill' as const, value: ' SQL ', source: '手工', confidence: 1, confirmed: true }];
    const candidates = [
      { id: 'duplicate', category: 'skill' as const, value: 'sql', source: '同步', confidence: 1, confirmed: false },
      { id: 'new', category: 'skill' as const, value: 'Power BI', source: '同步', confidence: 1, confirmed: false }
    ];
    const result = mergeResumeFacts(existing, candidates);
    expect(result.added).toBe(1);
    expect(result.facts[0]).toEqual(existing[0]);
    expect(result.facts[1].value).toBe('Power BI');
  });

  it('provides role-specific guidance without creating facts', () => {
    expect(resumeFactGuidance('data-analysis').examples.join(' ')).toContain('SQL');
    expect(resumeFactGuidance('finance-accounting').examples.join(' ')).toContain('月结');
  });
});
