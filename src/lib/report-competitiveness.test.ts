import { describe, expect, it } from 'vitest';
import { mockResume } from '$lib/mock-data';
import { buildLocalReportCompetitiveness } from '$lib/report-competitiveness';

describe('local report competitiveness', () => {
  it('distinguishes resume coverage, confirmed-fact evidence, and real gaps', () => {
    const resume = structuredClone(mockResume);
    resume.summary = '使用 Python 构建后端服务。';
    resume.professionalSkills = [];
    resume.experiences = [];
    resume.projects = [];
    resume.education = [];
    resume.certifications = [];
    resume.facts = [
      { id: 'fact-k8s', category: 'skill', value: '在生产环境使用 Kubernetes', source: '用户确认', confidence: 1, confirmed: true },
      { id: 'fact-rust-unconfirmed', category: 'skill', value: '学习 Rust', source: '草稿', confidence: 0.5, confirmed: false }
    ];

    const result = buildLocalReportCompetitiveness([
      { label: 'Python', count: 8, percentage: 80 },
      { label: 'Kubernetes', count: 5, percentage: 50 },
      { label: 'Rust', count: 3, percentage: 30 }
    ], resume, '2026-07-16T12:00:00+08:00');

    expect(result.items.map((item) => item.status)).toEqual(['covered', 'strengthenable', 'gap']);
    expect(result.items[0].resumePaths).toContain('/summary');
    expect(result.items[1].evidenceFactIds).toEqual(['fact-k8s']);
    expect(result.items[2].evidenceFactIds).toEqual([]);
  });

  it('limits the matrix to the first twelve market skills', () => {
    const skills = Array.from({ length: 15 }, (_, index) => ({
      label: `Skill ${index + 1}`, count: 15 - index, percentage: 50 - index
    }));
    expect(buildLocalReportCompetitiveness(skills, mockResume).items).toHaveLength(12);
  });
});
