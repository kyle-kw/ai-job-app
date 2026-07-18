import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Channel, invoke } from '@tauri-apps/api/core';
import packageMetadata from '../../../package.json';
import { mockJobs, mockResume, mockSnapshot } from '$lib/mock-data';
import { deterministicFit } from '$lib/fit';
import { filterJobs, sortJobs } from '$lib/job-filters';
import { buildClientJobDataReport, buildScrapeSampleSummary } from '$lib/report';
import { buildLocalReportCompetitiveness } from '$lib/report-competitiveness';
import { flattenProfessionalSkills, suggestedProfessionalSkillGroups } from '$lib/resume-templates';
import { applyResumeRebase, buildResumeRebasePreview } from '$lib/resume-rebase';
import { buildLocalResumeCoverage, summarizeCoverage } from '$lib/resume-coverage';
import type {
  AiProviderConfig,
  ApplyResumeEditsRequest,
  AppSettings,
  AppInfo,
  AppUpdateInfo,
  BackupInfo,
  BootstrapSnapshot,
  ClearDataResult,
  ClearDataScope,
  DeleteJobsResult,
  FitAnalysisResult,
  ImportResumePayload,
  InterviewPreparationState,
  Job,
  JobFilterOptions,
  JobOption,
  JobPage,
  JobQuery,
  JobDataReport,
  JobPreferences,
  ProviderTestResult,
  ProviderSaveResult,
  ReportKeyword,
  ReportCompetitivenessAnalysis,
  ReportCompetitivenessState,
  RenderResult,
  RenderResumeRequest,
  ResumeChatProposal,
  ResumeChatRequest,
  ResumeCommitResult,
  ResumeCoverageReport,
  ResumeEditCommitResult,
  ResumeRebasePreview,
  ResumeRebaseResolution,
  ResumeProfile,
  ResumeTemplateId,
  ResumeTargetRef,
  ResumeVariantCommitResult,
  ResumeVariantDetail,
  ResumeVariantSummary,
  ResumeVersionDetail,
  ResumeVersionSummary,
  SearchSpec,
  TaskEvent,
  TaskKind,
  TaskRun,
  UpdateEvent
} from '$lib/types';

const browserMode = () => typeof window === 'undefined' || !window.__TAURI_INTERNALS__;
let mockState: BootstrapSnapshot = structuredClone(mockSnapshot);
let mockJobsState: Job[] = structuredClone(mockJobs);
const mockReportKeywords: ReportKeyword[] = [
  { key: 'ai-agent', label: 'AI Agent', jobCount: 4, lastSeen: new Date().toISOString() },
  {
    key: 'data-analysis',
    label: '数据分析',
    jobCount: 2,
    lastSeen: new Date(Date.now() - 60_000).toISOString()
  }
];
const mockKeywordJobs: Record<string, string[]> = {
  'ai-agent': ['job-1', 'job-2', 'job-3', 'job-5'],
  'data-analysis': ['job-3', 'job-4']
};
const mockJobsForKeywords = (keywordKeys: string[]) => {
  const ids = new Set(keywordKeys.flatMap((key) => mockKeywordJobs[key] ?? []));
  return mockJobsState.filter((job) => ids.has(job.id));
};
const currentMockReportKeywords = () =>
  mockReportKeywords
    .map((keyword) => ({ ...keyword, jobCount: mockJobsForKeywords([keyword.key]).length }))
    .filter((keyword) => keyword.jobCount > 0);
const mockListeners = new Set<(event: TaskEvent) => void>();
const mockTimerIds = new Set<number>();
const initialMockPreparationState = (): InterviewPreparationState => ({
  status: 'missing',
  hasProvider: false,
  hasResume: true,
  preparation: null
});
const initialMockVersions = (): ResumeVersionDetail[] => [
  {
    id: 'resume-version-initial',
    resumeId: mockResume.id,
    version: mockResume.version,
    parentVersion: mockResume.version - 1,
    createdAt: mockResume.updatedAt,
    source: 'legacy',
    summary: '浏览器演示初始版本',
    profile: structuredClone(mockResume)
  }
];
let mockPreparationState = initialMockPreparationState();
let mockCompetitivenessCache: {
  scope: string;
  resumeVersion: number;
  signature: string;
  analysis: ReportCompetitivenessAnalysis;
} | null = null;
let mockVersions = initialMockVersions();
let mockVariants: ResumeVariantDetail[] = [];

export function resetBrowserMockState() {
  for (const timerId of mockTimerIds) window.clearTimeout(timerId);
  mockTimerIds.clear();
  mockListeners.clear();
  mockState = structuredClone(mockSnapshot);
  mockJobsState = structuredClone(mockJobs);
  mockKeywordJobs['ai-agent'] = ['job-1', 'job-2', 'job-3', 'job-5'];
  mockKeywordJobs['data-analysis'] = ['job-3', 'job-4'];
  mockPreparationState = initialMockPreparationState();
  mockCompetitivenessCache = null;
  mockVersions = initialMockVersions();
  mockVariants = [];
}

function emitMock(task: TaskRun) {
  const index = mockState.tasks.findIndex((item) => item.id === task.id);
  if (index >= 0) mockState.tasks[index] = task;
  else mockState.tasks.unshift(task);
  mockListeners.forEach((listener) => listener(structuredClone(task)));
}

function createMockTask(kind: TaskKind, title: string): TaskRun {
  const createdAt = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    kind,
    title,
    state: 'queued',
    progress: 0,
    message: '等待开始',
    createdAt,
    updatedAt: createdAt,
    logs: []
  };
}

function advanceMockTask(
  task: TaskRun,
  steps: Array<{ progress: number; message: string }>,
  done?: () => void
) {
  emitMock(task);
  steps.forEach((step, index) => {
    const timerId = window.setTimeout(
      () => {
        mockTimerIds.delete(timerId);
        task = {
          ...task,
          state: index === steps.length - 1 ? 'completed' : 'running',
          progress: step.progress,
          message: step.message,
          updatedAt: new Date().toISOString(),
          logs: [...task.logs, `[${new Date().toLocaleTimeString()}] ${step.message}`]
        };
        if (index === steps.length - 1) done?.();
        emitMock(task);
      },
      450 + index * 650
    );
    mockTimerIds.add(timerId);
  });
}

