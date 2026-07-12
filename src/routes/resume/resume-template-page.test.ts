import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import ResumePage from './+page.svelte';

describe('resume template examples', () => {
  beforeEach(() => {
    const state = structuredClone(mockSnapshot);
    state.resume = null;
    state.readiness.resume = false;
    snapshot.set(state);
  });
  afterEach(cleanup);

  it('previews a complete example with a safety warning and closes it', async () => {
    render(ResumePage);
    const dataCard = screen.getByText('数据分析').closest('article');
    expect(dataCard).not.toBeNull();

    await fireEvent.click(within(dataCard!).getByRole('button', { name: '查看完整示例' }));

    expect(screen.getByRole('dialog', { name: '数据分析简历' })).toBeInTheDocument();
    expect(screen.getByText('示例内容，请勿直接用于投递。示例不会写入主简历或事实库。')).toBeInTheDocument();
    expect(screen.getByText(/星河零售（示例公司）/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('creates a blank role template instead of copying preview facts', async () => {
    render(ResumePage);
    const financeCard = screen.getByText('财务会计').closest('article');
    expect(financeCard).not.toBeNull();

    await fireEvent.click(within(financeCard!).getByRole('button', { name: '使用此模板' }));
    await waitFor(() => expect(get(snapshot).resume?.templateId).toBe('finance-accounting'));

    const created = get(snapshot).resume;
    expect(created?.name).toBe('');
    expect(created?.experiences).toEqual([]);
    expect(created?.projects).toEqual([]);
    expect(created?.certifications).toEqual([]);
    expect(created?.facts).toEqual([]);
    expect(created?.professionalSkills.every((group) => group.items.length === 0)).toBe(true);
    expect(screen.queryByText('远航制造（示例公司）')).not.toBeInTheDocument();
  });
});
