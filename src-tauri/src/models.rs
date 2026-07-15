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
    #[serde(default)]
    pub input_hash: String,
    #[serde(default = "legacy_analysis_source")]
    pub analysis_source: String,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default = "legacy_cache_status")]
    pub cache_status: String,
}

fn legacy_analysis_source() -> String {
    "legacy".into()
}

fn legacy_cache_status() -> String {
    "legacy".into()
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessInformation {
    #[serde(default)]
    pub company_name: String,
    #[serde(default)]
    pub legal_representative: String,
    #[serde(default)]
    pub established_date: String,
    #[serde(default)]
    pub company_type: String,
    #[serde(default)]
    pub operating_status: String,
    #[serde(default)]
    pub registered_capital: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStructuredDetails {
    #[serde(default)]
    pub job_description: String,
    #[serde(default)]
    pub responsibilities: Vec<String>,
    #[serde(default)]
    pub requirements: Vec<String>,
    #[serde(default)]
    pub company_introduction: String,
    #[serde(default)]
    pub business_information: BusinessInformation,
    #[serde(default)]
    pub extracted_at: String,
    #[serde(default)]
    pub extractor_version: String,
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
    #[serde(default)]
    pub structured_details: Option<JobStructuredDetails>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobQuery {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub min_score: i64,
    #[serde(default)]
    pub only_new: bool,
    #[serde(default)]
    pub salary: String,
    #[serde(default)]
    pub company_scale: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub missing_description: bool,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPage {
    pub items: Vec<Job>,
    pub total: i64,
    pub pending_detail_count: i64,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobOption {
    pub id: String,
    pub title: String,
    pub company: String,
    pub last_seen: String,
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
    #[serde(default)]
    pub id: String,
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
    #[serde(default)]
    pub id: String,
    pub institution: String,
    pub area: String,
    pub degree: String,
    #[serde(default)]
    pub degree_detail: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfessionalSkillGroup {
    #[serde(default)]
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeProject {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeCertification {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub date: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProfessionalSkillsValue {
    Grouped(Vec<ProfessionalSkillGroup>),
    Legacy(Vec<String>),
}

fn deserialize_professional_skills<'de, D>(
    deserializer: D,
) -> Result<Vec<ProfessionalSkillGroup>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<ProfessionalSkillsValue>::deserialize(deserializer)?;
    Ok(match value {
        Some(ProfessionalSkillsValue::Grouped(groups)) => groups,
        Some(ProfessionalSkillsValue::Legacy(items)) if !items.is_empty() => {
            vec![ProfessionalSkillGroup {
                id: String::new(),
                label: "核心技能".into(),
                items,
            }]
        }
        _ => vec![],
    })
}

fn default_resume_template_id() -> String {
    "ai-engineering".into()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResumeColorTheme {
    Pine,
    Navy,
    Graphite,
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
    #[serde(default = "default_resume_template_id")]
    pub template_id: String,
    #[serde(
        default,
        alias = "skills",
        deserialize_with = "deserialize_professional_skills"
    )]
    pub professional_skills: Vec<ProfessionalSkillGroup>,
    #[serde(default)]
    pub experiences: Vec<ResumeExperience>,
    #[serde(default)]
    pub education: Vec<ResumeEducation>,
    #[serde(default)]
    pub projects: Vec<ResumeProject>,
    #[serde(default)]
    pub certifications: Vec<ResumeCertification>,
    #[serde(default)]
    pub facts: Vec<ResumeFact>,
    #[serde(default)]
    pub preferences: JobPreferences,
    pub source_file_name: String,
    pub updated_at: String,
    pub version: i64,
}

impl ResumeProfile {
    pub fn flattened_skills(&self) -> Vec<String> {
        let mut values = Vec::new();
        for item in self
            .professional_skills
            .iter()
            .flat_map(|group| group.items.iter())
        {
            let item = item.trim();
            if !item.is_empty()
                && !values
                    .iter()
                    .any(|value: &String| value.eq_ignore_ascii_case(item))
            {
                values.push(item.to_string());
            }
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_flat_skills_migrate_to_a_professional_skill_group() {
        let profile: ResumeProfile = serde_json::from_value(serde_json::json!({
            "id": "resume-legacy",
            "name": "测试用户",
            "headline": "AI 工程师",
            "email": "",
            "phone": "",
            "location": "上海",
            "website": "",
            "summary": "",
            "skills": ["Python", "SQL", "python"],
            "experiences": [],
            "education": [],
            "facts": [],
            "preferences": {},
            "sourceFileName": "legacy.json",
            "updatedAt": "2026-01-01T00:00:00+08:00",
            "version": 1
        }))
        .unwrap();

        assert_eq!(profile.template_id, "ai-engineering");
        assert_eq!(profile.professional_skills.len(), 1);
        assert_eq!(profile.professional_skills[0].label, "核心技能");
        assert_eq!(profile.flattened_skills(), vec!["Python", "SQL"]);
        assert!(profile.projects.is_empty());
        assert!(profile.certifications.is_empty());

        let serialized = serde_json::to_value(profile).unwrap();
        assert!(serialized.get("professionalSkills").is_some());
        assert!(serialized.get("skills").is_none());
    }

    #[test]
    fn legacy_provider_defaults_to_no_verified_vision_capability() {
        let provider: AiProviderConfig = serde_json::from_value(serde_json::json!({
            "id":"provider", "kind":"custom", "name":"Custom", "baseUrl":"https://example.invalid/v1",
            "model":"model", "isDefault":true, "verified":true
        })).unwrap();
        assert!(!provider.vision_verified);
    }

    #[test]
    fn resume_color_themes_use_the_frontend_wire_values() {
        assert_eq!(
            serde_json::to_value(ResumeColorTheme::Pine).unwrap(),
            "pine"
        );
        assert_eq!(
            serde_json::to_value(ResumeColorTheme::Navy).unwrap(),
            "navy"
        );
        assert_eq!(
            serde_json::to_value(ResumeColorTheme::Graphite).unwrap(),
            "graphite"
        );
    }

    #[test]
    fn legacy_privacy_boolean_does_not_acknowledge_the_beta_policy() {
        let settings: AppSettings = serde_json::from_value(serde_json::json!({
            "advancedMode": true,
            "privacyAcknowledged": true,
            "telemetry": true
        }))
        .unwrap();
        assert!(settings.advanced_mode);
        assert!(settings.automatic_update_checks);
        assert!(settings.privacy_acknowledged_version.is_none());
        assert!(settings.last_update_check_at.is_none());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConfig {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub allow_insecure_http: bool,
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
    pub is_default: bool,
    pub verified: bool,
    #[serde(default)]
    pub vision_verified: bool,
    #[serde(default)]
    pub last_tested_at: Option<String>,
    #[serde(default)]
    pub last_test_error: Option<String>,
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
pub struct ConfigurationItem {
    pub state: String,
    pub message: String,
    #[serde(default)]
    pub last_attempt_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSnapshot {
    pub boss: ConfigurationItem,
    pub llm: ConfigurationItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BossProfileState {
    pub configured: bool,
    #[serde(default)]
    pub configured_at: Option<String>,
    pub last_attempt_status: String,
    #[serde(default)]
    pub last_attempt_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Default for BossProfileState {
    fn default() -> Self {
        Self {
            configured: false,
            configured_at: None,
            last_attempt_status: "never".into(),
            last_attempt_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub advanced_mode: bool,
    #[serde(default = "default_automatic_update_checks")]
    pub automatic_update_checks: bool,
    #[serde(default)]
    pub privacy_acknowledged_version: Option<String>,
    #[serde(default)]
    pub last_update_check_at: Option<String>,
}

const fn default_automatic_update_checks() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            advanced_mode: false,
            automatic_update_checks: true,
            privacy_acknowledged_version: None,
            last_update_check_at: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeStatus {
    pub installed: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub identifier: String,
    pub os: String,
    pub arch: String,
    pub webview: String,
    pub schema_version: i64,
    pub sidecar_protocol: String,
    pub chrome: ChromeStatus,
    pub data_dir: String,
    pub legacy_data_detected: bool,
    #[serde(default)]
    pub last_update_check_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub version: String,
    pub current_version: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub download_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEvent {
    pub event: String,
    #[serde(default)]
    pub downloaded: u64,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file_name: String,
    pub path: String,
    pub size: u64,
    pub created_at: String,
    pub source_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearDataItemResult {
    pub item: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearDataResult {
    pub complete: bool,
    pub items: Vec<ClearDataItemResult>,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSnapshot {
    pub readiness: Readiness,
    pub configuration: ConfigurationSnapshot,
    pub resume: Option<ResumeProfile>,
    pub providers: Vec<AiProviderConfig>,
    pub tasks: Vec<TaskRun>,
    pub scrape_runs: Vec<ScrapeRun>,
    #[serde(default)]
    pub last_search_spec: Option<SearchSpec>,
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
    #[serde(default)]
    pub vision_supported: bool,
    #[serde(default)]
    pub vision_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSaveResult {
    pub providers: Vec<AiProviderConfig>,
    pub test_result: ProviderTestResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitAnalysisResult {
    pub job: Job,
    pub cache_hit: bool,
    pub source: String,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    pub path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteJobsResult {
    pub deleted_count: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportKeyword {
    pub key: String,
    pub label: String,
    pub job_count: i64,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDataReport {
    pub generated_at: String,
    #[serde(default)]
    pub selected_keywords: Vec<ReportKeyword>,
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
pub struct InterviewPreparationSkill {
    pub name: String,
    #[serde(default)]
    pub gap: Option<String>,
    pub action: String,
    #[serde(default)]
    pub job_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewPreparation {
    pub summary: String,
    #[serde(default)]
    pub skills: Vec<InterviewPreparationSkill>,
    #[serde(default)]
    pub project_ideas: Vec<String>,
    #[serde(default)]
    pub practice_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewPreparationState {
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub has_provider: bool,
    pub has_resume: bool,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub preparation: Option<InterviewPreparation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeFactCandidate {
    pub id: String,
    pub category: String,
    pub value: String,
    #[serde(default)]
    pub source_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeFieldEdit {
    pub id: String,
    pub path: String,
    pub label: String,
    pub operation: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
    pub rationale: String,
    #[serde(default)]
    pub evidence_fact_ids: Vec<String>,
    #[serde(default)]
    pub required_fact_candidate_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeChatProposal {
    pub proposal_id: String,
    pub resume_id: String,
    pub base_version: i64,
    #[serde(default)]
    pub job: Option<ResumeChatJob>,
    pub assistant_message: String,
    #[serde(default)]
    pub edits: Vec<ResumeFieldEdit>,
    #[serde(default)]
    pub fact_candidates: Vec<ResumeFactCandidate>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeChatJob {
    pub id: String,
    pub title: String,
    pub company: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeChatRequest {
    pub resume_id: String,
    pub expected_version: i64,
    #[serde(default)]
    pub job_id: Option<String>,
    pub messages: Vec<ResumeChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResumeEditsRequest {
    pub proposal: ResumeChatProposal,
    pub selected_edit_ids: Vec<String>,
    #[serde(default)]
    pub confirmed_fact_candidate_ids: Vec<String>,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeVersionSummary {
    pub id: String,
    pub resume_id: String,
    pub version: i64,
    #[serde(default)]
    pub parent_version: Option<i64>,
    pub created_at: String,
    pub source: String,
    pub summary: String,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub proposal_id: Option<String>,
    #[serde(default)]
    pub restored_from_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeVersionDetail {
    #[serde(flatten)]
    pub summary: ResumeVersionSummary,
    pub profile: ResumeProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeCommitResult {
    pub resume: ResumeProfile,
    pub version: ResumeVersionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarBossOutcome {
    pub login_succeeded: bool,
    pub reset_requested: bool,
    pub cleanup_succeeded: bool,
    #[serde(default)]
    pub closed_processes: i64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarJobBatch {
    #[serde(default)]
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub report_markdown: Option<String>,
}
