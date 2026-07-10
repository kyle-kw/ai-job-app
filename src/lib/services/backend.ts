import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { mockResume, mockSnapshot } from '$lib/mock-data';
import { buildClientJobDataReport } from '$lib/report';
import type {
  AiProviderConfig,
  AppSettings,
  BootstrapSnapshot,
  ImportResumePayload,
  Job,
  JobDataReport,
  JobPreferences,
  ProviderTestResult,
  RenderResult,
  ResumePatch,
  ResumeProfile,
  SearchSpec,
  TaskEvent,
  TaskKind,
  TaskRun
} from '$lib/types';

const browserMode = () => typeof window === 'undefined' || !window.__TAURI_INTERNALS__;
let mockState: BootstrapSnapshot = structuredClone(mockSnapshot);
const mockListeners = new Set<(event: TaskEvent) => void>();

function emitMock(task: TaskRun) {
  const index = mockState.tasks.findIndex((item) => item.id === task.id);
  if (index >= 0) mockState.tasks[index] = task;
  else mockState.tasks.unshift(task);
  mockListeners.forEach((listener) => listener(structuredClone(task)));
}

function createMockTask(kind: TaskKind, title: string): TaskRun {
  const createdAt = new Date().toISOString();
  return {
    id: crypto.randomUUID(), kind, title, state: 'queued', progress: 0, message: '等待开始', createdAt, updatedAt: createdAt, logs: []
  };
}

function advanceMockTask(task: TaskRun, steps: Array<{ progress: number; message: string }>, done?: () => void) {
  emitMock(task);
  steps.forEach((step, index) => {
    window.setTimeout(() => {
      task = {
        ...task,
        state: index === steps.length - 1 ? 'completed' : 'running',
        progress: step.progress,
        message: step.message,
        updatedAt: new Date().toISOString(),
        logs: [...task.logs, `[${new Date().toLocaleTimeString()}] ${step.message}`]
      };
      emitMock(task);
      if (index === steps.length - 1) done?.();
    }, 450 + index * 650);
  });
}

