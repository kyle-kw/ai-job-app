import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import ResumePage from './+page.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('resume nested list controls', () => {
  it('adds and removes skills with compact, accessible controls', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);

    const addSkill = screen.getByRole('button', { name: '添加技能到核心方向' });
    expect(addSkill).toHaveTextContent('');
    const initialSkillInputs = screen.getAllByRole('textbox', { name: /^技能 \d+\.\d+$/ }).length;

    await fireEvent.click(addSkill);
    expect(screen.getAllByRole('textbox', { name: /^技能 \d+\.\d+$/ })).toHaveLength(
      initialSkillInputs + 1
    );

    await fireEvent.click(screen.getByRole('button', { name: '删除技能：RAG' }));
    expect(screen.getByRole('dialog', { name: '确认删除简历内容' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: '确认删除' }));
    expect(screen.queryByDisplayValue('RAG')).not.toBeInTheDocument();
  });

  it('adds, reorders and removes work and project highlights', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);

    const originalSecondWorkHighlight = mockSnapshot.resume!.experiences[0].highlights[1];
    await fireEvent.click(screen.getAllByRole('button', { name: '下移经历成果 1' })[0]);
    expect(screen.getAllByPlaceholderText('经历成果')[0]).toHaveValue(originalSecondWorkHighlight);

    const initialWorkHighlightCount = screen.getAllByPlaceholderText('经历成果').length;
    const addWorkHighlight = screen.getByRole('button', { name: '添加经历成果 1' });
    expect(addWorkHighlight).toHaveTextContent('');
    await fireEvent.click(addWorkHighlight);
    expect(screen.getAllByPlaceholderText('经历成果')).toHaveLength(initialWorkHighlightCount + 1);

    await fireEvent.click(screen.getAllByRole('button', { name: '删除经历成果 1' })[0]);
    await fireEvent.click(screen.getByRole('button', { name: '确认删除' }));
    expect(screen.getAllByPlaceholderText('经历成果')).toHaveLength(initialWorkHighlightCount);

    await fireEvent.click(screen.getByRole('button', { name: '添加项目' }));
    const addProjectHighlight = screen.getByRole('button', { name: '添加项目成果 1' });
    expect(addProjectHighlight).toHaveTextContent('');
    await fireEvent.input(screen.getByLabelText('项目成果 1'), { target: { value: '项目成果 A' } });
    await fireEvent.click(addProjectHighlight);
    await fireEvent.input(screen.getByLabelText('项目成果 2'), { target: { value: '项目成果 B' } });
    await fireEvent.click(screen.getByRole('button', { name: '上移项目成果 2' }));
    expect(screen.getByLabelText('项目成果 1')).toHaveValue('项目成果 B');
    await fireEvent.click(screen.getByRole('button', { name: '删除项目成果 1' }));
    await fireEvent.click(screen.getByRole('button', { name: '确认删除' }));
    expect(screen.getByLabelText('项目成果 1')).toHaveValue('项目成果 A');
    expect(screen.queryByLabelText('项目成果 2')).not.toBeInTheDocument();
  });

  it('adds, reorders and removes education highlights', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);

    const addEducationHighlight = screen.getByRole('button', { name: '添加教育成果 1' });
    expect(addEducationHighlight).toHaveTextContent('');
    await fireEvent.click(addEducationHighlight);
    await fireEvent.input(screen.getByLabelText('教育成果 1'), { target: { value: '教育成果 A' } });
    await fireEvent.click(addEducationHighlight);
    await fireEvent.input(screen.getByLabelText('教育成果 2'), { target: { value: '教育成果 B' } });

    await fireEvent.click(screen.getByRole('button', { name: '上移教育成果 2' }));
    expect(screen.getByLabelText('教育成果 1')).toHaveValue('教育成果 B');
    await fireEvent.click(screen.getByRole('button', { name: '删除教育成果 1' }));
    await fireEvent.click(screen.getByRole('button', { name: '确认删除' }));
    expect(screen.getByLabelText('教育成果 1')).toHaveValue('教育成果 A');
    expect(screen.queryByLabelText('教育成果 2')).not.toBeInTheDocument();
  });
});

describe('resume editor viewport layout', () => {
  it('lets the two-column workspace consume only the height left below the variable header', () => {
    snapshot.set(structuredClone(mockSnapshot));
    const { container } = render(ResumePage);

    const page = container.querySelector('.page-content');
    const workspace = page?.querySelector(':scope > .grid.min-h-0.flex-1');

    expect(page).toHaveClass('flex', 'flex-col', 'overflow-hidden');
    expect(workspace).toBeInTheDocument();
    expect(workspace?.className).not.toContain('h-[calc(100%-94px)]');
  });
});
