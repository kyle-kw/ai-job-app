import { cleanup, render } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { mockResume } from '$lib/mock-data';
import { resumeTemplate } from '$lib/resume-templates';
import ResumePaper from './ResumePaper.svelte';

afterEach(cleanup);

describe('ResumePaper', () => {
  it('uses the reference navy preview theme by default', () => {
    const { container } = render(ResumePaper, {
      resume: mockResume,
      sections: resumeTemplate(mockResume.templateId).sectionOrder
    });

    const paper = container.querySelector('.resume-paper');
    expect(paper).toHaveAttribute('data-color-theme', 'navy');
    expect(paper).toHaveStyle('--resume-accent: #1F407A');
  });

  it('changes only the color theme independently of the structure template', () => {
    const resume = {
      ...structuredClone(mockResume),
      templateId: 'finance-accounting' as const,
      summary: '使用 Dify 与 FastAPI 交付内部 AI 服务。'
    };
    const { container } = render(ResumePaper, {
      resume,
      sections: resumeTemplate(resume.templateId).sectionOrder,
      colorTheme: 'pine'
    });

    const paper = container.querySelector('.resume-paper');
    expect(paper).toHaveAttribute('data-color-theme', 'pine');
    expect(paper).toHaveStyle('--resume-accent: #176B57');
    expect(paper).not.toHaveClass('theme-finance');
    expect(container.querySelector('strong')).toHaveTextContent('Dify');
  });
});
