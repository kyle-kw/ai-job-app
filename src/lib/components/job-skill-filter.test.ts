import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import JobSkillFilter from './JobSkillFilter.svelte';

afterEach(cleanup);

describe('JobSkillFilter', () => {
  it('announces the active keyboard option through the search combobox', async () => {
    render(JobSkillFilter, {
      options: [
        { label: 'Python', count: 3 },
        { label: 'Rust', count: 2 }
      ]
    });

    await fireEvent.click(screen.getByRole('button', { name: '技能筛选' }));
    const search = screen.getByRole('combobox', { name: '搜索技能' });

    expect(search).toHaveAttribute('aria-controls', 'job-skill-options');
    expect(search).toHaveAttribute('aria-activedescendant', 'job-skill-option-0');
    expect(document.getElementById('job-skill-option-0')).toHaveTextContent('Python');

    await fireEvent.keyDown(search, { key: 'ArrowDown' });

    expect(search).toHaveAttribute('aria-activedescendant', 'job-skill-option-1');
    expect(document.getElementById('job-skill-option-1')).toHaveTextContent('Rust');
  });
});
