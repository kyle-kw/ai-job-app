import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import { Channel, invoke } from '@tauri-apps/api/core';

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

import type { Backend } from './backend-contract';

export const tauriBackend = {
  async bootstrap(): Promise<BootstrapSnapshot> {
    return invoke('bootstrap');
  },
  async listJobsPage(query: JobQuery): Promise<JobPage> {
    return invoke('list_jobs_page', { query });
  },
  async listJobOptions(query = ''): Promise<JobOption[]> {
    return invoke('list_job_options', { query });
  },
  async listJobFilterOptions(): Promise<JobFilterOptions> {
    return invoke('list_job_filter_options');
  },
  async getJob(jobId: string): Promise<Job> {
    return invoke('get_job', { jobId });
  },
  async deleteJob(jobId: string): Promise<DeleteJobsResult> {
    return invoke('delete_job', { jobId });
  },
  async deleteMissingDescriptionJobs(query: JobQuery): Promise<DeleteJobsResult> {
    return invoke('delete_missing_description_jobs', { query });
  },
  async listReportKeywords(): Promise<ReportKeyword[]> {
    return invoke('list_report_keywords');
  },
  async getJobDataReport(keywordKeys: string[]): Promise<JobDataReport> {
    return invoke('get_job_data_report', { keywordKeys });
  },
  async exportJobsJson(outputPath: string, query?: JobQuery): Promise<RenderResult> {
    return invoke('export_jobs_json', { outputPath, query: query ?? null });
  },
  async exportJobDataReport(keywordKeys: string[], outputPath: string): Promise<RenderResult> {
    return invoke('export_job_data_report', { keywordKeys, outputPath });
  },
  async getReportCompetitivenessState(keywordKeys: string[]): Promise<ReportCompetitivenessState> {
    return invoke('get_report_competitiveness_state', { keywordKeys });
  },
  async generateReportCompetitiveness(
    keywordKeys: string[],
    force = false
  ): Promise<ReportCompetitivenessState> {
    return invoke('generate_report_competitiveness', { keywordKeys, force });
  },
  async getInterviewPreparationState(keywordKeys: string[]): Promise<InterviewPreparationState> {
    return invoke('get_interview_preparation_state', { keywordKeys });
  },
  async generateInterviewPreparation(
    keywordKeys: string[],
    force = false
  ): Promise<InterviewPreparationState> {
    return invoke('generate_interview_preparation', { keywordKeys, force });
  },
  async onTaskEvent(callback: (event: TaskEvent) => void): Promise<UnlistenFn> {
    return listen<TaskEvent>('task://event', (event) => callback(event.payload));
  },
  async startScrape(spec: SearchSpec): Promise<string> {
    return invoke('start_scrape', { spec });
  },
  async startJobDetailExtraction(): Promise<string> {
    return invoke('start_job_detail_extraction');
  },
  async setupBoss(options: { resetProfile: boolean }): Promise<string> {
    return invoke('setup_boss', { resetProfile: options.resetProfile });
  },
  async importResume(payload: ImportResumePayload): Promise<string> {
    return invoke('import_resume', { payload });
  },
  async saveResume(resume: ResumeProfile): Promise<ResumeProfile> {
    return invoke('save_resume', { resume });
  },
  async listResumeVariants(): Promise<ResumeVariantSummary[]> {
    return invoke('list_resume_variants');
  },
  async getResumeVariant(variantId: string): Promise<ResumeVariantDetail> {
    return invoke('get_resume_variant', { variantId });
  },
  async createResumeVariant(
    jobId: string,
    expectedResumeVersion: number
  ): Promise<ResumeVariantDetail> {
    return invoke('create_resume_variant', { jobId, expectedResumeVersion });
  },
  async saveResumeVariant(
    variantId: string,
    resume: ResumeProfile,
    expectedVersion: number
  ): Promise<ResumeVariantCommitResult> {
    return invoke('save_resume_variant', { variantId, resume, expectedVersion });
  },
  async deleteResumeVariant(variantId: string): Promise<number> {
    return invoke('delete_resume_variant', { variantId });
  },
  async previewResumeVariantRebase(variantId: string): Promise<ResumeRebasePreview> {
    return invoke('preview_resume_variant_rebase', { variantId });
  },
  async applyResumeVariantRebase(
    variantId: string,
    expectedVariantVersion: number,
    expectedMasterVersion: number,
    resolutions: ResumeRebaseResolution[]
  ): Promise<ResumeVariantCommitResult> {
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
    return invoke('restore_resume_variant_version', { variantId, versionId, expectedVersion });
  },
  async createResumeFromTemplate(templateId: ResumeTemplateId): Promise<ResumeProfile> {
    return invoke('create_resume_from_template', { templateId });
  },
  async savePreferences(preferences: JobPreferences): Promise<ResumeProfile> {
    return invoke('save_preferences', { preferences });
  },
  async analyzeJob(jobId: string, force = false): Promise<FitAnalysisResult> {
    return invoke('analyze_job', { jobId, force });
  },
  async startFitBatch(jobIds: string[]): Promise<string> {
    return invoke('start_fit_batch', { jobIds });
  },
  async startFitBatchForQuery(query: JobQuery): Promise<string> {
    return invoke('start_fit_batch_for_query', { query });
  },
  async openJobSource(jobId: string): Promise<void> {
    return invoke('open_job_source', { jobId });
  },
  async generateGreeting(jobId: string): Promise<string> {
    return invoke('generate_greeting', { jobId });
  },
  async renderResume(request: RenderResumeRequest): Promise<RenderResult> {
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
    return invoke('analyze_resume_coverage', { target, force });
  },
  async proposeResumeChatEdits(request: ResumeChatRequest): Promise<ResumeChatProposal> {
    return invoke('propose_resume_chat_edits', { request });
  },
  async applyResumeChatEdits(request: ApplyResumeEditsRequest): Promise<ResumeEditCommitResult> {
    return invoke('apply_resume_chat_edits', { request });
  },
  async listResumeVersions(resumeId: string): Promise<ResumeVersionSummary[]> {
    return invoke('list_resume_versions', { resumeId });
  },
  async getResumeVersion(versionId: string): Promise<ResumeVersionDetail> {
    return invoke('get_resume_version', { versionId });
  },
  async restoreResumeVersion(
    versionId: string,
    expectedVersion: number
  ): Promise<ResumeCommitResult> {
    return invoke('restore_resume_version', { versionId, expectedVersion });
  },
  async saveProvider(provider: AiProviderConfig): Promise<ProviderSaveResult> {
    return invoke('save_provider', { provider });
  },
  async testProvider(provider: AiProviderConfig): Promise<ProviderTestResult> {
    return invoke('test_provider', { provider });
  },
  async saveSettings(settings: AppSettings): Promise<AppSettings> {
    return invoke('save_settings', { settings });
  },
  async getAppInfo(): Promise<AppInfo> {
    return invoke('get_app_info');
  },
  async openGitHubIssues(): Promise<void> {
    return invoke('open_github_issues');
  },
  async checkForUpdate(manual = true): Promise<AppUpdateInfo | null> {
    return invoke('check_for_update', { manual });
  },
  async downloadAndInstallUpdate(onEvent: (event: UpdateEvent) => void): Promise<void> {
    const onEventChannel = new Channel<UpdateEvent>();
    onEventChannel.onmessage = onEvent;
    return invoke('download_and_install_update', { onEvent: onEventChannel });
  },
  async createBackup(outputPath: string): Promise<BackupInfo> {
    return invoke('create_backup', { outputPath });
  },
  async restoreBackup(backupPath: string): Promise<void> {
    return invoke('restore_backup', { backupPath });
  },
  async listAutomaticBackups(): Promise<BackupInfo[]> {
    return invoke('list_automatic_backups');
  },
  async clearData(scope: ClearDataScope): Promise<ClearDataResult> {
    return invoke('clear_data', { scope });
  },
  async exportDiagnostics(outputPath: string): Promise<string> {
    return invoke('export_diagnostics', { outputPath });
  },
  async restartApp(): Promise<void> {
    return invoke('restart_app');
  },
  async exitApp(): Promise<void> {
    return invoke('exit_app');
  }
} satisfies Backend;
