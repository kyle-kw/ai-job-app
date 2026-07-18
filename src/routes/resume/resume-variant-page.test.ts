import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { backend } from '$lib/services/backend';
import { mockJobs, mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import type { ResumeVariantDetail } from '$lib/types';
import ResumePage from './+page.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('resume variant page', () => {
  it('creates one job-bound copy without AI and saves it independently', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    const master = structuredClone(mockSnapshot.resume!);
    const now = new Date().toISOString();
    const detail: ResumeVariantDetail = {
      id: 'variant-1',
      jobId: mockJobs[0].id,
      jobTitle: mockJobs[0].title,
      company: mockJobs[0].company,
      name: `${mockJobs[0].company} · ${mockJobs[0].title}`,
      baseResumeId: master.id,
      baseResumeVersion: master.version,
      version: 1,
      createdAt: now,
      updatedAt: now,
      stale: false,
      profile: { ...structuredClone(master), id: 'variant-1', version: 1, updatedAt: now }
    };
    const { profile: _profile, ...summary } = detail;
    let created = false;
    vi.spyOn(backend, 'listResumeVariants').mockImplementation(async () =>
      created ? [summary] : []
    );
    vi.spyOn(backend, 'listJobOptions').mockResolvedValue([
      {
        id: mockJobs[0].id,
        title: mockJobs[0].title,
        company: mockJobs[0].company,
        lastSeen: mockJobs[0].lastSeen
      }
    ]);
    vi.spyOn(backend, 'getResumeVariant').mockResolvedValue(detail);
    vi.spyOn(backend, 'getJob').mockResolvedValue(mockJobs[0]);
    const create = vi.spyOn(backend, 'createResumeVariant').mockImplementation(async () => {
      created = true;
      return detail;
    });
    const propose = vi.spyOn(backend, 'proposeResumeChatEdits');
    const save = vi.spyOn(backend, 'saveResumeVariant').mockImplementation(async (_id, resume) => ({
      variant: { ...detail, version: 2, profile: { ...structuredClone(resume), version: 2 } },
      version: {
        id: 'version-2',
        resumeId: detail.id,
        version: 2,
        parentVersion: 1,
        createdAt: now,
        source: 'variant-manual',
        summary: '手工保存岗位版本'
      }
    }));
    const renderPdf = vi
      .spyOn(backend, 'renderResume')
      .mockResolvedValue({ path: 'variant.pdf', fileName: 'variant.pdf' });

    render(ResumePage);
    await fireEvent.click(screen.getByRole('button', { name: '岗位版本' }));
    await waitFor(() => expect(screen.getByText('创建第一份岗位定制简历')).toBeInTheDocument());
    await fireEvent.change(screen.getByLabelText('选择岗位版本目标'), {
      target: { value: mockJobs[0].id }
    });
    await fireEvent.click(screen.getByRole('button', { name: '创建岗位版本' }));
    await waitFor(() => expect(create).toHaveBeenCalledWith(mockJobs[0].id, master.version));
    expect(propose).not.toHaveBeenCalled();

    await waitFor(() => expect(screen.getByLabelText('职业标题')).toBeInTheDocument());
    await fireEvent.input(screen.getByLabelText('职业标题'), {
      target: { value: '专岗 AI 平台工程师' }
    });
    await fireEvent.click(screen.getByRole('button', { name: '保存修改' }));
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        detail.id,
        expect.objectContaining({ headline: '专岗 AI 平台工程师' }),
        1
      )
    );
    expect(master.headline).toBe(mockSnapshot.resume?.headline);

    await fireEvent.click(screen.getByRole('button', { name: '导出 PDF' }));
    await fireEvent.click(screen.getByRole('button', { name: '选择保存位置' }));
    await waitFor(() =>
      expect(renderPdf).toHaveBeenCalledWith(
        expect.objectContaining({
          target: { kind: 'variant', id: detail.id }
        })
      )
    );

    await fireEvent.click(screen.getByRole('button', { name: '求职偏好 · 只读' }));
    expect(screen.getByText('来自主简历基线的只读偏好')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '保存求职偏好' })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: /事实清单 · 只读/ }));
    expect(screen.getByText('来自主简历基线的只读事实')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: '新建岗位版本' }));
    expect(await screen.findByText('创建新的岗位定制简历')).toBeInTheDocument();
  });
});
