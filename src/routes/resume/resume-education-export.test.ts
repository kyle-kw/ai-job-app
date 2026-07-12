import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { backend } from '$lib/services/backend';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import ResumePage from './+page.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('resume education and PDF export', () => {
  it('adds, edits, and removes multiple education records', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);

    expect(screen.getAllByRole('button', { name: /删除教育经历/ })).toHaveLength(1);
    await fireEvent.click(screen.getByRole('button', { name: '添加教育经历' }));
    expect(screen.getAllByRole('button', { name: /删除教育经历/ })).toHaveLength(2);

    const degreeSelects = screen.getAllByLabelText('学历');
    await fireEvent.change(degreeSelects[1], { target: { value: '其他' } });
    expect(screen.getByPlaceholderText(/大专/)).toBeInTheDocument();

    await fireEvent.click(screen.getAllByRole('button', { name: /删除教育经历/ })[0]);
    expect(screen.getAllByRole('button', { name: /删除教育经历/ })).toHaveLength(1);
  });

  it('saves the selected template before rendering to the chosen filename', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const saveResume = vi.spyOn(backend, 'saveResume').mockImplementation(async (resume) => ({
      ...structuredClone(resume), version: resume.version + 1, updatedAt: new Date().toISOString()
    }));
    const renderResume = vi.spyOn(backend, 'renderResume').mockResolvedValue({ path: 'demo.pdf', fileName: 'demo.pdf' });
    render(ResumePage);

    await fireEvent.click(screen.getByRole('button', { name: '导出 PDF' }));
    const dialog = screen.getByRole('dialog', { name: '选择简历模板' });
    expect(dialog).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: /数据分析/ }));
    await fireEvent.click(screen.getByRole('button', { name: '选择保存位置' }));

    await waitFor(() => expect(saveResume).toHaveBeenCalled());
    expect(saveResume.mock.calls[0][0].templateId).toBe('data-analysis');
    await waitFor(() => expect(renderResume).toHaveBeenCalled());
    expect(renderResume.mock.calls[0][0].outputPath).toMatch(/^林知远-\d{8}-\d{6}\.pdf$/);
  });
});
