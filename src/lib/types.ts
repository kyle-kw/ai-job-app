export type TaskKind = 'scrape' | 'resume-import' | 'fit' | 'tailor' | 'render' | 'provider-test' | 'boss-login';
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
}

export interface ResumeFact {
  id: string;
  category: 'identity' | 'experience' | 'education' | 'skill' | 'project' | 'other';
  value: string;
  source: string;
  confidence: number;
  confirmed: boolean;
}

export interface ResumeExperience {
  company: string;
  position: string;
  location: string;
  startDate: string;
  endDate: string;
  highlights: string[];
}

export interface ResumeEducation {
  institution: string;
  area: string;
  degree: string;
  startDate: string;
  endDate: string;
  highlights: string[];
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
  skills: string[];
  experiences: ResumeExperience[];
  education: ResumeEducation[];
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

export type ProviderKind = 'xiaomi' | 'openrouter' | 'custom';

export interface AiProviderConfig {
  id: string;
  kind: ProviderKind;
  name: string;
  baseUrl: string;
  model: string;
  apiKey?: string;
  apiKeyRef?: string;
  isDefault: boolean;
  verified: boolean;
  lastTestedAt?: string | null;
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
}

export interface Readiness {
  ai: boolean;
  resume: boolean;
  boss: boolean;
}

export interface AppSettings {
  locale: 'zh-CN' | 'en';
  theme: 'light' | 'dark' | 'system';
  advancedMode: boolean;
  telemetry: false;
  privacyAcknowledged: boolean;
}

export interface BootstrapSnapshot {
  readiness: Readiness;
  jobs: Job[];
  resume: ResumeProfile | null;
  providers: AiProviderConfig[];
  tasks: TaskRun[];
  scrapeRuns: ScrapeRun[];
  settings: AppSettings;
}

export interface ProviderTestResult {
  ok: boolean;
  message: string;
  latencyMs: number;
  structuredOutput: boolean;
}

export interface ImportResumePayload {
  fileName: string;
  contentBase64: string;
}

export interface RenderResult {
  path: string;
  fileName: string;
}

export interface ReportBucket {
  label: string;
  count: number;
  percentage: number;
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

export interface JobDataReport {
  generatedAt: string;
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
}
