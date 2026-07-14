import { derived, get, writable } from 'svelte/store';
import { backend } from '$lib/services/backend';
import type { AppSettings, BootstrapSnapshot, ImportResumePayload, JobPreferences, SearchSpec, TaskEvent } from '$lib/types';

const empty: BootstrapSnapshot = {
  readiness: { ai: false, resume: false, boss: false },
  configuration: {
    boss: { state: 'needs_setup', message: '需要配置 BOSS 专用浏览器。' },
    llm: { state: 'needs_setup', message: '需要配置默认模型。' }
  },
  resume: null, providers: [], tasks: [], scrapeRuns: [],
  settings: { advancedMode: false, automaticUpdateChecks: true, privacyAcknowledgedVersion: null, lastUpdateCheckAt: null }
};

export const snapshot = writable<BootstrapSnapshot>(empty);
export const loading = writable(true);
export const appError = writable<string | null>(null);
let unlisten: (() => void) | null = null;
let bootstrapRequestId = 0;

export const runningTasks = derived(snapshot, ($snapshot) => $snapshot.tasks.filter((task) => task.state === 'queued' || task.state === 'running'));
export const completedTasks = derived(snapshot, ($snapshot) => $snapshot.tasks.filter((task) => task.state === 'completed' || task.state === 'failed'));

function mergeTask(event: TaskEvent) {
  snapshot.update((value) => {
    const index = value.tasks.findIndex((task) => task.id === event.id);
    const tasks = [...value.tasks];
    if (index >= 0) tasks[index] = event;
    else tasks.unshift(event);
    return { ...value, tasks };
  });
  if (event.state === 'completed' || event.state === 'failed' || event.state === 'cancelled') void refresh();
}

export async function initialize() {
  const requestId = ++bootstrapRequestId;
  loading.set(true);
  appError.set(null);
  try {
    const value = await backend.bootstrap();
    if (requestId !== bootstrapRequestId) return;
    snapshot.set(value);
    appError.set(null);
    unlisten?.();
    unlisten = await backend.onTaskEvent(mergeTask);
  } catch (error) {
    if (requestId === bootstrapRequestId) appError.set(error instanceof Error ? error.message : String(error));
  } finally {
    if (requestId === bootstrapRequestId) loading.set(false);
  }
}

export async function refresh() {
  const requestId = ++bootstrapRequestId;
  try {
    const value = await backend.bootstrap();
    if (requestId !== bootstrapRequestId) return;
    snapshot.set(value);
    appError.set(null);
  } catch (error) {
    if (requestId === bootstrapRequestId) appError.set(error instanceof Error ? error.message : String(error));
  }
}

export async function startScrape(spec: SearchSpec) {
  await backend.startScrape(spec);
  await refresh();
}

export async function setupBoss(options: { resetProfile: boolean }) {
  await backend.setupBoss(options);
  await refresh();
}

export async function importResume(payload: ImportResumePayload) {
  await backend.importResume(payload);
  await refresh();
}

export async function savePreferences(preferences: JobPreferences) {
  const resume = await backend.savePreferences(preferences);
  snapshot.update((value) => ({ ...value, resume }));
}

export async function saveSettings(settings: AppSettings) {
  const saved = await backend.saveSettings(settings);
  snapshot.update((value) => ({ ...value, settings: saved }));
}

export function currentSnapshot() {
  return get(snapshot);
}
