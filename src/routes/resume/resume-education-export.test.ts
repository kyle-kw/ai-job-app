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

  it('exports the preview layout with a temporary color theme without changing the structure template', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const saveResume = vi.spyOn(backend, 'saveResume').mockImplementation(async (resume) => ({
      ...structuredClone(resume), version: resume.version + 1, updatedAt: new Date().toISOString()
    }));
    const renderResume = vi.spyOn(backend, 'renderResume').mockResolvedValue({ path: 'demo.pdf', fileName: 'demo.pdf' });
    render(ResumePage);

    await fireEvent.input(screen.getByLabelText('职业标题'), { target: { value: 'AI 平台工程师' } });
    await fireEvent.click(screen.getByRole('button', { name: '导出 PDF' }));
    const dialog = screen.getByRole('dialog', { name: '选择颜色主题' });
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: /经典蓝/ })).toHaveAttribute('aria-pressed', 'true');
    await fireEvent.click(within(dialog).getByRole('button', { name: /松柏绿/ }));
    await fireEvent.click(screen.getByRole('button', { name: '选择保存位置' }));

    await waitFor(() => expect(saveResume).toHaveBeenCalled());
    expect(saveResume.mock.calls[0][0].templateId).toBe(mockSnapshot.resume?.templateId);
    await waitFor(() => expect(renderResume).toHaveBeenCalled());
    expect(renderResume.mock.calls[0][0].outputPath).toMatch(/^林知远-\d{8}-\d{6}\.pdf$/);
    expect(renderResume.mock.calls[0][0].colorTheme).toBe('pine');
  });
});
