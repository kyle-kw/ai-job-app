import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mockJobs, mockResume } from '$lib/mock-data';
import { backend } from '$lib/services/backend';
import type { ResumeChatProposal, ResumeCommitResult, ResumeVersionDetail, ResumeVersionSummary } from '$lib/types';
import ResumeChatDialog from './ResumeChatDialog.svelte';
import ResumeVersionDrawer from './ResumeVersionDrawer.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('ResumeChatDialog', () => {
  it('directs the user to model settings when AI is not ready', () => {
    render(ResumeChatDialog, { open: true, resume: mockResume, aiReady: false });

    expect(screen.getByText('先配置并验证 AI 模型')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '前往模型设置' })).toHaveAttribute('href', '/settings');
  });

  it('keeps edits reviewable and applies only the selected proposal', async () => {
    const proposal: ResumeChatProposal = {
      proposalId: 'proposal-1',
      resumeId: mockResume.id,
      baseVersion: mockResume.version,
      job: { id: mockJobs[0].id, title: mockJobs[0].title, company: mockJobs[0].company },
      assistantMessage: '我整理了一版更精简的个人简介。',
      edits: [{
        id: 'edit-summary',
        path: '/summary',
        label: '个人简介',
        operation: 'replace',
        before: mockResume.summary,
        after: '专注 RAG 与 Agent 工程落地的 AI 应用研发工程师。',
        rationale: '压缩重复信息。',
        evidenceFactIds: ['fact-rag'],
        requiredFactCandidateIds: []
      }],
      factCandidates: [],
      warnings: []
    };
    const commit: ResumeCommitResult = {
      resume: { ...structuredClone(mockResume), summary: String(proposal.edits[0].after), version: mockResume.version + 1 },
      version: {
        id: 'version-4', resumeId: mockResume.id, version: mockResume.version + 1,
        parentVersion: mockResume.version, createdAt: new Date().toISOString(), source: 'ai-chat', summary: 'AI 修改'
      }
    };
    const propose = vi.spyOn(backend, 'proposeResumeChatEdits').mockResolvedValue(proposal);
    const apply = vi.spyOn(backend, 'applyResumeChatEdits').mockResolvedValue(commit);

    render(ResumeChatDialog, { open: true, resume: mockResume, aiReady: true, initialJobId: mockJobs[0].id });
    await fireEvent.input(screen.getByRole('textbox', { name: '发送给简历 AI 的消息' }), { target: { value: '帮我精简个人简介' } });
    await fireEvent.click(screen.getByRole('button', { name: '发送' }));

    await waitFor(() => expect(propose).toHaveBeenCalledWith(expect.objectContaining({
      resumeId: mockResume.id,
      expectedVersion: mockResume.version,
      jobId: mockJobs[0].id
    })));
    expect(await screen.findByText('修改前')).toBeInTheDocument();
    expect(screen.getByText('修改后')).toBeInTheDocument();
    expect(screen.getByText(String(proposal.edits[0].after))).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: '应用所选修改' }));
    await waitFor(() => expect(apply).toHaveBeenCalledWith({
      proposal,
      selectedEditIds: ['edit-summary'],
      confirmedFactCandidateIds: [],
      expectedVersion: mockResume.version
    }));
  });
});

describe('ResumeVersionDrawer', () => {
  it('loads an immutable version and restores it as a new version', async () => {
    const current: ResumeVersionSummary = {
      id: 'version-3', resumeId: mockResume.id, version: 3, parentVersion: 2,
      createdAt: '2026-07-11T08:00:00.000Z', source: 'manual', summary: '当前版本'
    };
    const older: ResumeVersionSummary = {
      id: 'version-2', resumeId: mockResume.id, version: 2, parentVersion: 1,
      createdAt: '2026-07-10T08:00:00.000Z', source: 'import', summary: '首次导入'
    };
    const details = new Map<string, ResumeVersionDetail>([
      [current.id, { ...current, profile: structuredClone(mockResume) }],
      [older.id, { ...older, profile: { ...structuredClone(mockResume), version: 2, summary: '旧版简介' } }]
    ]);
    const restored: ResumeCommitResult = {
      resume: { ...structuredClone(mockResume), version: 4, summary: '旧版简介' },
      version: {
        id: 'version-4', resumeId: mockResume.id, version: 4, parentVersion: 3,
        createdAt: '2026-07-11T09:00:00.000Z', source: 'rollback', summary: '恢复到 v2', restoredFromVersion: 2
      }
    };
    vi.spyOn(backend, 'listResumeVersions').mockResolvedValue([current, older]);
    vi.spyOn(backend, 'getResumeVersion').mockImplementation(async (id) => details.get(id) ?? { ...restored.version, profile: restored.resume });
    const restore = vi.spyOn(backend, 'restoreResumeVersion').mockResolvedValue(restored);
    vi.spyOn(window, 'confirm').mockReturnValue(true);

    render(ResumeVersionDrawer, { open: true, resume: mockResume, hasUnsavedChanges: false });
    expect(await screen.findByText('首次导入', { exact: false })).toBeInTheDocument();
    await fireEvent.click(screen.getByText('版本 2'));
    expect(await screen.findByText('旧版简介')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '恢复为新版本' }));

    await waitFor(() => expect(restore).toHaveBeenCalledWith('version-2', mockResume.version));
  });
});
