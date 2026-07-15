import { afterEach, describe, expect, it, vi } from 'vitest';
import { backend } from '$lib/services/backend';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('support links', () => {
  it('opens the repository Issues page in browser mode', async () => {
    const open = vi.spyOn(window, 'open').mockImplementation(() => null);

    await backend.openGitHubIssues();

    expect(open).toHaveBeenCalledWith(
      'https://github.com/kyle-kw/ai-job-app/issues',
      '_blank',
      'noopener,noreferrer'
    );
  });
});
