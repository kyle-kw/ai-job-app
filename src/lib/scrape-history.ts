import type { ScrapeRun } from '$lib/types';

function startedAtMillis(run: ScrapeRun): number {
  const value = Date.parse(run.startedAt);
  return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY;
}

export function latestSuccessfulScrapeKeyword(runs: readonly ScrapeRun[]): string {
  const latest = runs
    .filter((run) => Boolean(run.completedAt) && Boolean(run.keyword.trim()))
    .reduce<ScrapeRun | null>((selected, run) => {
      if (!selected || startedAtMillis(run) > startedAtMillis(selected)) return run;
      return selected;
    }, null);

  return latest?.keyword.trim() ?? '';
}