export const backend = {
  async bootstrap(): Promise<BootstrapSnapshot> {
    if (browserMode()) return structuredClone(mockState);
    return invoke('bootstrap');
  },

  async listJobsPage(query: JobQuery): Promise<JobPage> {
    if (browserMode()) {
      const scoped = query.keywordKeys?.length
        ? mockJobsForKeywords(query.keywordKeys)
        : mockJobsState;
      const filtered = sortJobs(filterJobs(scoped, query), query.sort);
      const offset = Number(query.cursor || 0);
      const items = filtered.slice(offset, offset + 50);
      return {
        items: structuredClone(items),
        total: filtered.length,
        pendingDetailCount: mockJobsState.filter(
          (job) => job.description.trim() && !job.structuredDetails
        ).length,
        nextCursor: offset + items.length < filtered.length ? String(offset + items.length) : null
      };
    }
    return invoke('list_jobs_page', { query });
  },

  async listJobOptions(query = ''): Promise<JobOption[]> {
    if (browserMode()) {
      const text = query.trim().toLowerCase();
      return mockJobsState
        .filter((job) => !text || `${job.title} ${job.company}`.toLowerCase().includes(text))
        .slice(0, 50)
        .map(({ id, title, company, lastSeen }) => ({ id, title, company, lastSeen }));
    }
    return invoke('list_job_options', { query });
  },

  async listJobFilterOptions(): Promise<JobFilterOptions> {
    if (browserMode()) {
      const skillCounts = new Map<string, { label: string; count: number }>();
      for (const job of mockJobsState) {
        for (const label of new Set(job.skills.map((skill) => skill.trim()).filter(Boolean))) {
          const key = label.toLocaleLowerCase();
          const current = skillCounts.get(key);
          skillCounts.set(key, {
            label: current?.label ?? label,
            count: (current?.count ?? 0) + 1
          });
        }
      }
      return {
        cities: [
          ...new Set(
            mockJobsState
              .map((job) => job.location.split('·', 1)[0]?.trim())
              .filter(Boolean) as string[]
          )
        ].sort((left, right) => left.localeCompare(right, 'zh-CN')),
        experiences: [
          ...new Set(mockJobsState.map((job) => job.experience.trim()).filter(Boolean))
        ].sort((left, right) => left.localeCompare(right, 'zh-CN', { numeric: true })),
        skills: [...skillCounts.values()].sort(
          (left, right) =>
            right.count - left.count || left.label.localeCompare(right.label, 'zh-CN')
        )
      };
    }
    return invoke('list_job_filter_options');
  },

  async getJob(jobId: string): Promise<Job> {
    if (browserMode()) {
      const job = mockJobsState.find((item) => item.id === jobId);
      if (!job) throw new Error('岗位不存在。');
      return structuredClone(job);
    }
    return invoke('get_job', { jobId });
  },

  async deleteJob(jobId: string): Promise<DeleteJobsResult> {
    if (browserMode()) {
      const before = mockJobsState.length;
      mockJobsState = mockJobsState.filter((job) => job.id !== jobId);
      const deletedCount = before - mockJobsState.length;
      if (!deletedCount) throw new Error('岗位不存在或已被删除。');
      return { deletedCount };
    }
    return invoke('delete_job', { jobId });
  },

  async deleteMissingDescriptionJobs(query: JobQuery): Promise<DeleteJobsResult> {
    if (browserMode()) {
      const { cursor: _cursor, ...filters } = query;
      const ids = new Set(
        filterJobs(mockJobsState, { ...filters, missingDescription: true }).map((job) => job.id)
      );
      mockJobsState = mockJobsState.filter((job) => !ids.has(job.id));
      return { deletedCount: ids.size };
    }
    return invoke('delete_missing_description_jobs', { query });
  },

  async listReportKeywords(): Promise<ReportKeyword[]> {
    if (browserMode()) return structuredClone(currentMockReportKeywords());
    return invoke('list_report_keywords');
  },

  async getJobDataReport(keywordKeys: string[]): Promise<JobDataReport> {
    if (browserMode()) {
      const selected = currentMockReportKeywords().filter((keyword) =>
        keywordKeys.includes(keyword.key)
      );
      return buildClientJobDataReport(
        mockJobsForKeywords(keywordKeys),
        selected,
        mockState.scrapeRuns
      );
    }
    return invoke('get_job_data_report', { keywordKeys });
  },

  async exportJobsJson(outputPath: string, query?: JobQuery): Promise<RenderResult> {
    if (browserMode())
      return {
        path: outputPath || 'browser-demo://岗位数据.json',
        fileName: outputPath.split(/[\\/]/).at(-1) || '岗位数据_demo.json'
      };
    return invoke('export_jobs_json', { outputPath, query: query ?? null });
  },

  async exportJobDataReport(keywordKeys: string[], outputPath: string): Promise<RenderResult> {
    if (browserMode())
      return {
        path: outputPath || 'browser-demo://岗位数据报告.html',
        fileName: outputPath.split(/[\\/]/).at(-1) || '岗位数据报告_demo.html'
      };
    return invoke('export_job_data_report', { keywordKeys, outputPath });
  },

  async getReportCompetitivenessState(keywordKeys: string[]): Promise<ReportCompetitivenessState> {
    if (browserMode()) {
      const hasResume = Boolean(mockState.resume);
      const hasProvider = mockState.readiness.ai;
      const jobs = mockJobsForKeywords(keywordKeys);
      if (!hasResume)
        return {
          status: 'missing',
          reason: 'no_resume',
          hasResume,
          hasProvider,
          local: null,
          ai: null,
          effectiveSource: null
        };
      if (!jobs.length)
        return {
          status: 'missing',
          reason: 'no_jobs',
          hasResume,
          hasProvider,
          local: null,
          ai: null,
          effectiveSource: null
        };
      const report = buildClientJobDataReport(
        jobs,
        currentMockReportKeywords().filter((keyword) => keywordKeys.includes(keyword.key)),
        mockState.scrapeRuns
      );
      const local = buildLocalReportCompetitiveness(report.topSkills, mockState.resume!);
      const scope = [...keywordKeys].sort().join('|');
      const signature = report.topSkills
        .slice(0, 12)
        .map((item) => `${item.label}:${item.count}`)
        .join('|');
      const fresh =
        mockCompetitivenessCache?.scope === scope &&
        mockCompetitivenessCache.resumeVersion === mockState.resume!.version &&
        mockCompetitivenessCache.signature === signature;
      return {
        status: fresh ? 'fresh' : mockCompetitivenessCache?.scope === scope ? 'stale' : 'missing',
        reason: hasProvider
          ? fresh
            ? null
            : mockCompetitivenessCache?.scope === scope
              ? 'data_changed'
              : null
          : 'no_provider',
        hasResume,
        hasProvider,
        generatedAt: fresh ? mockCompetitivenessCache?.analysis.generatedAt : null,
        local,
        ai: fresh ? mockCompetitivenessCache?.analysis : null,
        effectiveSource: fresh ? 'ai' : 'local'
      };
    }
    return invoke('get_report_competitiveness_state', { keywordKeys });
  },

  async generateReportCompetitiveness(
    keywordKeys: string[],
    force = false
  ): Promise<ReportCompetitivenessState> {
    if (browserMode()) {
      if (!mockState.resume) throw new Error('请先导入主简历');
      if (!mockState.readiness.ai) throw new Error('请先配置并验证默认模型');
      const report = buildClientJobDataReport(
        mockJobsForKeywords(keywordKeys),
        currentMockReportKeywords().filter((keyword) => keywordKeys.includes(keyword.key)),
        mockState.scrapeRuns
      );
      const local = buildLocalReportCompetitiveness(report.topSkills, mockState.resume);
      const analysis: ReportCompetitivenessAnalysis = {
        ...local,
        source: 'ai',
        generatedAt: new Date().toISOString(),
        items: local.items.map((item) => ({ ...item, rationale: `AI 语义复核：${item.rationale}` }))
      };
      mockCompetitivenessCache = {
        scope: [...keywordKeys].sort().join('|'),
        resumeVersion: mockState.resume.version,
        signature: report.topSkills
          .slice(0, 12)
          .map((item) => `${item.label}:${item.count}`)
          .join('|'),
        analysis
      };
      void force;
      return backend.getReportCompetitivenessState(keywordKeys);
    }
    return invoke('generate_report_competitiveness', { keywordKeys, force });
  },

  async getInterviewPreparationState(keywordKeys: string[]): Promise<InterviewPreparationState> {
    if (browserMode()) {
      return structuredClone({
        ...mockPreparationState,
        hasProvider: mockState.readiness.ai,
        hasResume: Boolean(mockState.resume),
        reason: mockState.readiness.ai ? mockPreparationState.reason : 'no_provider'
      });
    }
    return invoke('get_interview_preparation_state', { keywordKeys });
  },

  async generateInterviewPreparation(
    keywordKeys: string[],
    force = false
  ): Promise<InterviewPreparationState> {
    if (browserMode()) {
      const scopedJobs = mockJobsForKeywords(keywordKeys);
      if (!scopedJobs.length) throw new Error('所选关键词暂无岗位数据');
      if (!mockState.readiness.ai) throw new Error('请先配置并验证默认模型');
      if (!force && mockPreparationState.status === 'fresh')
        return structuredClone(mockPreparationState);
      const report = buildClientJobDataReport(
        scopedJobs,
        currentMockReportKeywords().filter((keyword) => keywordKeys.includes(keyword.key)),
        mockState.scrapeRuns
      );
      mockPreparationState = {
        status: 'fresh',
        hasProvider: true,
        hasResume: Boolean(mockState.resume),
        reason: mockState.resume ? null : 'no_resume',
        generatedAt: new Date().toISOString(),
        preparation: {
          summary: '优先准备高频技能的原理、工程实践与可量化项目案例。',
          skills: report.topSkills.slice(0, 6).map((skill) => ({
            name: skill.label,
            gap:
              mockState.resume &&
              flattenProfessionalSkills(mockState.resume).some(
                (item) => item.toLowerCase() === skill.label.toLowerCase()
              )
                ? 'ready'
                : 'unknown',
            action: `准备一个能说明 ${skill.label} 实际应用的项目案例。`,
            jobCount: skill.count
          })),
          projectIdeas: ['准备一段从需求、方案、取舍到结果的完整项目复盘。'],
          practiceQuestions: ['如何评估并改进一个生产级 AI 应用？']
        }
      };
      return structuredClone(mockPreparationState);
    }
    return invoke('generate_interview_preparation', { keywordKeys, force });
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
      mockState.lastSearchSpec = structuredClone(spec);
      const task = createMockTask('scrape', `抓取 ${spec.city} · ${spec.keyword}`);
      advanceMockTask(
        task,
        [
          { progress: 12, message: '正在检查 BOSS 登录状态' },
          { progress: 34, message: '正在抓取第 1 页岗位' },
          { progress: 68, message: '正在补充职位详情' },
          { progress: 88, message: '正在去重并写入本地岗位库' },
          { progress: 100, message: '抓取完成，岗位已写入本地库' }
        ],
        () => {
          mockState.readiness.boss = true;
          const completedAt = new Date().toISOString();
          const knownKeyword = mockReportKeywords.find(
            (keyword) => keyword.label.trim().toLowerCase() === spec.keyword.trim().toLowerCase()
          );
          if (knownKeyword) mockKeywordJobs[knownKeyword.key] = mockJobsState.map((job) => job.id);
          mockState.scrapeRuns.unshift({
            id: crypto.randomUUID(),
            keyword: spec.keyword.trim(),
            city: spec.city,
            totalSeen: mockJobsState.length,
            inserted: 0,
            updated: mockJobsState.length,
            startedAt: task.createdAt,
            completedAt,
            reportMarkdown: `## 本次岗位样本观察\n\n- 本次整理 **${mockJobsState.length}** 个本地去重岗位样本。`,
            searchSpec: structuredClone(spec),
            resolvedCity: spec.city,
            detailSummary: {
              total: mockJobsState.length,
              processed: mockJobsState.length,
              succeeded: 0,
              skipped: mockJobsState.length,
              failed: 0
            },
            sample: buildScrapeSampleSummary(mockJobsState)
          });
        }
      );
      return task.id;
    }
    return invoke('start_scrape', { spec });
  },

  async startJobDetailExtraction(): Promise<string> {
    if (browserMode()) {
      const task = createMockTask(
        'job-detail-extraction',
        `批量提取 ${mockJobsState.length} 条岗位详情`
      );
      advanceMockTask(
        task,
        [
          { progress: 12, message: '正在清理岗位详情页面噪声' },
          { progress: 56, message: '正在提取岗位职责与任职要求' },
          { progress: 82, message: '正在提取公司介绍与工商信息' },
          { progress: 100, message: `提取完成：成功 ${mockJobsState.length}，失败 0` }
        ],
        () => {
          mockJobsState = mockJobsState.map((job) => ({
            ...job,
            structuredDetails: {
              jobDescription: job.description,
              responsibilities: [job.description],
              requirements: [],
              companyIntroduction: '',
              businessInformation: {
                companyName: '',
                legalRepresentative: '',
                establishedDate: '',
                companyType: '',
                operatingStatus: '',
                registeredCapital: ''
              },
              extractedAt: new Date().toISOString(),
              extractorVersion: 'job-detail-extraction@1.0.0'
            }
          }));
        }
      );
      return task.id;
    }
    return invoke('start_job_detail_extraction');
  },

  async setupBoss(options: { resetProfile: boolean }): Promise<string> {
    if (browserMode()) {
      const task = createMockTask(
        'boss-login',
        options.resetProfile ? '重新配置 BOSS 专用浏览器' : '配置 BOSS 专用浏览器'
      );
      mockState.configuration.boss = {
        state: 'running',
        message: '等待完成 BOSS 登录',
        lastAttemptAt: new Date().toISOString()
      };
      advanceMockTask(
        task,
        [
          { progress: 20, message: '正在启动独立 Chrome Profile' },
          { progress: 65, message: '等待完成登录' },
          { progress: 100, message: 'BOSS 登录配置已完成，专用 Chrome 已自动关闭' }
        ],
        () => {
          mockState.readiness.boss = true;
          mockState.configuration.boss = {
            state: 'ready',
            message: 'BOSS 专用 Chrome Profile 已配置',
            lastAttemptAt: new Date().toISOString()
          };
        }
      );
      return task.id;
    }
    return invoke('setup_boss', { resetProfile: options.resetProfile });
  },

  async importResume(payload: ImportResumePayload): Promise<string> {
    if (browserMode()) {
      const task = createMockTask('resume-import', `解析 ${payload.fileName}`);
      advanceMockTask(
        task,
        [
          { progress: 24, message: '正在提取简历文本' },
          { progress: 56, message: '正在识别经历和技能' },
          { progress: 82, message: '正在校验低置信度字段' },
          { progress: 100, message: '主简历已生成' }
        ],
        () => {
          mockState.resume = {
            ...mockResume,
            sourceFileName: payload.fileName,
            updatedAt: new Date().toISOString()
          };
          mockState.readiness.resume = true;
        }
      );
      return task.id;
    }
    return invoke('import_resume', { payload });
  },

  async saveResume(resume: ResumeProfile): Promise<ResumeProfile> {
    if (browserMode()) {
      mockState.resume = {
        ...resume,
        updatedAt: new Date().toISOString(),
        version: resume.version + 1
      };
      mockState.readiness.resume = true;
      mockVersions.unshift({
        id: crypto.randomUUID(),
        resumeId: mockState.resume.id,
        version: mockState.resume.version,
        parentVersion: resume.version,
        createdAt: mockState.resume.updatedAt,
        source: 'manual',
        summary: '手工保存主简历',
        profile: structuredClone(mockState.resume)
      });
      return structuredClone(mockState.resume);
    }
    return invoke('save_resume', { resume });
  },

  async listResumeVariants(): Promise<ResumeVariantSummary[]> {
    if (browserMode())
      return mockVariants.map(({ profile: _, ...item }) => ({
        ...structuredClone(item),
        stale: Boolean(mockState.resume && mockState.resume.version > item.baseResumeVersion)
      }));
    return invoke('list_resume_variants');
  },

  async getResumeVariant(variantId: string): Promise<ResumeVariantDetail> {
    if (browserMode()) {
      const variant = mockVariants.find((item) => item.id === variantId);
      if (!variant) throw new Error('variant_not_found: 岗位版本不存在');
      return {
        ...structuredClone(variant),
        stale: Boolean(mockState.resume && mockState.resume.version > variant.baseResumeVersion)
      };
    }
    return invoke('get_resume_variant', { variantId });
  },

  async createResumeVariant(
    jobId: string,
    expectedResumeVersion: number
  ): Promise<ResumeVariantDetail> {
    if (browserMode()) {
      const existing = mockVariants.find((item) => item.jobId === jobId);
      if (existing) return structuredClone(existing);
      const master = mockState.resume;
      const job = mockJobsState.find((item) => item.id === jobId);
      if (!master) throw new Error('请先导入主简历');
      if (!job) throw new Error('岗位不存在');
      if (master.version !== expectedResumeVersion)
        throw new Error('version_conflict: 主简历已变化');
      const now = new Date().toISOString();
      const id = crypto.randomUUID();
      const profile = { ...structuredClone(master), id, version: 1, updatedAt: now };
      const detail: ResumeVariantDetail = {
        id,
        jobId,
        jobTitle: job.title,
        company: job.company,
        name: `${job.company} · ${job.title}`,
        baseResumeId: master.id,
        baseResumeVersion: master.version,
        version: 1,
        createdAt: now,
        updatedAt: now,
        stale: false,
        profile
      };
      mockVariants.unshift(detail);
      mockVersions.unshift({
        id: crypto.randomUUID(),
        resumeId: id,
        version: 1,
        parentVersion: null,
        createdAt: now,
        source: 'variant-create',
        summary: '创建岗位版本',
        jobId,
        profile: structuredClone(profile)
      });
      return structuredClone(detail);
    }
    return invoke('create_resume_variant', { jobId, expectedResumeVersion });
  },

  async saveResumeVariant(
    variantId: string,
    resume: ResumeProfile,
    expectedVersion: number
  ): Promise<ResumeVariantCommitResult> {
    if (browserMode()) {
      const index = mockVariants.findIndex((item) => item.id === variantId);
      if (index < 0) throw new Error('variant_not_found: 岗位版本不存在');
      const current = mockVariants[index];
      if (current.version !== expectedVersion) throw new Error('version_conflict: 岗位版本已变化');
      const now = new Date().toISOString();
      const profile = {
        ...structuredClone(resume),
        id: variantId,
        version: current.version + 1,
        updatedAt: now,
        facts: structuredClone(current.profile.facts),
        preferences: structuredClone(current.profile.preferences)
      };
      const variant = { ...current, version: profile.version, updatedAt: now, profile };
      mockVariants[index] = variant;
      const version: ResumeVersionSummary = {
        id: crypto.randomUUID(),
        resumeId: variantId,
        version: profile.version,
        parentVersion: expectedVersion,
        createdAt: now,
        source: 'variant-manual',
        summary: '手工保存岗位版本',
        jobId: current.jobId
      };
      mockVersions.unshift({ ...version, profile: structuredClone(profile) });
      return structuredClone({ variant, version });
    }
    return invoke('save_resume_variant', { variantId, resume, expectedVersion });
  },

  async deleteResumeVariant(variantId: string): Promise<number> {
    if (browserMode()) {
      const previous = mockVariants.length;
      mockVariants = mockVariants.filter((item) => item.id !== variantId);
      mockVersions = mockVersions.filter((item) => item.resumeId !== variantId);
      return previous - mockVariants.length;
    }
    return invoke('delete_resume_variant', { variantId });
  },

  async previewResumeVariantRebase(variantId: string): Promise<ResumeRebasePreview> {
    if (browserMode()) {
      const variant = mockVariants.find((item) => item.id === variantId);
      const master = mockState.resume;
      if (!variant || !master) throw new Error('无法读取岗位版本或主简历');
      const base = mockVersions.find(
        (item) =>
          item.resumeId === variant.baseResumeId && item.version === variant.baseResumeVersion
      )?.profile;
      if (!base) throw new Error('base_version_missing: 主简历基线不存在');
      return buildResumeRebasePreview(
        variant.id,
        variant.version,
        variant.baseResumeVersion,
        base,
        master,
        variant.profile
      );
    }
    return invoke('preview_resume_variant_rebase', { variantId });
  },

  async applyResumeVariantRebase(
    variantId: string,
    expectedVariantVersion: number,
    expectedMasterVersion: number,
    resolutions: ResumeRebaseResolution[]
  ): Promise<ResumeVariantCommitResult> {
    if (browserMode()) {
      const variant = mockVariants.find((item) => item.id === variantId);
      const master = mockState.resume;
      if (!variant || !master) throw new Error('无法读取岗位版本或主简历');
      if (variant.version !== expectedVariantVersion || master.version !== expectedMasterVersion)
        throw new Error('version_conflict: 内容已变化');
      const preview = await backend.previewResumeVariantRebase(variantId);
      const candidate = applyResumeRebase(variant.profile, master, preview, resolutions);
      const committed = await backend.saveResumeVariant(
        variantId,
        candidate,
        expectedVariantVersion
      );
      const stored = mockVariants.find((item) => item.id === variantId)!;
      stored.profile.facts = structuredClone(master.facts);
      stored.profile.preferences = structuredClone(master.preferences);
      stored.baseResumeVersion = master.version;
      stored.stale = false;
      committed.variant.baseResumeVersion = master.version;
      committed.variant.profile.facts = structuredClone(master.facts);
      committed.variant.profile.preferences = structuredClone(master.preferences);
      committed.variant.stale = false;
      committed.version.source = 'variant-rebase';
      committed.version.summary = `同步主简历 v${master.version}`;
      const storedVersion = mockVersions.find((item) => item.id === committed.version.id);
      if (storedVersion) {
        storedVersion.source = 'variant-rebase';
        storedVersion.summary = committed.version.summary;
        storedVersion.profile = structuredClone(stored.profile);
      }
      return structuredClone(committed);
    }
    return invoke('apply_resume_variant_rebase', {
      variantId,
      expectedVariantVersion,
      expectedMasterVersion,
      resolutions
    });
  },

  async restoreResumeVariantVersion(
    variantId: string,
    versionId: string,
    expectedVersion: number
  ): Promise<ResumeVariantCommitResult> {
    if (browserMode()) {
      const version = mockVersions.find(
        (item) => item.id === versionId && item.resumeId === variantId
      );
      if (!version) throw new Error('简历版本不存在');
      const result = await backend.saveResumeVariant(variantId, version.profile, expectedVersion);
      result.version.source = 'variant-rollback';
      result.version.summary = `恢复到 v${version.version} 的内容`;
      return result;
    }
    return invoke('restore_resume_variant_version', { variantId, versionId, expectedVersion });
  },

  async createResumeFromTemplate(templateId: ResumeTemplateId): Promise<ResumeProfile> {
    if (browserMode()) {
      const now = new Date().toISOString();
      mockState.resume = {
        id: 'resume-master',
        name: '',
        headline: '',
        email: '',
        phone: '',
        location: '',
        website: '',
        summary: '',
        templateId,
        professionalSkills: suggestedProfessionalSkillGroups(templateId),
        experiences: [],
        education: [],
        projects: [],
        certifications: [],
        facts: [],
        preferences: {
          targetRoles: [],
          cities: [],
          remotePreference: 'flexible',
          energizingTasks: [],
          drainingTasks: [],
          hardConstraints: []
        },
        sourceFileName: `内置${templateId}模板`,
        updatedAt: now,
        version: 1
      };
      mockState.readiness.resume = true;
      return structuredClone(mockState.resume);
    }
    return invoke('create_resume_from_template', { templateId });
  },

  async savePreferences(preferences: JobPreferences): Promise<ResumeProfile> {
    if (browserMode()) {
      if (!mockState.resume) throw new Error('请先导入简历');
      mockState.resume.preferences = structuredClone(preferences);
      return structuredClone(mockState.resume);
    }
    return invoke('save_preferences', { preferences });
  },

  async analyzeJob(jobId: string, force = false): Promise<FitAnalysisResult> {
    if (browserMode()) {
      const job = mockJobsState.find((item) => item.id === jobId);
      if (!job) throw new Error('岗位不存在');
      if (!mockState.resume) throw new Error('请先导入主简历');
      const cacheHit = !force && job.fit?.cacheStatus === 'fresh';
      if (!cacheHit) {
        job.fit = {
          ...deterministicFit(job, mockState.resume),
          inputHash: `mock-${job.id}-${mockState.resume.version}`,
          analysisSource: mockState.readiness.ai ? 'llm' : 'local',
          fallbackReason: mockState.readiness.ai ? null : 'provider_missing',
          cacheStatus: 'fresh'
        };
      }
      return {
        job: structuredClone(job),
        cacheHit,
        source: job.fit?.analysisSource === 'llm' ? 'llm' : 'local',
        warning: mockState.readiness.ai ? null : '尚未配置模型，已使用本地基础匹配。'
      };
    }
    return invoke('analyze_job', { jobId, force });
  },

  async startFitBatch(jobIds: string[]): Promise<string> {
    if (browserMode()) {
      if (!mockState.resume) throw new Error('请先导入主简历');
      const task = createMockTask('fit', `批量分析 ${jobIds.length} 个岗位`);
      advanceMockTask(
        task,
        [
          { progress: 10, message: '正在准备匹配上下文' },
          { progress: 60, message: '正在分析当前筛选结果' },
          { progress: 100, message: `完成：AI 0，本地基础 ${jobIds.length}，缓存跳过 0，失败 0` }
        ],
        () => {
          mockJobsState = mockJobsState.map((job) =>
            jobIds.includes(job.id)
              ? {
                  ...job,
                  fit: {
                    ...deterministicFit(job, mockState.resume!),
                    inputHash: `mock-${job.id}-${mockState.resume!.version}`,
                    analysisSource: mockState.readiness.ai ? 'llm' : 'local',
                    cacheStatus: 'fresh'
                  }
                }
              : job
          );
        }
      );
      return task.id;
    }
    return invoke('start_fit_batch', { jobIds });
  },

  async startFitBatchForQuery(query: JobQuery): Promise<string> {
    if (browserMode()) {
      return backend.startFitBatch(filterJobs(mockJobsState, query).map((job) => job.id));
    }
    return invoke('start_fit_batch_for_query', { query });
  },

  async openJobSource(jobId: string): Promise<void> {
    if (browserMode()) {
      const job = mockJobsState.find((item) => item.id === jobId);
      if (!job?.sourceUrl) throw new Error('原岗位链接不可用');
      window.open(job.sourceUrl, '_blank', 'noopener,noreferrer');
      return;
    }
    return invoke('open_job_source', { jobId });
  },

  async generateGreeting(jobId: string): Promise<string> {
    if (browserMode()) {
      const job = mockJobsState.find((item) => item.id === jobId);
      if (!job) throw new Error('岗位不存在');
      job.greeting = `您好，我有 RAG 与 Agent 工程落地经验，和贵司${job.title}较匹配，方便聊聊吗？`;
      return job.greeting;
    }
    return invoke('generate_greeting', { jobId });
  },

  async renderResume(request: RenderResumeRequest): Promise<RenderResult> {
    if (browserMode())
      return {
        path: request.outputPath || 'browser-demo://resume.pdf',
        fileName: request.outputPath.split(/[\\/]/).at(-1) || '主简历_demo.pdf'
      };
    return invoke('render_resume', {
      outputPath: request.outputPath,
      colorTheme: request.colorTheme,
      target: request.target
    });
  },

  async analyzeResumeCoverage(
    target: ResumeTargetRef,
    force = false
  ): Promise<ResumeCoverageReport> {
    if (browserMode()) {
      const variant =
        target.kind === 'variant' ? mockVariants.find((item) => item.id === target.id) : null;
      const resume = variant?.profile ?? mockState.resume;
      const job = variant ? mockJobsState.find((item) => item.id === variant.jobId) : null;
      if (!resume || !job) throw new Error('岗位版本或关联岗位不存在');
      const local = buildLocalResumeCoverage(job, target, resume);
      return summarizeCoverage({
        ...local,
        source: 'ai',
        generatedAt: new Date().toISOString(),
        items: local.items.map((item) => ({
          ...item,
          rationale: item.status === 'unknown' ? 'AI 未找到足够证据，保持未知。' : item.rationale
        }))
      });
    }
    return invoke('analyze_resume_coverage', { target, force });
  },

  async proposeResumeChatEdits(request: ResumeChatRequest): Promise<ResumeChatProposal> {
    if (browserMode()) {
      if (request.jobId && request.marketContext)
        throw new Error('invalid_request: 关联岗位与市场样本上下文不能同时使用');
      if (request.marketContext && request.target.kind !== 'master')
        throw new Error('invalid_request: 市场样本上下文仅可用于主简历');
      const variant =
        request.target.kind === 'variant'
          ? mockVariants.find((item) => item.id === request.target.id)
          : null;
      const resume = variant?.profile ?? mockState.resume;
      if (!resume) throw new Error('请先导入主简历');
      if (!mockState.readiness.ai) throw new Error('请先配置并验证默认模型');
      if (request.expectedVersion !== resume.version)
        throw new Error('version_conflict: 简历已变化');
      const last = request.messages.at(-1)?.content ?? '';
      const shouldShorten = /精简|缩短|简洁/.test(last);
      const after = shouldShorten
        ? resume.summary.slice(0, Math.max(40, Math.floor(resume.summary.length * 0.75)))
        : resume.summary;
      const jobId = variant?.jobId ?? request.jobId;
      let marketContext = null;
      if (request.marketContext) {
        const keywordKeys = [
          ...new Set(
            request.marketContext.keywordKeys.map((key) => key.trim().toLowerCase()).filter(Boolean)
          )
        ].sort();
        if (!keywordKeys.length || keywordKeys.length > 8)
          throw new Error('invalid_market_context: 请选择 1 至 8 个有效报告关键词');
        const selectedKeywords = currentMockReportKeywords().filter((keyword) =>
          keywordKeys.includes(keyword.key)
        );
        if (selectedKeywords.length !== keywordKeys.length)
          throw new Error('invalid_market_context: 包含未知或已失效的报告关键词');
        if (request.marketContext.focusSkills.length > 12)
          throw new Error('invalid_market_context: 最多关注 12 个当前报告技能');
        const jobs = mockJobsForKeywords(keywordKeys);
        const report = buildClientJobDataReport(jobs, selectedKeywords, mockState.scrapeRuns);
        const analysis = buildLocalReportCompetitiveness(report.topSkills, resume);
        const focusSkills = request.marketContext.focusSkills
          .map((skill) => skill.trim())
          .filter(Boolean);
        const items = focusSkills.length
          ? focusSkills.map((skill) => {
              const item = analysis.items.find(
                (candidate) => candidate.label.toLowerCase() === skill.toLowerCase()
              );
              if (!item)
                throw new Error(`invalid_market_context: 技能“${skill}”不在当前报告范围内`);
              return item;
            })
          : analysis.items.slice(0, 12);
        marketContext = {
          keywordKeys,
          keywordLabels: selectedKeywords.map((keyword) => keyword.label),
          totalJobs: jobs.length,
          skills: [
            ...new Map(
              items.map((item) => [
                item.id,
                {
                  label: item.label,
                  jobCount: item.jobCount,
                  percentage: item.percentage,
                  status: item.status,
                  rationale: item.rationale
                }
              ])
            ).values()
          ]
        };
      }
      const allowMockEdit = shouldShorten && !marketContext;
      return {
        proposalId: crypto.randomUUID(),
        target: request.target,
        baseVersion: resume.version,
        job: jobId
          ? (() => {
              const job = mockJobsState.find((item) => item.id === jobId);
              return job ? { id: job.id, title: job.title, company: job.company } : null;
            })()
          : null,
        marketContext,
        assistantMessage: marketContext
          ? '我已读取后端确认的本地市场样本。缺少候选人事实依据时，我会先核对经历，不会把市场要求直接写进简历。'
          : shouldShorten
            ? '我整理了一版更精简的个人简介，请审核后应用。'
            : '请告诉我希望修改的具体字段或目标；我不会在没有事实依据时改写。',
        edits: allowMockEdit
          ? [
              {
                id: crypto.randomUUID(),
                path: '/summary',
                label: '个人简介',
                operation: 'replace',
                before: resume.summary,
                after,
                rationale: '压缩重复表述，保留现有事实。',
                evidenceFactIds: [],
                requiredFactCandidateIds: []
              }
            ]
          : [],
        factCandidates: [],
        warnings: []
      };
    }
    return invoke('propose_resume_chat_edits', { request });
  },

  async applyResumeChatEdits(request: ApplyResumeEditsRequest): Promise<ResumeEditCommitResult> {
    if (browserMode()) {
      if (request.proposal.target.kind === 'variant') {
        const variant = mockVariants.find((item) => item.id === request.proposal.target.id);
        if (!variant || variant.version !== request.expectedVersion)
          throw new Error('version_conflict: 岗位版本已变化');
        if (request.proposal.factCandidates.length || request.confirmedFactCandidateIds.length)
          throw new Error('fact_requires_master: 请先在主简历确认新事实');
        const next = structuredClone(variant.profile) as ResumeProfile & Record<string, unknown>;
        for (const edit of request.proposal.edits.filter((item) =>
          request.selectedEditIds.includes(item.id)
        )) {
          const key = edit.path.slice(1);
          if (key in next) next[key] = structuredClone(edit.after);
        }
        const result = await backend.saveResumeVariant(variant.id, next, request.expectedVersion);
        result.version.source = 'variant-ai';
        result.version.summary = `岗位版本 AI 应用 ${request.selectedEditIds.length} 项修改`;
        return result;
      }
      if (!mockState.resume || mockState.resume.version !== request.expectedVersion)
        throw new Error('version_conflict: 简历已变化');
      const next = structuredClone(mockState.resume) as ResumeProfile & Record<string, unknown>;
      for (const edit of request.proposal.edits.filter((item) =>
        request.selectedEditIds.includes(item.id)
      )) {
        const key = edit.path.slice(1);
        if (key in next) next[key] = structuredClone(edit.after);
      }
      next.version += 1;
      next.updatedAt = new Date().toISOString();
      mockState.resume = next;
      const version: ResumeVersionSummary = {
        id: crypto.randomUUID(),
        resumeId: next.id,
        version: next.version,
        parentVersion: request.expectedVersion,
        createdAt: next.updatedAt,
        source: request.proposal.marketContext ? 'market-ai-chat' : 'ai-chat',
        summary: request.proposal.marketContext
          ? `市场样本 AI 修改 · 应用 ${request.selectedEditIds.length} 项修改`
          : `AI 对话应用 ${request.selectedEditIds.length} 项修改`,
        jobId: request.proposal.job?.id ?? null,
        proposalId: request.proposal.proposalId
      };
      mockVersions.unshift({ ...version, profile: structuredClone(next) });
      return { resume: structuredClone(next), version };
    }
    return invoke('apply_resume_chat_edits', { request });
  },

  async listResumeVersions(resumeId: string): Promise<ResumeVersionSummary[]> {
    if (browserMode())
      return structuredClone(
        mockVersions
          .filter((item) => item.resumeId === resumeId)
          .map(({ profile: _, ...item }) => item)
      );
    return invoke('list_resume_versions', { resumeId });
  },

  async getResumeVersion(versionId: string): Promise<ResumeVersionDetail> {
    if (browserMode()) {
      const version = mockVersions.find((item) => item.id === versionId);
      if (!version) throw new Error('简历版本不存在');
      return structuredClone(version);
    }
    return invoke('get_resume_version', { versionId });
  },

  async restoreResumeVersion(
    versionId: string,
    expectedVersion: number
  ): Promise<ResumeCommitResult> {
    if (browserMode()) {
      const version = mockVersions.find((item) => item.id === versionId);
      if (!version || !mockState.resume) throw new Error('简历版本不存在');
      if (mockState.resume.version !== expectedVersion)
        throw new Error('version_conflict: 简历已变化');
      const restored = {
        ...structuredClone(version.profile),
        version: expectedVersion + 1,
        updatedAt: new Date().toISOString(),
        preferences: structuredClone(mockState.resume.preferences)
      };
      mockState.resume = restored;
      const summary: ResumeVersionSummary = {
        id: crypto.randomUUID(),
        resumeId: restored.id,
        version: restored.version,
        parentVersion: expectedVersion,
        createdAt: restored.updatedAt,
        source: 'rollback',
        summary: `恢复到 v${version.version} 的内容`,
        restoredFromVersion: version.version
      };
      mockVersions.unshift({ ...summary, profile: structuredClone(restored) });
      return { resume: structuredClone(restored), version: summary };
    }
    return invoke('restore_resume_version', { versionId, expectedVersion });
  },

  async saveProvider(provider: AiProviderConfig): Promise<ProviderSaveResult> {
    if (browserMode()) {
      const existing = mockState.providers.find((item) => item.id === provider.id);
      const ok = Boolean(
        (provider.apiKey || existing?.apiKeyRef) && provider.baseUrl && provider.model
      );
      if (!ok) throw new Error('请填写 API Key、Base URL 和模型名');
      const testResult: ProviderTestResult = {
        ok: true,
        message: '连接成功，结构化输出正常',
        latencyMs: 684,
        structuredOutput: true,
        visionSupported: true,
        visionMessage: '图片识别能力正常'
      };
      mockState.providers = mockState.providers.map((item) =>
        item.id === provider.id
          ? {
              ...provider,
              apiKey: undefined,
              apiKeyRef: provider.apiKey ? `keychain://${provider.id}` : item.apiKeyRef,
              verified: true,
              visionVerified: true,
              lastTestedAt: new Date().toISOString(),
              lastTestError: null
            }
          : provider.isDefault
            ? { ...item, isDefault: false }
            : item
      );
      mockState.readiness.ai = true;
      mockState.configuration.llm = {
        state: 'ready',
        message: '默认模型已验证。',
        lastAttemptAt: new Date().toISOString()
      };
      return structuredClone({ providers: mockState.providers, testResult });
    }
    return invoke('save_provider', { provider });
  },

  async testProvider(provider: AiProviderConfig): Promise<ProviderTestResult> {
    if (browserMode()) {
      await new Promise((resolve) => window.setTimeout(resolve, 700));
      const existing = mockState.providers.find((item) => item.id === provider.id);
      const ok = Boolean(
        (provider.apiKey || existing?.apiKeyRef) && provider.baseUrl && provider.model
      );
      return {
        ok,
        message: ok ? '连接成功，结构化输出正常' : '请填写 API Key、Base URL 和模型名',
        latencyMs: 684,
        structuredOutput: ok,
        visionSupported: ok,
        visionMessage: ok ? '图片识别能力正常' : '未通过图片识别测试'
      };
    }
    return invoke('test_provider', { provider });
  },

  async saveSettings(settings: AppSettings): Promise<AppSettings> {
    if (browserMode()) {
      mockState.settings = structuredClone(settings);
      return structuredClone(settings);
    }
    return invoke('save_settings', { settings });
  },

  async getAppInfo(): Promise<AppInfo> {
    if (browserMode()) {
      return {
        version: packageMetadata.version,
        identifier: 'io.github.aijobapp',
        os: 'browser',
        arch: 'demo',
        webview: navigator.userAgent,
        schemaVersion: null,
        sidecarProtocol: 'demo',
        chrome: { installed: true, version: '浏览器演示', executablePath: null },
        dataDir: '<browser-demo>',
        legacyDataDetected: false,
        lastUpdateCheckAt: mockState.settings.lastUpdateCheckAt
      };
    }
    return invoke('get_app_info');
  },

  async openGitHubIssues(): Promise<void> {
    if (browserMode()) {
      window.open('https://github.com/kyle-kw/ai-job-app/issues', '_blank', 'noopener,noreferrer');
      return;
    }
    return invoke('open_github_issues');
  },

  async checkForUpdate(manual = true): Promise<AppUpdateInfo | null> {
    if (browserMode()) return null;
    return invoke('check_for_update', { manual });
  },

  async downloadAndInstallUpdate(onEvent: (event: UpdateEvent) => void): Promise<void> {
    if (browserMode()) throw new Error('更新安装仅桌面版可用');
    const onEventChannel = new Channel<UpdateEvent>();
    onEventChannel.onmessage = onEvent;
    return invoke('download_and_install_update', { onEvent: onEventChannel });
  },

  async createBackup(outputPath: string): Promise<BackupInfo> {
    if (browserMode()) throw new Error('备份导出仅桌面版可用');
    return invoke('create_backup', { outputPath });
  },

  async restoreBackup(backupPath: string): Promise<void> {
    if (browserMode()) throw new Error('备份恢复仅桌面版可用');
    return invoke('restore_backup', { backupPath });
  },

  async listAutomaticBackups(): Promise<BackupInfo[]> {
    if (browserMode()) return [];
    return invoke('list_automatic_backups');
  },

  async clearData(scope: ClearDataScope): Promise<ClearDataResult> {
    if (browserMode()) {
      return {
        complete: true,
        items: [{ item: scope, ok: true, message: '浏览器演示不会修改任何数据' }],
        restartRequired: false
      };
    }
    return invoke('clear_data', { scope });
  },

  async exportDiagnostics(outputPath: string): Promise<string> {
    if (browserMode()) throw new Error('诊断导出仅桌面版可用');
    return invoke('export_diagnostics', { outputPath });
  },

  async restartApp(): Promise<void> {
    if (browserMode()) return;
    return invoke('restart_app');
  },

  async exitApp(): Promise<void> {
    if (browserMode()) return;
    return invoke('exit_app');
  }
};
