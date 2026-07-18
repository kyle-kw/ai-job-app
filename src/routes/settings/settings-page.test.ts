import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { backend } from '$lib/services/backend';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import SettingsPage from './+page.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function readySnapshot() {
  const value = structuredClone(mockSnapshot);
  value.providers[0].apiKeyRef = 'keychain://provider-xiaomi';
  return value;
}

describe('settings page', () => {
  it('opens GitHub Issues from the support action', async () => {
    snapshot.set(readySnapshot());
    const openGitHubIssues = vi.spyOn(backend, 'openGitHubIssues').mockResolvedValue();
    render(SettingsPage);

    await fireEvent.click(screen.getByRole('button', { name: 'GitHub Issues 支持' }));

    expect(openGitHubIssues).toHaveBeenCalledTimes(1);
  });

  it('does not start BOSS Chrome when the page opens', async () => {
    snapshot.set(readySnapshot());
    const setupBoss = vi.spyOn(backend, 'setupBoss');
    const getAppInfo = vi.spyOn(backend, 'getAppInfo');
    render(SettingsPage);

    await waitFor(() => expect(getAppInfo).toHaveBeenCalledTimes(1));
    expect(setupBoss).not.toHaveBeenCalled();
  });

  it('shows test-only guidance after testing and saved guidance after saving', async () => {
    snapshot.set(readySnapshot());
    vi.spyOn(backend, 'bootstrap').mockResolvedValue(readySnapshot());
    vi.spyOn(backend, 'testProvider').mockResolvedValue({
      ok: true,
      message: '连接成功，结构化输出正常',
      latencyMs: 120,
      structuredOutput: true,
      visionSupported: true,
      visionMessage: '图片识别能力正常'
    });
    vi.spyOn(backend, 'saveProvider').mockResolvedValue({
      providers: readySnapshot().providers,
      testResult: {
        ok: true,
        message: '连接成功，结构化输出正常',
        latencyMs: 120,
        structuredOutput: true,
        visionSupported: true,
        visionMessage: '图片识别能力正常'
      }
    });
    render(SettingsPage);

    await fireEvent.click(screen.getByRole('button', { name: '测试连接' }));
    expect(
      await screen.findByText('本次仅测试连接；点击“验证并保存”后配置才会生效。')
    ).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: '验证并保存' }));
    expect(await screen.findByText('配置已保存并生效。')).toBeInTheDocument();
    expect(
      screen.queryByText('本次仅测试连接；点击“验证并保存”后配置才会生效。')
    ).not.toBeInTheDocument();
  });

  it('persists disabling automatic update checks while keeping manual checks available', async () => {
    snapshot.set(readySnapshot());
    const saveSettings = vi
      .spyOn(backend, 'saveSettings')
      .mockImplementation(async (settings) => structuredClone(settings));
    render(SettingsPage);

    const automaticChecks = screen.getByRole('checkbox', { name: '自动检查更新' });
    expect(automaticChecks).toBeChecked();
    await fireEvent.click(automaticChecks);
    expect(
      screen.getByText('自动检查已关闭。你仍可在“关于与诊断”中手动检查更新。')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '检查更新' })).toBeEnabled();
    await waitFor(() =>
      expect(saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({ automaticUpdateChecks: false })
      )
    );
    expect(screen.queryByRole('button', { name: '保存设置' })).not.toBeInTheDocument();
  });

  it('automatically saves advanced mode and rolls the switch back when saving fails', async () => {
    const ready = readySnapshot();
    ready.settings.advancedMode = false;
    snapshot.set(ready);
    const saveSettings = vi
      .spyOn(backend, 'saveSettings')
      .mockRejectedValueOnce(new Error('写入失败'))
      .mockImplementation(async (settings) => structuredClone(settings));
    render(SettingsPage);

    const advancedMode = screen.getByRole('checkbox', { name: '高级模式' });
    expect(advancedMode).not.toBeChecked();
    await fireEvent.click(advancedMode);

    await waitFor(() =>
      expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ advancedMode: true }))
    );
    await waitFor(() => expect(advancedMode).not.toBeChecked());
    expect(await screen.findByText('自动保存失败：写入失败')).toBeInTheDocument();

    await fireEvent.click(advancedMode);
    await waitFor(() => expect(advancedMode).toBeChecked());
    await waitFor(() =>
      expect(saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ advancedMode: true }))
    );
  });

  it('uses in-app confirmation dialogs for all four data clearing actions', async () => {
    const ready = readySnapshot();
    snapshot.set(ready);
    vi.spyOn(backend, 'bootstrap').mockResolvedValue(ready);
    vi.spyOn(backend, 'getAppInfo').mockResolvedValue({
      version: '0.2.0',
      identifier: 'io.github.aijobapp',
      os: 'windows',
      arch: 'x86_64',
      webview: 'test',
      schemaVersion: 5,
      sidecarProtocol: '2',
      chrome: { installed: true, version: 'test', executablePath: 'chrome.exe' },
      dataDir: 'test-data',
      legacyDataDetected: true,
      lastUpdateCheckAt: new Date(2026, 6, 15, 11, 26, 46).toISOString()
    });
    vi.spyOn(backend, 'listAutomaticBackups').mockResolvedValue([]);
    const clearData = vi.spyOn(backend, 'clearData').mockImplementation(async (scope) => ({
      complete: true,
      items: [{ item: scope, ok: true, message: '已清除' }],
      restartRequired: scope === 'all'
    }));
    render(SettingsPage);
    await waitFor(() => expect(screen.getByRole('button', { name: '删除旧版遗留' })).toBeEnabled());
    expect(screen.getByText('2026-07-15 11:26:46')).toBeInTheDocument();
    expect(screen.queryByText('数据库 schema')).not.toBeInTheDocument();
    expect(screen.queryByText('旧版遗留数据')).not.toBeInTheDocument();

    const actions = [
      {
        button: '清除模型密钥',
        title: '确认清除模型密钥',
        confirm: '确认清除',
        scope: 'modelKeys'
      },
      {
        button: '清除 BOSS 数据',
        title: '确认清除 BOSS 登录数据',
        confirm: '确认清除',
        scope: 'bossProfile'
      },
      {
        button: '删除旧版遗留',
        title: '确认删除旧版遗留数据',
        confirm: '确认删除',
        scope: 'legacyData'
      },
      {
        button: '清除全部数据',
        title: '确认清除全部应用数据',
        confirm: '确认全部清除',
        scope: 'all'
      }
    ] as const;

    for (const [index, action] of actions.entries()) {
      const trigger = screen.getByRole('button', { name: action.button });
      await waitFor(() => expect(trigger).toBeEnabled());
      await fireEvent.click(trigger);
      let dialog = screen.getByRole('dialog', { name: action.title });
      expect(clearData).toHaveBeenCalledTimes(index);
      await fireEvent.click(within(dialog).getByRole('button', { name: '取消' }));
      expect(screen.queryByRole('dialog', { name: action.title })).not.toBeInTheDocument();

      await fireEvent.click(screen.getByRole('button', { name: action.button }));
      dialog = screen.getByRole('dialog', { name: action.title });
      await fireEvent.click(within(dialog).getByRole('button', { name: action.confirm }));
      await waitFor(() => expect(clearData).toHaveBeenNthCalledWith(index + 1, action.scope));
      await waitFor(() =>
        expect(screen.queryByRole('dialog', { name: action.title })).not.toBeInTheDocument()
      );
      await waitFor(() => expect(trigger).toBeEnabled());
    }
  });

  it('refreshes the successful local update-check time after a manual check', async () => {
    snapshot.set(readySnapshot());
    const checkedAt = new Date(2026, 6, 15, 11, 26, 46).toISOString();
    const baseInfo = {
      version: '0.2.0',
      identifier: 'io.github.aijobapp',
      os: 'windows',
      arch: 'x86_64',
      webview: 'test',
      schemaVersion: 5,
      sidecarProtocol: '2',
      chrome: { installed: true, version: 'test', executablePath: 'chrome.exe' },
      dataDir: 'test-data',
      legacyDataDetected: false
    };
    const getAppInfo = vi
      .spyOn(backend, 'getAppInfo')
      .mockResolvedValueOnce({ ...baseInfo, lastUpdateCheckAt: null })
      .mockResolvedValue({ ...baseInfo, lastUpdateCheckAt: checkedAt });
    vi.spyOn(backend, 'listAutomaticBackups').mockResolvedValue([]);
    vi.spyOn(backend, 'checkForUpdate').mockResolvedValue(null);
    render(SettingsPage);
    await waitFor(() => expect(getAppInfo).toHaveBeenCalledTimes(1));
    expect(await screen.findByText('尚未检查')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: '检查更新' }));

    await waitFor(() => expect(getAppInfo).toHaveBeenCalledTimes(2));
    expect(await screen.findByText('2026-07-15 11:26:46')).toBeInTheDocument();
  });
});
