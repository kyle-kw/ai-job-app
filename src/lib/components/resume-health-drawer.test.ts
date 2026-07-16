import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ResumeHealthReport } from '$lib/types';
import ResumeHealthDrawer from './ResumeHealthDrawer.svelte';

afterEach(() => cleanup());

describe('ResumeHealthDrawer', () => {
  it('sends only the selected health issues to the reviewable AI flow', async () => {
    const report: ResumeHealthReport = {
      issues: [
        { id: 'name', code: 'missing-name', severity: 'error', path: '/name', label: '姓名', message: '请填写姓名。' },
        { id: 'summary', code: 'summary-length', severity: 'suggestion', path: '/summary', label: '个人简介', message: '建议补充个人简介。' }
      ],
      errorCount: 1,
      warningCount: 0,
      suggestionCount: 1
    };
    const optimize = vi.fn();
    render(ResumeHealthDrawer, { open: true, report, aiReady: true, $$events: { ai: optimize } } as never);

    await fireEvent.click(screen.getByRole('checkbox', { name: '选择体检问题：姓名' }));
    await fireEvent.click(screen.getByRole('button', { name: '请 AI 优化' }));

    expect(optimize).toHaveBeenCalledTimes(1);
    expect(optimize.mock.calls[0][0].detail.issues).toEqual([report.issues[1]]);
  });
});
