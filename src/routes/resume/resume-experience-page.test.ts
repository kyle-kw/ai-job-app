import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { backend } from '$lib/services/backend';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import ResumePage from './+page.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('resume work experience editing', () => {
  it('adds an editable experience and includes it in the saved resume', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const saveResume = vi.spyOn(backend, 'saveResume').mockImplementation(async (resume) => ({
      ...structuredClone(resume),
      version: resume.version + 1,
      updatedAt: new Date().toISOString()
    }));
    render(ResumePage);

    const initialCount = screen.getAllByLabelText('公司名称').length;
    await fireEvent.click(screen.getByRole('button', { name: '添加经历' }));

    const companies = screen.getAllByLabelText('公司名称');
    const positions = screen.getAllByLabelText('职位名称');
    const highlights = screen.getAllByPlaceholderText('经历成果');
    expect(companies).toHaveLength(initialCount + 1);

    await fireEvent.input(companies.at(-1)!, { target: { value: '远航科技' } });
    await fireEvent.input(positions.at(-1)!, { target: { value: '平台工程师' } });
    await fireEvent.input(highlights.at(-1)!, { target: { value: '搭建稳定的任务平台。' } });
    await fireEvent.click(screen.getByRole('button', { name: '保存修改' }));

    await waitFor(() => expect(saveResume).toHaveBeenCalledTimes(1));
    const added = saveResume.mock.calls[0][0].experiences.at(-1);
    expect(added).toMatchObject({
      company: '远航科技',
      position: '平台工程师',
      location: '',
      startDate: '',
      endDate: '',
      highlights: ['搭建稳定的任务平台。']
    });
    expect(added?.id).toEqual(expect.any(String));
  });

  it('deletes blank experiences immediately and confirms before deleting populated ones', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);

    const initialCount = screen.getAllByLabelText('公司名称').length;
    await fireEvent.click(screen.getByRole('button', { name: '添加经历' }));
    expect(screen.getAllByLabelText('公司名称')).toHaveLength(initialCount + 1);

    const deleteButtons = screen.getAllByRole('button', { name: /删除工作经历/ });
    await fireEvent.click(deleteButtons.at(-1)!);
    expect(screen.getAllByLabelText('公司名称')).toHaveLength(initialCount);
    expect(screen.queryByRole('dialog', { name: '确认删除简历内容' })).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: '删除工作经历：云帆科技' }));
    expect(screen.getByRole('dialog', { name: '确认删除简历内容' })).toBeInTheDocument();
    expect(screen.getAllByLabelText('公司名称')).toHaveLength(initialCount);
    await fireEvent.click(screen.getByRole('button', { name: '取消' }));
    expect(screen.queryByRole('dialog', { name: '确认删除简历内容' })).not.toBeInTheDocument();
    expect(screen.getAllByLabelText('公司名称')).toHaveLength(initialCount);

    await fireEvent.click(screen.getByRole('button', { name: '删除工作经历：云帆科技' }));
    await fireEvent.click(screen.getByRole('button', { name: '确认删除' }));
    expect(screen.getAllByLabelText('公司名称')).toHaveLength(initialCount - 1);
  });
});
