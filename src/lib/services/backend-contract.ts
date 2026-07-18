import { type UnlistenFn } from '@tauri-apps/api/event';

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
  UpdateEvent
} from '$lib/types';

export interface Backend {
  bootstrap(): Promise<BootstrapSnapshot>;
  listJobsPage(query: JobQuery): Promise<JobPage>;
  listJobOptions(query?: string): Promise<JobOption[]>;
  listJobFilterOptions(): Promise<JobFilterOptions>;
  getJob(jobId: string): Promise<Job>;
  deleteJob(jobId: string): Promise<DeleteJobsResult>;
  deleteMissingDescriptionJobs(query: JobQuery): Promise<DeleteJobsResult>;
  listReportKeywords(): Promise<ReportKeyword[]>;
  getJobDataReport(keywordKeys: string[]): Promise<JobDataReport>;
  exportJobsJson(outputPath: string, query?: JobQuery): Promise<RenderResult>;
  exportJobDataReport(keywordKeys: string[], outputPath: string): Promise<RenderResult>;
  getReportCompetitivenessState(keywordKeys: string[]): Promise<ReportCompetitivenessState>;
  generateReportCompetitiveness(
    keywordKeys: string[],
    force?: boolean
  ): Promise<ReportCompetitivenessState>;
  getInterviewPreparationState(keywordKeys: string[]): Promise<InterviewPreparationState>;
  generateInterviewPreparation(
    keywordKeys: string[],
    force?: boolean
  ): Promise<InterviewPreparationState>;
  onTaskEvent(callback: (event: TaskEvent) => void): Promise<UnlistenFn>;
  startScrape(spec: SearchSpec): Promise<string>;
  startJobDetailExtraction(): Promise<string>;
  setupBoss(options: { resetProfile: boolean }): Promise<string>;
  importResume(payload: ImportResumePayload): Promise<string>;
  saveResume(resume: ResumeProfile): Promise<ResumeProfile>;
  listResumeVariants(): Promise<ResumeVariantSummary[]>;
  getResumeVariant(variantId: string): Promise<ResumeVariantDetail>;
  createResumeVariant(jobId: string, expectedResumeVersion: number): Promise<ResumeVariantDetail>;
  saveResumeVariant(
    variantId: string,
    resume: ResumeProfile,
    expectedVersion: number
  ): Promise<ResumeVariantCommitResult>;
  deleteResumeVariant(variantId: string): Promise<number>;
  previewResumeVariantRebase(variantId: string): Promise<ResumeRebasePreview>;
  applyResumeVariantRebase(
    variantId: string,
    expectedVariantVersion: number,
    expectedMasterVersion: number,
    resolutions: ResumeRebaseResolution[]
  ): Promise<ResumeVariantCommitResult>;
  restoreResumeVariantVersion(
    variantId: string,
    versionId: string,
    expectedVersion: number
  ): Promise<ResumeVariantCommitResult>;
  createResumeFromTemplate(templateId: ResumeTemplateId): Promise<ResumeProfile>;
  savePreferences(preferences: JobPreferences): Promise<ResumeProfile>;
  analyzeJob(jobId: string, force?: boolean): Promise<FitAnalysisResult>;
  startFitBatch(jobIds: string[]): Promise<string>;
  startFitBatchForQuery(query: JobQuery): Promise<string>;
  openJobSource(jobId: string): Promise<void>;
  generateGreeting(jobId: string): Promise<string>;
  renderResume(request: RenderResumeRequest): Promise<RenderResult>;
  analyzeResumeCoverage(target: ResumeTargetRef, force?: boolean): Promise<ResumeCoverageReport>;
  proposeResumeChatEdits(request: ResumeChatRequest): Promise<ResumeChatProposal>;
  applyResumeChatEdits(request: ApplyResumeEditsRequest): Promise<ResumeEditCommitResult>;
  listResumeVersions(resumeId: string): Promise<ResumeVersionSummary[]>;
  getResumeVersion(versionId: string): Promise<ResumeVersionDetail>;
  restoreResumeVersion(versionId: string, expectedVersion: number): Promise<ResumeCommitResult>;
  saveProvider(provider: AiProviderConfig): Promise<ProviderSaveResult>;
  testProvider(provider: AiProviderConfig): Promise<ProviderTestResult>;
  saveSettings(settings: AppSettings): Promise<AppSettings>;
  getAppInfo(): Promise<AppInfo>;
  openGitHubIssues(): Promise<void>;
  checkForUpdate(manual?: boolean): Promise<AppUpdateInfo | null>;
  downloadAndInstallUpdate(onEvent: (event: UpdateEvent) => void): Promise<void>;
  createBackup(outputPath: string): Promise<BackupInfo>;
  restoreBackup(backupPath: string): Promise<void>;
  listAutomaticBackups(): Promise<BackupInfo[]>;
  clearData(scope: ClearDataScope): Promise<ClearDataResult>;
  exportDiagnostics(outputPath: string): Promise<string>;
  restartApp(): Promise<void>;
  exitApp(): Promise<void>;
}
