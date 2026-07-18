import { describe, expect, it } from 'vitest';
import { latestCompletedScrapeRun, latestNonEmptyScrapeRun } from './scrape-history';
import type { ScrapeRun } from './types';

const run = (overrides: Partial<ScrapeRun>): ScrapeRun => ({
  id: crypto.randomUUID(),
  keyword: 'AI Agent',
  city: '上海',
  totalSeen: 1,
  inserted: 1,
  updated: 0,
  startedAt: '2026-07-10T08:00:00.000Z',
  completedAt: '2026-07-10T08:10:00.000Z',
  ...overrides
});

describe('scrape history selectors', () => {
  it('returns null when there is no completed scrape with a keyword', () => {
    expect(latestCompletedScrapeRun([])).toBeNull();
    expect(latestCompletedScrapeRun([run({ completedAt: null })])).toBeNull();
    expect(latestCompletedScrapeRun([run({ keyword: '   ' })])).toBeNull();
  });

  it('selects the latest completed run regardless of array order or result count', () => {
    const runs = [
      run({ keyword: 'newest', totalSeen: 0, startedAt: '2026-07-12T08:00:00.000Z' }),
      run({ keyword: 'older', startedAt: '2026-07-11T08:00:00.000Z' })
    ];
    expect(latestCompletedScrapeRun(runs)?.keyword).toBe('newest');
  });

  it('skips completed zero-result runs for non-empty selection', () => {
    const runs = [
      run({ keyword: 'zero', totalSeen: 0, startedAt: '2026-07-13T08:00:00.000Z' }),
      run({ keyword: 'non-empty', totalSeen: 2, startedAt: '2026-07-12T08:00:00.000Z' })
    ];
    expect(latestNonEmptyScrapeRun(runs)?.keyword).toBe('non-empty');
  });

  it('keeps the first input when timestamps are equal', () => {
    const first = run({ keyword: 'first' });
    const second = run({ keyword: 'second' });
    expect(latestCompletedScrapeRun([first, second])).toBe(first);
  });
});
