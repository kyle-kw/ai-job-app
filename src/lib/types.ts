export type TaskKind = 'scrape' | 'job-detail-extraction' | 'resume-import' | 'fit' | 'tailor' | 'render' | 'provider-test' | 'boss-login';
export type TaskState = 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface TaskRun {
  id: string;
  kind: TaskKind;
  title: string;
  state: TaskState;
  progress: number;
  message: string;
  recoverableError?: string | null;
  createdAt: string;
  updatedAt: string;
  logs: string[];
}

export interface TaskEvent extends TaskRun {}

export interface SearchSpec {
  keyword: string;
  city: string;
  pages: number;
  salary?: string;
  experience?: string;
  degree?: string;
  companyScale?: string;
}

export interface Job {
  id: string;
  source: 'boss';
  externalId: string;
  title: string;
  company: string;
  salary: string;
  location: string;
  experience: string;
  degree: string;
  companyScale: string;
  companyStage: string;
  industry: string;
  skills: string[];
  welfare: string[];
  description: string;
  sourceUrl: string;
  bossName?: string;
  bossTitle?: string;
  firstSeen: string;
  lastSeen: string;
  isNew: boolean;
  fit?: FitReport | null;
  greeting?: string | null;
  patches?: ResumePatch[];
  structuredDetails?: JobStructuredDetails | null;
}

export interface BusinessInformation {
  companyName: string;
  legalRepresentative: string;
  establishedDate: string;
  companyType: string;
  operatingStatus: string;
  registeredCapital: string;
}

export interface JobStructuredDetails {
  jobDescription: string;
  responsibilities: string[];
  requirements: string[];
  companyIntroduction: string;
  businessInformation: BusinessInformation;
  extractedAt: string;
  extractorVersion: string;
}

export type FitDimensionKey = 'technical' | 'experience' | 'behavior' | 'career';

export interface FitDimension {
  key: FitDimensionKey;
  label: string;
  score: number | null;
  weight: number;
  note: string;
  evidence: string[];
}

export interface HardConstraint {
  label: string;
  status: 'pass' | 'flag' | 'fail' | 'unknown';
  note: string;
}

export interface FitReport {
  overallScore: number;
  confidence: number;
  verdict: 'strong' | 'good' | 'moderate' | 'weak' | 'poor';
  recommendation: string;
  summary: string;
  dimensions: FitDimension[];
  hardConstraints: HardConstraint[];
  strengths: string[];
  gaps: string[];
  evidence: string[];
  generatedAt: string;
  skillVersion: string;
  inputHash?: string;
  analysisSource?: 'llm' | 'local' | 'legacy';
  fallbackReason?: 'provider_missing' | 'llm_failed' | 'invalid_output' | null;
  cacheStatus?: 'fresh' | 'stale' | 'legacy';
}

export type ResumeFactCategory = 'identity' | 'experience' | 'education' | 'skill' | 'project' | 'certification' | 'other';

export interface ResumeFact {
  id: string;
  category: ResumeFactCategory;
  value: string;
  source: string;
  confidence: number;
  confirmed: boolean;
}

export interface ResumeExperience {
  id: string;
  company: string;
  position: string;
  location: string;
  startDate: string;
  endDate: string;
  highlights: string[];
}

export type ResumeDegree = '' | '本科' | '硕士' | '博士' | '其他';

export interface ResumeEducation {
  id: string;
  institution: string;
  area: string;
  degree: ResumeDegree;
  degreeDetail: string;
  startDate: string;
  endDate: string;
  highlights: string[];
}

export type ResumeTemplateId = 'general' | 'ai-engineering' | 'data-analysis' | 'finance-accounting';

export interface ProfessionalSkillGroup {
  id: string;
  label: string;
  items: string[];
}

export interface ResumeProject {
  id: string;
  name: string;
  summary: string;
  startDate: string;
  endDate: string;
  highlights: string[];
}

export interface ResumeCertification {
  id: string;
  name: string;
  issuer: string;
  date: string;
}

export interface JobPreferences {
  targetRoles: string[];
  cities: string[];
  remotePreference: 'onsite' | 'hybrid' | 'remote' | 'flexible';
  energizingTasks: string[];
  drainingTasks: string[];
  hardConstraints: string[];
}

export interface ResumeProfile {
  id: string;
  name: string;
  headline: string;
  email: string;
  phone: string;
  location: string;
  website: string;
  summary: string;
  templateId: ResumeTemplateId;
  professionalSkills: ProfessionalSkillGroup[];
  experiences: ResumeExperience[];
  education: ResumeEducation[];
  projects: ResumeProject[];
  certifications: ResumeCertification[];
  facts: ResumeFact[];
  preferences: JobPreferences;
  sourceFileName: string;
  updatedAt: string;
  version: number;
}

