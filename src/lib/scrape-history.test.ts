import { describe, expect, it } from 'vitest';
import { latestSuccessfulScrapeKeyword } from './scrape-history';
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

describe('latestSuccessfulScrapeKeyword', () => {
  it('returns empty when there is no completed scrape', () => {
    expect(latestSuccessfulScrapeKeyword([])).toBe('');
    expect(latestSuccessfulScrapeKeyword([run({ keyword: '失败尝试', completedAt: null })])).toBe(
      ''
    );
  });

  it('uses the newest completed non-empty keyword regardless of array order', () => {
    const runs = [
      run({ keyword: ' 数据分析 ', startedAt: '2026-07-12T08:00:00.000Z' }),
      run({ keyword: '财务会计', startedAt: '2026-07-11T08:00:00.000Z' }),
      run({ keyword: '   ', startedAt: '2026-07-13T08:00:00.000Z' })
    ];
    expect(latestSuccessfulScrapeKeyword(runs)).toBe('数据分析');
  });
});
