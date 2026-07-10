use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSpec {
    pub keyword: String,
    pub city: String,
    pub pages: u8,
    #[serde(default)]
    pub salary: Option<String>,
    #[serde(default)]
    pub experience: Option<String>,
    #[serde(default)]
    pub degree: Option<String>,
    #[serde(default)]
    pub company_scale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitDimension {
    pub key: String,
    pub label: String,
    pub score: Option<i64>,
    pub weight: i64,
    pub note: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardConstraint {
    pub label: String,
    pub status: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitReport {
    pub overall_score: i64,
    pub confidence: i64,
    pub verdict: String,
    pub recommendation: String,
    pub summary: String,
    pub dimensions: Vec<FitDimension>,
    #[serde(default)]
    pub hard_constraints: Vec<HardConstraint>,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub generated_at: String,
    pub skill_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumePatch {
    pub id: String,
    pub job_id: String,
    pub section: String,
    pub before: String,
    pub after: String,
    pub rationale: String,
    #[serde(default)]
    pub evidence_fact_ids: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub source: String,
    pub external_id: String,
    pub title: String,
    pub company: String,
    pub salary: String,
    pub location: String,
    pub experience: String,
    pub degree: String,
    pub company_scale: String,
    pub company_stage: String,
    pub industry: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub welfare: Vec<String>,
    pub description: String,
    pub source_url: String,
    #[serde(default)]
    pub boss_name: Option<String>,
    #[serde(default)]
    pub boss_title: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
    #[serde(default)]
    pub is_new: bool,
    #[serde(default)]
    pub fit: Option<FitReport>,
    #[serde(default)]
    pub greeting: Option<String>,
    #[serde(default)]
    pub patches: Vec<ResumePatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeFact {
    pub id: String,
    pub category: String,
    pub value: String,
    pub source: String,
    pub confidence: f64,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeExperience {
    pub company: String,
    pub position: String,
    pub location: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeEducation {
    pub institution: String,
    pub area: String,
    pub degree: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JobPreferences {
    #[serde(default)]
    pub target_roles: Vec<String>,
    #[serde(default)]
    pub cities: Vec<String>,
    #[serde(default = "default_remote_preference")]
    pub remote_preference: String,
    #[serde(default)]
    pub energizing_tasks: Vec<String>,
    #[serde(default)]
    pub draining_tasks: Vec<String>,
    #[serde(default)]
    pub hard_constraints: Vec<String>,
}

fn default_remote_preference() -> String {
    "flexible".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeProfile {
    pub id: String,
    pub name: String,
    pub headline: String,
    pub email: String,
    pub phone: String,
    pub location: String,
    pub website: String,
    pub summary: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub experiences: Vec<ResumeExperience>,
    #[serde(default)]
    pub education: Vec<ResumeEducation>,
    #[serde(default)]
    pub facts: Vec<ResumeFact>,
    #[serde(default)]
    pub preferences: JobPreferences,
    pub source_file_name: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConfig {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
    pub is_default: bool,
    pub verified: bool,
    #[serde(default)]
    pub last_tested_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeRun {
    pub id: String,
    pub keyword: String,
    pub city: String,
    pub total_seen: i64,
    pub inserted: i64,
    pub updated: i64,
    pub started_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub report_markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub state: String,
    pub progress: i64,
    pub message: String,
    #[serde(default)]
    pub recoverable_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub ai: bool,
    pub resume: bool,
    pub boss: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub locale: String,
    pub theme: String,
    pub advanced_mode: bool,
    pub telemetry: bool,
    pub privacy_acknowledged: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: "zh-CN".into(),
            theme: "light".into(),
            advanced_mode: false,
            telemetry: false,
            privacy_acknowledged: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSnapshot {
    pub readiness: Readiness,
    pub jobs: Vec<Job>,
    pub resume: Option<ResumeProfile>,
    pub providers: Vec<AiProviderConfig>,
    pub tasks: Vec<TaskRun>,
    pub scrape_runs: Vec<ScrapeRun>,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResumePayload {
    pub file_name: String,
    pub content_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: i64,
    pub structured_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    pub path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportBucket {
    pub label: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SalarySummary {
    pub sample_count: i64,
    pub median_low_k: Option<f64>,
    pub median_mid_k: Option<f64>,
    pub median_high_k: Option<f64>,
    pub extra_months_count: i64,
    pub bands: Vec<ReportBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SalaryByExperience {
    pub label: String,
    pub count: i64,
    pub median_k: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDataReport {
    pub generated_at: String,
    pub data_from: Option<String>,
    pub data_to: Option<String>,
    pub total_jobs: i64,
    pub total_companies: i64,
    pub total_cities: i64,
    pub detail_jobs: i64,
    pub detail_coverage: f64,
    pub salary: SalarySummary,
    pub experience: Vec<ReportBucket>,
    pub degree: Vec<ReportBucket>,
    pub roles: Vec<ReportBucket>,
    pub cities: Vec<ReportBucket>,
    pub industries: Vec<ReportBucket>,
    pub company_scales: Vec<ReportBucket>,
    pub top_skills: Vec<ReportBucket>,
    pub skill_pairs: Vec<ReportBucket>,
    pub welfare: Vec<ReportBucket>,
    pub salary_by_experience: Vec<SalaryByExperience>,
    pub insights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarJobBatch {
    #[serde(default)]
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub report_markdown: Option<String>,
}
