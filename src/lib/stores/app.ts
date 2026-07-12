import { derived, get, writable } from 'svelte/store';
import { backend } from '$lib/services/backend';
import type { AppSettings, BootstrapSnapshot, ImportResumePayload, JobPreferences, SearchSpec, TaskEvent } from '$lib/types';

const empty: BootstrapSnapshot = {
  readiness: { ai: false, resume: false, boss: false },
  configuration: {
    boss: { state: 'needs_setup', message: '需要配置 BOSS 专用浏览器。' },
    llm: { state: 'needs_setup', message: '需要配置默认模型。' }
  },
  jobs: [], resume: null, providers: [], tasks: [], scrapeRuns: [],
  settings: { locale: 'zh-CN', theme: 'light', advancedMode: false, telemetry: false, privacyAcknowledged: false }
};

export const snapshot = writable<BootstrapSnapshot>(empty);
export const loading = writable(true);
export const appError = writable<string | null>(null);
let unlisten: (() => void) | null = null;

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
  loading.set(true);
  appError.set(null);
  try {
    snapshot.set(await backend.bootstrap());
    unlisten?.();
    unlisten = await backend.onTaskEvent(mergeTask);
  } catch (error) {
    appError.set(error instanceof Error ? error.message : String(error));
  } finally {
    loading.set(false);
  }
}

export async function refresh() {
  try {
    snapshot.set(await backend.bootstrap());
  } catch (error) {
    appError.set(error instanceof Error ? error.message : String(error));
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
