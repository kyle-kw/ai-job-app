import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import ReportTrendChart from './ReportTrendChart.svelte';

describe('report trend chart', () => {
  afterEach(cleanup);

  it('shows readable axis labels, summary statistics, and focusable data points', async () => {
    render(ReportTrendChart, { points: [
      { date: '2026-07-10', count: 0 },
      { date: '2026-07-11', count: 2 },
      { date: '2026-07-12', count: 5 }
    ] });

    expect(screen.getByText('7月10日')).toBeInTheDocument();
    expect(screen.getByText('7月12日')).toBeInTheDocument();
    expect(screen.getByText(/窗口合计/)).toHaveTextContent('7');
    expect(screen.getByText(/日均/)).toHaveTextContent('2.3');
    expect(screen.getByText(/峰值/)).toHaveTextContent('5 个 · 7月12日');

    const peak = screen.getByRole('button', { name: '7月12日新增 5 个岗位' });
    await fireEvent.focus(peak);
    expect(screen.getByRole('status')).toHaveTextContent('7月12日 5 个新增岗位');
  });

  it('uses a clear empty state when the window has no new jobs', () => {
    render(ReportTrendChart, { points: [
      { date: '2026-07-10', count: 0 },
      { date: '2026-07-11', count: 0 }
    ] });

    expect(screen.getByText('当前窗口暂无新增岗位')).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });
});
