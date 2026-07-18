import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { backend } from '$lib/services/backend';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import ResumePage from './+page.svelte';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('resume fact page integration', () => {
  it('saves fact confirmation changes through the versioned resume API', async () => {
    const state = structuredClone(mockSnapshot);
    snapshot.set(state);
    const saveResume = vi.spyOn(backend, 'saveResume').mockImplementation(async (resume) => ({
      ...structuredClone(resume),
      version: resume.version + 1,
      updatedAt: new Date().toISOString()
    }));

    render(ResumePage);
    await fireEvent.click(screen.getByRole('button', { name: /事实清单/ }));
    const confirmations = await screen.findAllByRole('checkbox', { name: /我确认这条事实真实/ });
    await fireEvent.click(confirmations[0]);
    await fireEvent.click(screen.getByRole('button', { name: '保存事实清单' }));

    await waitFor(() => expect(saveResume).toHaveBeenCalled());
    const saved = saveResume.mock.calls[0][0];
    expect(saved.facts[0].confirmed).toBe(false);
    expect(saved.version).toBe(state.resume?.version);
  });
});
