import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';
import { shouldReloadAfterClear } from '$lib/clear-data';
import { snapshot } from '$lib/stores/app';
import PrivacyGate from './PrivacyGate.svelte';

const mocks = vi.hoisted(() => ({ downloadAndInstallUpdate: vi.fn() }));
vi.mock('$lib/services/backend', () => ({
  backend: { downloadAndInstallUpdate: mocks.downloadAndInstallUpdate }
}));

import UpdateDialog from './UpdateDialog.svelte';

afterEach(cleanup);

describe('public beta distribution UI', () => {
  beforeEach(() => {
    snapshot.set(structuredClone(mockSnapshot));
    mocks.downloadAndInstallUpdate.mockReset();
  });

  it('blocks first use until the user explicitly accepts or exits', async () => {
    const onAccept = vi.fn();
    const onExit = vi.fn();
    render(PrivacyGate, { accepting: false, onAccept, onExit });
    expect(screen.getByRole('dialog', { name: '隐私与使用说明' })).toHaveAttribute('aria-modal', 'true');
    expect(screen.getByText(/不会检查更新、访问 BOSS/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '同意并继续' }));
    await fireEvent.click(screen.getByRole('button', { name: '退出应用' }));
    expect(onAccept).toHaveBeenCalledOnce();
    expect(onExit).toHaveBeenCalledOnce();
  });

  it('protects active tasks from update installation', () => {
    snapshot.set({
      ...structuredClone(mockSnapshot),
      tasks: [{
        id: 'task', kind: 'fit', title: '分析', state: 'running', progress: 10,
        message: '运行中', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(), logs: []
      }]
    });
    render(UpdateDialog, {
      update: { version: '0.2.1', currentVersion: '0.2.0', notes: '安全更新', downloadSize: 10 * 1024 * 1024 },
      onLater: vi.fn()
    });
    expect(screen.getByText('安全更新')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '任务结束后更新' })).toBeDisabled();
  });

  it('renders updater download progress', async () => {
    mocks.downloadAndInstallUpdate.mockImplementation(async (onEvent) => {
      onEvent({ event: 'progress', downloaded: 5, total: 10 });
      throw new Error('test stop');
    });
    render(UpdateDialog, {
      update: { version: '0.2.1', currentVersion: '0.2.0', notes: '安全更新', downloadSize: 10 },
      onLater: vi.fn()
    });
    await fireEvent.click(screen.getByRole('button', { name: '下载并安装' }));
    await waitFor(() => expect(screen.getByText('50%')).toBeInTheDocument());
  });

  it('does not bootstrap a deleted database after clearing all data', () => {
    expect(shouldReloadAfterClear({
      complete: true,
      items: [{ item: 'applicationData', ok: true, message: '已清除' }],
      restartRequired: true
    })).toBe(false);
    expect(shouldReloadAfterClear({
      complete: true,
      items: [{ item: 'modelKeys', ok: true, message: '已清除' }],
      restartRequired: false
    })).toBe(true);
  });
});