export interface ResumePatch {
  id: string;
  jobId: string;
  section: string;
  before: string;
  after: string;
  rationale: string;
  evidenceFactIds: string[];
  status: 'pending' | 'accepted' | 'rejected';
}

export type ProviderKind = 'xiaomi' | 'custom';

export interface AiProviderConfig {
  id: string;
  kind: ProviderKind;
  name: string;
  baseUrl: string;
  model: string;
  allowInsecureHttp: boolean;
  apiKey?: string;
  apiKeyRef?: string;
  isDefault: boolean;
  verified: boolean;
  visionVerified: boolean;
  lastTestedAt?: string | null;
  lastTestError?: string | null;
}

export interface ScrapeRun {
  id: string;
  keyword: string;
  city: string;
  totalSeen: number;
  inserted: number;
  updated: number;
  startedAt: string;
  completedAt?: string | null;
  reportMarkdown?: string | null;
  searchSpec?: SearchSpec | null;
  resolvedCity?: string | null;
  detailSummary?: ScrapeDetailSummary | null;
  sample?: ScrapeSampleSummary | null;
}

export interface ScrapeDetailSummary {
  total: number;
  processed: number;
  succeeded: number;
  skipped: number;
  failed: number;
}

export interface ScrapeSampleSummary {
  jobIds: string[];
  totalJobs: number;
  detailJobs: number;
  detailCoverage: number;
  salarySampleCount: number;
  medianSalaryK?: number | null;
  skillSampleCount: number;
  skillCoverage: number;
  skills: ReportBucket[];
}

export interface Readiness {
  ai: boolean;
  resume: boolean;
  boss: boolean;
}

export type ConfigurationState = 'needs_setup' | 'running' | 'ready' | 'failed';

export interface ConfigurationItem {
  state: ConfigurationState;
  message: string;
  lastAttemptAt?: string | null;
}

export interface ConfigurationSnapshot {
  boss: ConfigurationItem;
  llm: ConfigurationItem;
}

export interface AppSettings {
  advancedMode: boolean;
  automaticUpdateChecks: boolean;
  privacyAcknowledgedVersion?: string | null;
  lastUpdateCheckAt?: string | null;
}

export interface ChromeStatus {
  installed: boolean;
  version?: string | null;
  executablePath?: string | null;
}

export interface AppInfo {
  version: string;
  identifier: string;
  os: string;
  arch: string;
  webview: string;
  schemaVersion: number;
  sidecarProtocol: string;
  chrome: ChromeStatus;
  dataDir: string;
  legacyDataDetected: boolean;
  lastUpdateCheckAt?: string | null;
}

export interface AppUpdateInfo {
  version: string;
  currentVersion: string;
  publishedAt?: string | null;
  notes: string;
  downloadSize?: number | null;
}

export interface UpdateEvent {
  event: 'started' | 'progress' | 'downloaded' | 'finished';
  downloaded: number;
  total?: number | null;
  message?: string | null;
}

export interface BackupInfo {
  fileName: string;
  path: string;
  size: number;
  createdAt: string;
  sourceVersion: string;
}

export type ClearDataScope = 'modelKeys' | 'bossProfile' | 'legacyData' | 'all';

export interface ClearDataItemResult {
  item: string;
  ok: boolean;
  message: string;
}

export interface ClearDataResult {
  complete: boolean;
  items: ClearDataItemResult[];
  restartRequired: boolean;
}

export interface BootstrapSnapshot {
  readiness: Readiness;
  configuration: ConfigurationSnapshot;
  resume: ResumeProfile | null;
  providers: AiProviderConfig[];
  tasks: TaskRun[];
  scrapeRuns: ScrapeRun[];
  lastSearchSpec: SearchSpec | null;
  settings: AppSettings;
}

export interface JobQuery {
  query: string;
  minScore: number;
  onlyNew: boolean;
  salary: '' | '402' | '403' | '404' | '405' | '406' | '407';
  companyScale: '' | '301' | '302' | '303' | '304' | '305' | '306';
  city: string;
  missingDescription: boolean;
  keywordKeys?: string[];
  skills?: string[];
  experience?: string;
  salaryBand?: ReportSalaryBand;
  cursor?: string | null;
}

export interface JobPage {
  items: Job[];
  total: number;
  pendingDetailCount: number;
  nextCursor?: string | null;
}

export interface JobOption {
  id: string;
  title: string;
  company: string;
  lastSeen: string;
}

export interface ProviderTestResult {
  ok: boolean;
  message: string;
  latencyMs: number;
  structuredOutput: boolean;
  visionSupported: boolean;
  visionMessage: string;
}

export interface ProviderSaveResult {
  providers: AiProviderConfig[];
  testResult: ProviderTestResult;
}

