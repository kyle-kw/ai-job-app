import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import TaskDrawer from './TaskDrawer.svelte';

afterEach(cleanup);

describe('TaskDrawer accessibility', () => {
  it('exposes modal semantics and closes with Escape', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(TaskDrawer, { open: true });
    expect(screen.getByRole('dialog', { name: '任务中心' })).toHaveAttribute('aria-modal', 'true');
    await fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '任务中心' })).not.toBeInTheDocument());
  });
});
