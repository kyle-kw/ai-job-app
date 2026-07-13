import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';

const mocks = vi.hoisted(() => ({ bootstrap: vi.fn(), onTaskEvent: vi.fn() }));
vi.mock('$lib/services/backend', () => ({
  backend: { bootstrap: mocks.bootstrap, onTaskEvent: mocks.onTaskEvent }
}));

import { appError, refresh, snapshot } from './app';

describe('app bootstrap refresh ordering', () => {
  beforeEach(() => {
    mocks.bootstrap.mockReset();
    appError.set('old error');
  });

  it('only applies the newest response and clears a previous error', async () => {
    let resolveFirst!: (value: typeof mockSnapshot) => void;
    let resolveSecond!: (value: typeof mockSnapshot) => void;
    const first = new Promise<typeof mockSnapshot>((resolve) => { resolveFirst = resolve; });
    const second = new Promise<typeof mockSnapshot>((resolve) => { resolveSecond = resolve; });
    mocks.bootstrap.mockReturnValueOnce(first).mockReturnValueOnce(second);
    const firstRefresh = refresh();
    const secondRefresh = refresh();
    resolveSecond({ ...structuredClone(mockSnapshot), settings: { ...mockSnapshot.settings, advancedMode: true } });
    await secondRefresh;
    resolveFirst({ ...structuredClone(mockSnapshot), settings: { ...mockSnapshot.settings, advancedMode: false } });
    await firstRefresh;
    expect(get(snapshot).settings.advancedMode).toBe(true);
    expect(get(appError)).toBeNull();
  });
});
