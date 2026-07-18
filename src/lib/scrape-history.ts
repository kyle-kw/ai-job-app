import type { ScrapeRun } from '$lib/types';

function startedAtMillis(run: ScrapeRun): number {
  const value = Date.parse(run.startedAt);
  return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY;
}

function latestRun(
  runs: readonly ScrapeRun[],
  predicate: (run: ScrapeRun) => boolean
): ScrapeRun | null {
  return runs.reduce<ScrapeRun | null>((selected, run) => {
    if (!predicate(run)) return selected;
    if (!selected || startedAtMillis(run) > startedAtMillis(selected)) return run;
    return selected;
  }, null);
}

export function latestCompletedScrapeRun(runs: readonly ScrapeRun[]): ScrapeRun | null {
  return latestRun(runs, (run) => Boolean(run.completedAt) && Boolean(run.keyword.trim()));
}

export function latestNonEmptyScrapeRun(runs: readonly ScrapeRun[]): ScrapeRun | null {
  return latestRun(
    runs,
    (run) => Boolean(run.completedAt) && Boolean(run.keyword.trim()) && run.totalSeen > 0
  );
}