export const backend = {
  async bootstrap(): Promise<BootstrapSnapshot> {
    if (browserMode()) return structuredClone(mockState);
    return invoke('bootstrap');
  },

  async getJobDataReport(): Promise<JobDataReport> {
    if (browserMode()) return buildClientJobDataReport(mockState.jobs);
    return invoke('get_job_data_report');
  },

  async exportJobDataReport(): Promise<RenderResult> {
    if (browserMode()) return { path: 'browser-demo://岗位数据报告.html', fileName: '岗位数据报告_demo.html' };
    return invoke('export_job_data_report');
  },

  async onTaskEvent(callback: (event: TaskEvent) => void): Promise<UnlistenFn> {
    if (browserMode()) {
      mockListeners.add(callback);
      return () => mockListeners.delete(callback);
    }
    return listen<TaskEvent>('task://event', (event) => callback(event.payload));
  },

  async startScrape(spec: SearchSpec): Promise<string> {
    if (browserMode()) {
      let task = createMockTask('scrape', `抓取 ${spec.city} · ${spec.keyword}`);
      advanceMockTask(task, [
        { progress: 12, message: '正在检查 BOSS 登录状态' },
        { progress: 34, message: '正在抓取第 1 页岗位' },
        { progress: 68, message: '正在补充职位详情' },
        { progress: 88, message: '正在去重并写入本地岗位库' },
        { progress: 100, message: '抓取完成，市场报告已生成' }
      ], () => {
        mockState.readiness.boss = true;
      });
      return task.id;
    }
    return invoke('start_scrape', { spec });
  },

  async setupBoss(): Promise<string> {
    if (browserMode()) {
      const task = createMockTask('boss-login', '连接 BOSS 专用浏览器');
      advanceMockTask(task, [
        { progress: 20, message: '正在启动独立 Chrome Profile' },
        { progress: 65, message: '等待完成登录' },
        { progress: 100, message: '已确认登录状态' }
      ], () => { mockState.readiness.boss = true; });
      return task.id;
    }
    return invoke('setup_boss');
  },

  async importResume(payload: ImportResumePayload): Promise<string> {
    if (browserMode()) {
      const task = createMockTask('resume-import', `解析 ${payload.fileName}`);
      advanceMockTask(task, [
        { progress: 24, message: '正在提取简历文本' },
        { progress: 56, message: '正在识别经历和技能' },
        { progress: 82, message: '正在校验低置信度字段' },
        { progress: 100, message: '主简历已生成' }
      ], () => {
        mockState.resume = { ...mockResume, sourceFileName: payload.fileName, updatedAt: new Date().toISOString() };
        mockState.readiness.resume = true;
      });
      return task.id;
    }
    return invoke('import_resume', { payload });
  },

  async saveResume(resume: ResumeProfile): Promise<ResumeProfile> {
    if (browserMode()) {
      mockState.resume = { ...resume, updatedAt: new Date().toISOString(), version: resume.version + 1 };
      mockState.readiness.resume = true;
      return structuredClone(mockState.resume);
    }
    return invoke('save_resume', { resume });
  },

  async savePreferences(preferences: JobPreferences): Promise<ResumeProfile> {
    if (browserMode()) {
      if (!mockState.resume) throw new Error('请先导入简历');
      mockState.resume.preferences = structuredClone(preferences);
      return structuredClone(mockState.resume);
    }
    return invoke('save_preferences', { preferences });
  },

  async analyzeJob(jobId: string): Promise<Job> {
    if (browserMode()) {
      const job = mockState.jobs.find((item) => item.id === jobId);
      if (!job) throw new Error('岗位不存在');
      return structuredClone(job);
    }
    return invoke('analyze_job', { jobId });
  },

  async generateGreeting(jobId: string): Promise<string> {
    if (browserMode()) {
      const job = mockState.jobs.find((item) => item.id === jobId);
      if (!job) throw new Error('岗位不存在');
      job.greeting = `您好，我有 RAG 与 Agent 工程落地经验，和贵司${job.title}较匹配，方便聊聊吗？`;
      return job.greeting;
    }
    return invoke('generate_greeting', { jobId });
  },

  async proposeTailoring(jobId: string): Promise<ResumePatch[]> {
    if (browserMode()) {
      const job = mockState.jobs.find((item) => item.id === jobId);
      if (!job) throw new Error('岗位不存在');
      const patches: ResumePatch[] = [
        { id: crypto.randomUUID(), jobId, section: '个人简介', before: mockResume.summary, after: `3 年企业级 AI 应用研发经验，专注于 RAG、Agent 工作流与生产级服务，能够独立推动${job.title}相关能力从验证到上线。`, rationale: '把岗位最看重的 RAG、Agent 与工程化能力前置。', evidenceFactIds: ['fact-rag', 'fact-agent', 'fact-stack'], status: 'pending' },
        { id: crypto.randomUUID(), jobId, section: '云帆科技 · 经历 1', before: mockResume.experiences[0].highlights[0], after: '主导企业知识库 RAG 平台研发与评测，检索命中率提升 23%，覆盖 6 个业务团队。', rationale: '使用“主导、研发、评测”准确对应 JD 的交付职责。', evidenceFactIds: ['fact-rag'], status: 'pending' }
      ];
      job.patches = patches;
      return structuredClone(patches);
    }
    return invoke('propose_tailoring', { jobId });
  },

  async updatePatch(jobId: string, patchId: string, status: ResumePatch['status'], after?: string): Promise<ResumePatch[]> {
    if (browserMode()) {
      const job = mockState.jobs.find((item) => item.id === jobId);
      if (!job?.patches) return [];
      job.patches = job.patches.map((patch) => patch.id === patchId ? { ...patch, status, after: after ?? patch.after } : patch);
      return structuredClone(job.patches);
    }
    return invoke('update_resume_patch', { jobId, patchId, status, after });
  },

  async renderResume(jobId?: string): Promise<RenderResult> {
    if (browserMode()) return { path: 'browser-demo://resume.pdf', fileName: jobId ? '专岗简历_demo.pdf' : '主简历_demo.pdf' };
    return invoke('render_resume', { jobId: jobId ?? null });
  },

  async saveProvider(provider: AiProviderConfig): Promise<AiProviderConfig[]> {
    if (browserMode()) {
      mockState.providers = mockState.providers.map((item) => item.id === provider.id ? { ...provider, apiKey: undefined, apiKeyRef: provider.apiKey ? `keychain://${provider.id}` : item.apiKeyRef } : item);
      return structuredClone(mockState.providers);
    }
    return invoke('save_provider', { provider });
  },

  async testProvider(provider: AiProviderConfig): Promise<ProviderTestResult> {
    if (browserMode()) {
      await new Promise((resolve) => window.setTimeout(resolve, 700));
      const ok = Boolean(provider.apiKey && provider.baseUrl && provider.model);
      if (ok) {
        mockState.providers = mockState.providers.map((item) => item.id === provider.id ? { ...item, ...provider, apiKey: undefined, verified: true, lastTestedAt: new Date().toISOString() } : item);
        mockState.readiness.ai = true;
      }
      return { ok, message: ok ? '连接成功，结构化输出正常' : '请填写 API Key、Base URL 和模型名', latencyMs: 684, structuredOutput: ok };
    }
    return invoke('test_provider', { provider });
  },

  async saveSettings(settings: AppSettings): Promise<AppSettings> {
    if (browserMode()) {
      mockState.settings = structuredClone(settings);
      return structuredClone(settings);
    }
    return invoke('save_settings', { settings });
  }
};
