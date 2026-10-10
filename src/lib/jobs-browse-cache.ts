import { get, writable } from 'svelte/store';
import type { CompanyScaleFilterCode } from '$lib/job-filters';
import { availableProviderConfigs } from '$lib/provider-policy';
import type { BootstrapSnapshot, Job, JobSort, ScrapeRun } from '$lib/types';

export const JOB_LIST_SCROLL_SELECTOR = '.job-list';
export const JOB_DETAIL_SCROLL_SELECTOR = '.job-detail';
export const jobsBrowseHref = writable('/jobs');
export const jobsBrowseRevision = writable(0);

export interface JobsBrowseSnapshot {
  viewKey: string;
  href: string;
  dataKey: string;
  listValid: boolean;
  jobs: Job[];
  totalJobs: number;
  pendingDetailCount: number;
  nextCursor: string | null;
  selectedId: string;
  activeTab: 'description' | 'fit';
  scrollTop: number;
  detailScrollTop: number;
  shellScrollTop: number;
  query: string;
  minScore: number;
  onlyNew: boolean;
  companyScaleFilter: CompanyScaleFilterCode;
  missingDescription: boolean;
  sort: JobSort;
}

let snapshot: JobsBrowseSnapshot | null = null;
let recordedScrollTop = 0;
let recordedDetailScrollTop = 0;

export function invalidateJobsBrowseData(): number {
  const revision = get(jobsBrowseRevision) + 1;
  jobsBrowseRevision.set(revision);
  return revision;
}

export function jobsBrowseDataKey(state: BootstrapSnapshot): string {
  const tasks = state.tasks
    .filter((task) => ['scrape', 'job-detail-extraction', 'fit'].includes(task.kind))
    .map(({ id, kind, state, updatedAt }) => ({ id, kind, state, updatedAt }))
    .sort((left, right) => left.id.localeCompare(right.id));
  const resume = state.resume
    ? { id: state.resume.id, version: state.resume.version, updatedAt: state.resume.updatedAt }
    : null;
  const provider = availableProviderConfigs(state.providers).find((item) => item.isDefault);
  const providerKey = provider
    ? {
        id: provider.id,
        kind: provider.kind,
        baseUrl: provider.baseUrl,
        model: provider.model,
        verified: provider.verified,
        allowInsecureHttp: provider.allowInsecureHttp,
        reasoningEffort: provider.reasoningEffort ?? null,
        temperature: provider.temperature ?? null,
        maxTokens: provider.maxTokens ?? null
      }
    : null;
  const latestScrape = state.scrapeRuns.reduce<ScrapeRun | null>((latest, run) => {
    if (!latest || run.startedAt > latest.startedAt) return run;
    if (run.startedAt === latest.startedAt && run.id > latest.id) return run;
    return latest;
  }, null);
  return JSON.stringify({
    revision: get(jobsBrowseRevision),
    tasks,
    resume,
    provider: providerKey,
    scrape: latestScrape
      ? { id: latestScrape.id, completedAt: latestScrape.completedAt ?? null }
      : null
  });
}

export function jobsViewKeyFromSearch(searchParams: URLSearchParams): string {
  const job = searchParams.get('job')?.trim() ?? '';
  const from = searchParams.get('from') === 'report' ? 'report' : '';
  const keyword = [
    ...new Set(
      searchParams
        .getAll('keyword')
        .map((value) => value.trim())
        .filter(Boolean)
    )
  ];
  const skill = [
    ...new Set(
      searchParams
        .getAll('skill')
        .map((value) => value.trim())
        .filter(Boolean)
    )
  ];
  const city = searchParams.get('city')?.trim() ?? '';
  const experience = searchParams.get('experience')?.trim() ?? '';
  const salaryBand = searchParams.get('salaryBand') ?? '';
  const normalizedBand = ['under-15', '15-25', '25-35', '35-50', '50-plus'].includes(salaryBand)
    ? salaryBand
    : '';
  return JSON.stringify({
    job,
    from,
    keyword,
    skill,
    city,
    experience,
    salaryBand: normalizedBand
  });
}

export function saveJobsBrowseSnapshot(next: JobsBrowseSnapshot): void {
  snapshot = {
    ...next,
    jobs: next.jobs.slice()
  };
  jobsBrowseHref.set(next.href);
}

export function readJobsBrowseSnapshot(viewKey: string): JobsBrowseSnapshot | null {
  if (!snapshot || snapshot.viewKey !== viewKey) return null;
  return {
    ...snapshot,
    jobs: snapshot.jobs.slice()
  };
}

export function recordJobsBrowseScrollTop(top: number): void {
  recordedScrollTop = top;
}

export function jobsListScrollElement(root: ParentNode = document): HTMLElement | null {
  const node = root.querySelector(JOB_LIST_SCROLL_SELECTOR);
  return node instanceof HTMLElement ? node : null;
}

export function readJobsBrowseScrollTop(root: ParentNode = document): number {
  const node = jobsListScrollElement(root);
  return node ? node.scrollTop : recordedScrollTop;
}

export function restoreJobsListScrollTop(top: number, root: ParentNode = document): void {
  const node = jobsListScrollElement(root);
  if (node) {
    node.scrollTop = top;
    recordJobsBrowseScrollTop(node.scrollTop);
  }
}

export function recordJobsDetailScrollTop(top: number): void {
  recordedDetailScrollTop = top;
}

export function jobsDetailScrollElement(root: ParentNode = document): HTMLElement | null {
  const node = root.querySelector(JOB_DETAIL_SCROLL_SELECTOR);
  return node instanceof HTMLElement ? node : null;
}

export function readJobsDetailScrollTop(root: ParentNode = document): number {
  const node = jobsDetailScrollElement(root);
  return node ? node.scrollTop : recordedDetailScrollTop;
}

export function restoreJobsDetailScrollTop(top: number, root: ParentNode = document): void {
  const node = jobsDetailScrollElement(root);
  if (node) {
    node.scrollTop = top;
    recordJobsDetailScrollTop(node.scrollTop);
  }
}

export function clearJobsBrowseSnapshot(): void {
  snapshot = null;
  recordedScrollTop = 0;
  recordedDetailScrollTop = 0;
  jobsBrowseHref.set('/jobs');
}
