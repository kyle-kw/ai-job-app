import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ReportPage from './+page.svelte';

describe('full job data report page', () => {
  it('renders the all-jobs analysis sections', async () => {
    render(ReportPage);
    expect(await screen.findByText('技能需求与共现组合')).toBeInTheDocument();
    expect(screen.getByText('薪资与候选人门槛')).toBeInTheDocument();
    expect(screen.getByText('市场结构')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /导出 HTML/ })).toBeEnabled();
  });
});