export interface FitAnalysisResult {
  job: Job;
  cacheHit: boolean;
  source: 'llm' | 'local';
  warning?: string | null;
}

export interface ImportResumePayload {
  fileName: string;
  contentBase64: string;
}

export interface RenderResult {
  path: string;
  fileName: string;
}

export interface DeleteJobsResult {
  deletedCount: number;
}

export type ResumeColorTheme = 'pine' | 'navy' | 'graphite';

export interface RenderResumeRequest {
  outputPath: string;
  colorTheme: ResumeColorTheme;
  target?: ResumeTargetRef;
}

export interface ReportBucket {
  label: string;
  count: number;
  percentage: number;
}

export interface ReportKeyword {
  key: string;
  label: string;
  jobCount: number;
  lastSeen: string;
}

export interface SalarySummary {
  sampleCount: number;
  medianLowK?: number | null;
  medianMidK?: number | null;
  medianHighK?: number | null;
  extraMonthsCount: number;
  bands: ReportBucket[];
}

export interface SalaryByExperience {
  label: string;
  count: number;
  medianK: number;
}

export type ReportSalaryBand = '' | 'under-15' | '15-25' | '25-35' | '35-50' | '50-plus';

export interface ReportSampleMetric { count: number; coverage: number; }

export interface ReportSampleQuality {
  detail: ReportSampleMetric;
  salary: ReportSampleMetric;
  skill: ReportSampleMetric;
  experience: ReportSampleMetric;
  degree: ReportSampleMetric;
  limitations: string[];
}

export interface ReportBatchSnapshot {
  runId: string;
  completedAt: string;
  searchSpec: SearchSpec;
  totalJobs: number;
  detailCoverage: number;
  salarySampleCount: number;
  medianSalaryK?: number | null;
}

export interface ReportBatchSkillChange {
  label: string;
  currentCount: number;
  currentPercentage: number;
  previousCount: number;
  previousPercentage: number;
  deltaPercentagePoints: number;
}

export interface ReportBatchComparison {
  status: 'available' | 'unavailable';
  reason?: 'multi_keyword' | 'no_captured_run' | 'no_comparable_run' | null;
  current?: ReportBatchSnapshot | null;
  previous?: ReportBatchSnapshot | null;
  jobCountChangePercentage?: number | null;
  newlyObservedJobs: number;
  notObservedJobs: number;
  salaryMedianDeltaK?: number | null;
  skillChanges: ReportBatchSkillChange[];
}

export interface JobDataReport {
  generatedAt: string;
  selectedKeywords: ReportKeyword[];
  dataFrom?: string | null;
  dataTo?: string | null;
  totalJobs: number;
  totalCompanies: number;
  totalCities: number;
  detailJobs: number;
  detailCoverage: number;
  salary: SalarySummary;
  experience: ReportBucket[];
  degree: ReportBucket[];
  roles: ReportBucket[];
  cities: ReportBucket[];
  industries: ReportBucket[];
  companyScales: ReportBucket[];
  topSkills: ReportBucket[];
  skillPairs: ReportBucket[];
  welfare: ReportBucket[];
  salaryByExperience: SalaryByExperience[];
  insights: string[];
  sampleQuality: ReportSampleQuality;
  batchComparison: ReportBatchComparison;
}

export interface ReportCompetitivenessItem {
  id: string;
  label: string;
  jobCount: number;
  percentage: number;
  status: ResumeCoverageStatus;
  resumePaths: string[];
  evidenceFactIds: string[];
  rationale: string;
}

export interface ReportCompetitivenessAnalysis {
  source: 'local' | 'ai';
  resumeId: string;
  resumeVersion: number;
  generatedAt: string;
  items: ReportCompetitivenessItem[];
}

export interface ReportCompetitivenessState {
  status: 'missing' | 'fresh' | 'stale';
  reason?: 'no_provider' | 'no_resume' | 'no_jobs' | 'data_changed' | string | null;
  hasResume: boolean;
  hasProvider: boolean;
  generatedAt?: string | null;
  local?: ReportCompetitivenessAnalysis | null;
  ai?: ReportCompetitivenessAnalysis | null;
  effectiveSource?: 'local' | 'ai' | null;
}

export interface InterviewPreparationSkill {
  name: string;
  gap?: string;
  action: string;
  jobCount?: number;
}

export interface InterviewPreparation {
  summary: string;
  skills: InterviewPreparationSkill[];
  projectIdeas: string[];
  practiceQuestions: string[];
}

export interface InterviewPreparationState {
  status: 'missing' | 'fresh' | 'stale';
  reason?: 'no_provider' | 'no_resume' | 'no_jobs' | string | null;
  hasProvider: boolean;
  hasResume: boolean;
  generatedAt?: string | null;
  preparation?: InterviewPreparation | null;
}

