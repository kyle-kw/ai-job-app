use super::*;

impl Database {
    pub fn interview_preparation_by_key(
        &self,
        cache_key: &str,
    ) -> Result<Option<InterviewPreparationCacheRecord>, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json FROM interview_preparation_cache WHERE cache_key=?1",
                [cache_key],
                interview_cache_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn latest_interview_preparation(
        &self,
        scope_key: &str,
    ) -> Result<Option<InterviewPreparationCacheRecord>, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json FROM interview_preparation_cache WHERE scope_key=?1 ORDER BY generated_at DESC LIMIT 1",
                [scope_key],
                interview_cache_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn save_interview_preparation(
        &self,
        record: &InterviewPreparationCacheRecord,
    ) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let payload =
            serde_json::to_string(&record.preparation).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO interview_preparation_cache(cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(cache_key) DO UPDATE SET scope_key=excluded.scope_key,dataset_hash=excluded.dataset_hash,resume_id=excluded.resume_id,resume_version=excluded.resume_version,provider_fingerprint=excluded.provider_fingerprint,skill_version=excluded.skill_version,generated_at=excluded.generated_at,payload_json=excluded.payload_json",
                params![record.cache_key, record.scope_key, record.dataset_hash, record.resume_id, record.resume_version, record.provider_fingerprint, record.skill_version, record.generated_at, payload],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM interview_preparation_cache WHERE cache_key NOT IN (SELECT cache_key FROM interview_preparation_cache ORDER BY generated_at DESC LIMIT 10)",
                [],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn report_competitiveness_by_key(
        &self,
        cache_key: &str,
    ) -> Result<Option<ReportCompetitivenessCacheRecord>, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json FROM report_competitiveness_cache WHERE cache_key=?1",
                [cache_key],
                report_competitiveness_cache_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn latest_report_competitiveness(
        &self,
        scope_key: &str,
    ) -> Result<Option<ReportCompetitivenessCacheRecord>, String> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json FROM report_competitiveness_cache WHERE scope_key=?1 ORDER BY generated_at DESC LIMIT 1",
                [scope_key],
                report_competitiveness_cache_from_row,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn save_report_competitiveness(
        &self,
        record: &ReportCompetitivenessCacheRecord,
    ) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let payload = serde_json::to_string(&record.analysis).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO report_competitiveness_cache(cache_key,scope_key,dataset_hash,resume_id,resume_version,provider_fingerprint,skill_version,generated_at,payload_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(cache_key) DO UPDATE SET scope_key=excluded.scope_key,dataset_hash=excluded.dataset_hash,resume_id=excluded.resume_id,resume_version=excluded.resume_version,provider_fingerprint=excluded.provider_fingerprint,skill_version=excluded.skill_version,generated_at=excluded.generated_at,payload_json=excluded.payload_json",
                params![record.cache_key, record.scope_key, record.dataset_hash, record.resume_id, record.resume_version, record.provider_fingerprint, record.skill_version, record.generated_at, payload],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM report_competitiveness_cache WHERE cache_key NOT IN (SELECT cache_key FROM report_competitiveness_cache ORDER BY generated_at DESC LIMIT 10)",
                [],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }
}
