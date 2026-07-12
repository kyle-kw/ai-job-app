import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mockResume } from '$lib/mock-data';
import ResumeFactsEditor from './ResumeFactsEditor.svelte';

function renderEditor(
  resume: typeof mockResume,
  listener: (event: CustomEvent<{ facts: typeof mockResume.facts }>) => void,
  hasUnsavedChanges = false
) {
  return render(ResumeFactsEditor, {
    resume,
    hasUnsavedChanges,
    $$events: { factschange: listener }
  } as never);
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('ResumeFactsEditor', () => {
  it('shows data-analysis guidance and adds a manual fact as pending by default', async () => {
    const resume = { ...structuredClone(mockResume), templateId: 'data-analysis' as const };
    const changes = vi.fn();
    renderEditor(resume, changes);

    expect(screen.getByText(/指标口径、数据规模/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '新增事实' }));
    await fireEvent.input(screen.getByLabelText('事实内容'), { target: { value: '使用 SQL 建立经营指标体系' } });
    await fireEvent.click(screen.getByRole('button', { name: '添加事实' }));

    const added = changes.mock.calls[0][0].detail.facts.at(-1);
    expect(added).toMatchObject({
      category: 'experience', value: '使用 SQL 建立经营指标体系',
      source: '用户手工维护', confidence: 1, confirmed: false
    });
  });

  it('resets confirmation after a semantic edit and keeps the resume body untouched', async () => {
    const resume = structuredClone(mockResume);
    const originalSummary = resume.summary;
    const changes = vi.fn();
    renderEditor(resume, changes);

    await fireEvent.click(screen.getAllByRole('button', { name: '编辑' })[0]);
    const confirmation = screen.getByRole('checkbox', { name: /我确认内容真实/ });
    expect(confirmation).toBeChecked();
    await fireEvent.input(screen.getByLabelText('事实内容'), { target: { value: 'RAG 检索命中率提升 25%' } });
    expect(confirmation).not.toBeChecked();
    await fireEvent.click(screen.getByRole('button', { name: '保存修改' }));

    const edited = changes.mock.calls[0][0].detail.facts.find((fact: { id: string }) => fact.id === 'fact-rag');
    expect(edited).toMatchObject({ value: 'RAG 检索命中率提升 25%', source: '用户手工维护', confirmed: false });
    expect(resume.summary).toBe(originalSummary);
  });

  it('supplements finance facts without duplicating or confirming them', async () => {
    const resume = {
      ...structuredClone(mockResume), templateId: 'finance-accounting' as const,
      professionalSkills: [{ id: 'finance', label: '财务系统与办公工具', items: ['Excel'] }],
      facts: [{ id: 'existing', category: 'skill' as const, value: 'excel', source: '手工', confidence: 1, confirmed: true }]
    };
    const changes = vi.fn();
    renderEditor(resume, changes);

    expect(screen.getByText(/核算主体、月结时效/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '从简历补全' }));

    const facts = changes.mock.calls[0][0].detail.facts;
    expect(facts.filter((fact: { category: string; value: string }) => fact.category === 'skill' && fact.value.toLowerCase() === 'excel')).toHaveLength(1);
    expect(facts.slice(1).every((fact: { confirmed: boolean }) => !fact.confirmed)).toBe(true);
    expect(facts.some((fact: { category: string }) => fact.category === 'education')).toBe(true);
  });

  it('deletes only the selected fact after confirmation', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const resume = structuredClone(mockResume);
    const changes = vi.fn();
    renderEditor(resume, changes, true);

    await fireEvent.click(screen.getByRole('button', { name: `删除事实：${resume.facts[0].value}` }));
    expect(changes.mock.calls[0][0].detail.facts).toHaveLength(resume.facts.length - 1);
    expect(changes.mock.calls[0][0].detail.facts.some((fact: { id: string }) => fact.id === resume.facts[0].id)).toBe(false);
  });
});