export interface ResumeChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

export interface ResumeTargetRef {
  kind: 'master' | 'variant';
  id: string;
}

export interface ResumeVariantSummary {
  id: string;
  jobId: string;
  jobTitle: string;
  company: string;
  name: string;
  baseResumeId: string;
  baseResumeVersion: number;
  version: number;
  createdAt: string;
  updatedAt: string;
  stale: boolean;
}

export interface ResumeVariantDetail extends ResumeVariantSummary {
  profile: ResumeProfile;
}

export interface ResumeVariantCommitResult {
  variant: ResumeVariantDetail;
  version: ResumeVersionSummary;
}

export type ResumeHealthSeverity = 'error' | 'warning' | 'suggestion';

export interface ResumeHealthIssue {
  id: string;
  code: string;
  severity: ResumeHealthSeverity;
  path: string;
  label: string;
  message: string;
}

export interface ResumeHealthReport {
  issues: ResumeHealthIssue[];
  errorCount: number;
  warningCount: number;
  suggestionCount: number;
}

export type ResumeCoverageStatus = 'covered' | 'strengthenable' | 'gap' | 'unknown';

export interface ResumeCoverageItem {
  id: string;
  label: string;
  kind: 'skill' | 'requirement';
  status: ResumeCoverageStatus;
  resumePaths: string[];
  evidenceFactIds: string[];
  rationale: string;
}

export interface ResumeCoverageReport {
  jobId: string;
  target: ResumeTargetRef;
  targetVersion: number;
  source: 'local' | 'ai';
  generatedAt: string;
  items: ResumeCoverageItem[];
  coveredCount: number;
  strengthenableCount: number;
  gapCount: number;
  unknownCount: number;
}

export interface ResumeRebaseChange {
  path: string;
  label: string;
  base: unknown;
  master: unknown;
  variant: unknown;
}

export interface ResumeRebasePreview {
  variantId: string;
  variantVersion: number;
  baseResumeVersion: number;
  masterVersion: number;
  autoChanges: ResumeRebaseChange[];
  conflicts: ResumeRebaseChange[];
}

export interface ResumeRebaseResolution {
  path: string;
  choice: 'variant' | 'master';
}

export interface ResumeFactCandidate {
  id: string;
  category: ResumeFact['category'];
  value: string;
  sourceMessageId?: string | null;
}

export interface ResumeFieldEdit {
  id: string;
  path: string;
  label: string;
  operation: 'replace';
  before: unknown;
  after: unknown;
  rationale: string;
  evidenceFactIds: string[];
  requiredFactCandidateIds: string[];
}

export interface ResumeChatProposal {
  proposalId: string;
  target: ResumeTargetRef;
  baseVersion: number;
  job?: { id: string; title: string; company: string } | null;
  marketContext?: ResumeChatMarketContext | null;
  assistantMessage: string;
  edits: ResumeFieldEdit[];
  factCandidates: ResumeFactCandidate[];
  warnings: string[];
}

export interface MarketResumeContextRequest {
  keywordKeys: string[];
  focusSkills: string[];
}

export interface ResumeChatMarketSkill {
  label: string;
  jobCount: number;
  percentage: number;
  status: ResumeCoverageStatus;
  rationale: string;
}

export interface ResumeChatMarketContext {
  keywordKeys: string[];
  keywordLabels: string[];
  totalJobs: number;
  skills: ResumeChatMarketSkill[];
}

export interface ResumeChatRequest {
  target: ResumeTargetRef;
  expectedVersion: number;
  jobId?: string | null;
  marketContext?: MarketResumeContextRequest | null;
  messages: ResumeChatMessage[];
}

export interface ApplyResumeEditsRequest {
  proposal: ResumeChatProposal;
  selectedEditIds: string[];
  confirmedFactCandidateIds: string[];
  expectedVersion: number;
}

export type ResumeEditCommitResult = ResumeCommitResult | ResumeVariantCommitResult;

export type ResumeVersionSource = 'legacy' | 'import' | 'template' | 'manual' | 'ai-chat' | 'market-ai-chat' | 'rollback'
  | 'variant-create' | 'variant-manual' | 'variant-ai' | 'variant-rebase' | 'variant-rollback';

export interface ResumeVersionSummary {
  id: string;
  resumeId: string;
  version: number;
  parentVersion?: number | null;
  createdAt: string;
  source: ResumeVersionSource;
  summary: string;
  jobId?: string | null;
  proposalId?: string | null;
  restoredFromVersion?: number | null;
}

export interface ResumeVersionDetail extends ResumeVersionSummary {
  profile: ResumeProfile;
}

export interface ResumeCommitResult {
  resume: ResumeProfile;
  version: ResumeVersionSummary;
}
