use super::*;

impl Database {
    pub fn active_resume(&self) -> Result<Option<ResumeProfile>, String> {
        let connection = self.connect()?;
        let json = connection
            .query_row("SELECT payload_json FROM resume_profiles WHERE is_active = 1 ORDER BY updated_at DESC LIMIT 1", [], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|error| error.to_string())?;
        json.map(|value| {
            let mut resume: ResumeProfile =
                serde_json::from_str(&value).map_err(|error| error.to_string())?;
            ensure_resume_item_ids(&mut resume);
            Ok(resume)
        })
        .transpose()
    }

    pub fn save_resume(&self, resume: &ResumeProfile) -> Result<(), String> {
        let previous_confirmed = self
            .active_resume()?
            .as_ref()
            .map(confirmed_fact_signature)
            .unwrap_or_default();
        let mut resume = resume.clone();
        ensure_resume_item_ids(&mut resume);
        validate_resume_facts(&mut resume)?;
        let confirmed_changed = previous_confirmed != confirmed_fact_signature(&resume);
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE resume_profiles SET is_active = 0", [])
            .map_err(|error| error.to_string())?;
        let payload = serde_json::to_string(&resume).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO resume_profiles(id, payload_json, updated_at, is_active) VALUES (?1, ?2, ?3, 1) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json, updated_at=excluded.updated_at, is_active=1",
                params![resume.id, payload, resume.updated_at],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,profile_json) VALUES (?1,?2,?3,NULL,?4,'legacy','保存的简历版本',?5)",
                params![uuid::Uuid::new_v4().to_string(), resume.id, resume.version, resume.updated_at, payload],
            )
            .map_err(|error| error.to_string())?;
        if confirmed_changed {
            clear_job_greetings(&transaction)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_resume(
        &self,
        mut candidate: ResumeProfile,
        expected_version: i64,
        source: &str,
        summary: &str,
        job_id: Option<String>,
        proposal_id: Option<String>,
        restored_from_version: Option<i64>,
    ) -> Result<ResumeCommitResult, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let current_json: Option<String> = transaction
            .query_row(
                "SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let current = current_json
            .map(|payload| {
                serde_json::from_str::<ResumeProfile>(&payload).map_err(|error| error.to_string())
            })
            .transpose()?;
        let previous_confirmed = current
            .as_ref()
            .map(confirmed_fact_signature)
            .unwrap_or_default();
        let parent_version = current.as_ref().map(|resume| resume.version);
        match current {
            Some(current) => {
                if current.version != expected_version {
                    return Err(format!(
                        "version_conflict: 当前简历为 v{}，请刷新后重新生成建议。",
                        current.version
                    ));
                }
                candidate.id = current.id;
                candidate.version = current.version + 1;
            }
            None => {
                if expected_version != 0 {
                    return Err("version_conflict: 当前没有可提交的主简历。".into());
                }
                if candidate.id.trim().is_empty() {
                    candidate.id = uuid::Uuid::new_v4().to_string();
                }
                candidate.version = 1;
            }
        }
        candidate.updated_at = time::shanghai_rfc3339();
        ensure_resume_item_ids(&mut candidate);
        validate_resume_facts(&mut candidate)?;
        let confirmed_changed = previous_confirmed != confirmed_fact_signature(&candidate);
        let payload = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        transaction
            .execute("UPDATE resume_profiles SET is_active=0", [])
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO resume_profiles(id,payload_json,updated_at,is_active) VALUES (?1,?2,?3,1) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json,updated_at=excluded.updated_at,is_active=1",
                params![candidate.id, payload, candidate.updated_at],
            )
            .map_err(|error| error.to_string())?;
        let version = ResumeVersionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            resume_id: candidate.id.clone(),
            version: candidate.version,
            parent_version,
            created_at: candidate.updated_at.clone(),
            source: source.into(),
            summary: summary.into(),
            job_id,
            proposal_id,
            restored_from_version,
        };
        transaction
            .execute(
                "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![version.id, version.resume_id, version.version, version.parent_version, version.created_at, version.source, version.summary, version.job_id, version.proposal_id, version.restored_from_version, payload],
            )
            .map_err(|error| error.to_string())?;
        if confirmed_changed {
            clear_job_greetings(&transaction)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(ResumeCommitResult {
            resume: candidate,
            version,
        })
    }

    pub fn list_resume_variants(&self) -> Result<Vec<ResumeVariantSummary>, String> {
        let connection = self.connect()?;
        let master_version = active_resume_version(&connection)?;
        let mut statement = connection
            .prepare(
                "SELECT v.id,v.job_id,j.title,j.company,v.name,v.base_resume_id,v.base_resume_version,v.version,v.created_at,v.updated_at
                 FROM resume_variants v JOIN jobs j ON j.id=v.job_id ORDER BY v.updated_at DESC,v.id ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let base_resume_version: i64 = row.get(6)?;
                Ok(ResumeVariantSummary {
                    id: row.get(0)?,
                    job_id: row.get(1)?,
                    job_title: row.get(2)?,
                    company: row.get(3)?,
                    name: row.get(4)?,
                    base_resume_id: row.get(5)?,
                    base_resume_version,
                    version: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    stale: master_version.is_some_and(|version| version > base_resume_version),
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn get_resume_variant(&self, id: &str) -> Result<Option<ResumeVariantDetail>, String> {
        self.resume_variant_by("v.id", id)
    }

    pub fn get_resume_variant_for_job(
        &self,
        job_id: &str,
    ) -> Result<Option<ResumeVariantDetail>, String> {
        self.resume_variant_by("v.job_id", job_id)
    }

    fn resume_variant_by(
        &self,
        column: &str,
        value: &str,
    ) -> Result<Option<ResumeVariantDetail>, String> {
        let connection = self.connect()?;
        let master_version = active_resume_version(&connection)?;
        let sql = format!(
            "SELECT v.id,v.job_id,j.title,j.company,v.name,v.base_resume_id,v.base_resume_version,v.version,v.created_at,v.updated_at,v.payload_json
             FROM resume_variants v JOIN jobs j ON j.id=v.job_id WHERE {column}=?1"
        );
        let record: Option<(ResumeVariantSummary, String)> = connection
            .query_row(&sql, [value], |row| {
                let base_resume_version: i64 = row.get(6)?;
                Ok((
                    ResumeVariantSummary {
                        id: row.get(0)?,
                        job_id: row.get(1)?,
                        job_title: row.get(2)?,
                        company: row.get(3)?,
                        name: row.get(4)?,
                        base_resume_id: row.get(5)?,
                        base_resume_version,
                        version: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                        stale: master_version.is_some_and(|version| version > base_resume_version),
                    },
                    row.get(10)?,
                ))
            })
            .optional()
            .map_err(|error| error.to_string())?;
        record
            .map(|(summary, payload)| {
                let mut profile: ResumeProfile =
                    serde_json::from_str(&payload).map_err(|error| error.to_string())?;
                ensure_resume_item_ids(&mut profile);
                Ok(ResumeVariantDetail { summary, profile })
            })
            .transpose()
    }

    pub fn create_resume_variant(
        &self,
        job_id: &str,
        expected_resume_version: i64,
    ) -> Result<ResumeVariantDetail, String> {
        // Creation is idempotent per job. Reopening an existing variant must not depend on the
        // current master resume, so the expected version only applies to the initial clone below.
        if let Some(existing) = self.get_resume_variant_for_job(job_id)? {
            return Ok(existing);
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let (job_title, company): (String, String) = transaction
            .query_row(
                "SELECT title,company FROM jobs WHERE id=?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "job_not_found: 岗位不存在。".to_string())?;
        let master_payload: String = transaction
            .query_row("SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1", [], |row| row.get(0))
            .optional().map_err(|error| error.to_string())?
            .ok_or_else(|| "resume_not_found: 请先导入主简历。".to_string())?;
        let master: ResumeProfile =
            serde_json::from_str(&master_payload).map_err(|error| error.to_string())?;
        if master.version != expected_resume_version {
            return Err(format!(
                "version_conflict: 当前主简历为 v{}，请刷新后重试。",
                master.version
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = time::shanghai_rfc3339();
        let mut profile = master.clone();
        profile.id = id.clone();
        profile.version = 1;
        profile.updated_at = now.clone();
        ensure_resume_item_ids(&mut profile);
        let payload = serde_json::to_string(&profile).map_err(|error| error.to_string())?;
        let name = format!("{company} · {job_title}");
        transaction.execute(
            "INSERT INTO resume_variants(id,job_id,base_resume_id,base_resume_version,version,name,created_at,updated_at,payload_json) VALUES (?1,?2,?3,?4,1,?5,?6,?6,?7)",
            params![id, job_id, master.id, master.version, name, now, payload],
        ).map_err(|error| error.to_string())?;
        transaction.execute(
            "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,profile_json) VALUES (?1,?2,1,NULL,?3,'variant-create','创建岗位版本',?4,?5)",
            params![uuid::Uuid::new_v4().to_string(), id, now, job_id, payload],
        ).map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        self.get_resume_variant(&id)?
            .ok_or_else(|| "storage_error: 岗位版本创建后无法读取。".into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_resume_variant(
        &self,
        variant_id: &str,
        mut candidate: ResumeProfile,
        expected_version: i64,
        source: &str,
        summary: &str,
        restored_from_version: Option<i64>,
        new_base_resume_version: Option<i64>,
    ) -> Result<ResumeVariantCommitResult, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let current_record: Option<(String, i64, String, i64)> = transaction.query_row(
            "SELECT payload_json,version,job_id,base_resume_version FROM resume_variants WHERE id=?1",
            [variant_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional().map_err(|error| error.to_string())?;
        let (current_payload, current_version, job_id, base_resume_version) =
            current_record.ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
        if current_version != expected_version {
            return Err(format!(
                "version_conflict: 当前岗位版本为 v{current_version}，请刷新后重试。"
            ));
        }
        let current: ResumeProfile =
            serde_json::from_str(&current_payload).map_err(|error| error.to_string())?;
        candidate.id = variant_id.to_string();
        candidate.version = current_version + 1;
        candidate.updated_at = time::shanghai_rfc3339();
        if new_base_resume_version.is_none() {
            candidate.facts = current.facts;
            candidate.preferences = current.preferences;
        }
        ensure_resume_item_ids(&mut candidate);
        validate_resume_facts(&mut candidate)?;
        let payload = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        let base_version = new_base_resume_version.unwrap_or(base_resume_version);
        let changed = transaction.execute(
            "UPDATE resume_variants SET payload_json=?1,version=?2,base_resume_version=?3,updated_at=?4 WHERE id=?5 AND version=?6",
            params![payload, candidate.version, base_version, candidate.updated_at, variant_id, expected_version],
        ).map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("version_conflict: 岗位版本已变化，请刷新后重试。".into());
        }
        let version = ResumeVersionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            resume_id: variant_id.to_string(),
            version: candidate.version,
            parent_version: Some(current_version),
            created_at: candidate.updated_at.clone(),
            source: source.into(),
            summary: summary.into(),
            job_id: Some(job_id.clone()),
            proposal_id: None,
            restored_from_version,
        };
        transaction.execute(
            "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10)",
            params![version.id, version.resume_id, version.version, version.parent_version, version.created_at, version.source, version.summary, job_id, version.restored_from_version, payload],
        ).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM resume_coverage_cache WHERE target_kind='variant' AND target_id=?1",
                [variant_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        let variant = self
            .get_resume_variant(variant_id)?
            .ok_or_else(|| "storage_error: 岗位版本保存后无法读取。".to_string())?;
        Ok(ResumeVariantCommitResult { variant, version })
    }

    pub fn delete_resume_variant(&self, variant_id: &str) -> Result<i64, String> {
        let connection = self.connect()?;
        let changed = connection
            .execute("DELETE FROM resume_variants WHERE id=?1", [variant_id])
            .map_err(|error| error.to_string())?;
        Ok(changed as i64)
    }

    pub fn restore_resume_variant_version(
        &self,
        variant_id: &str,
        version_id: &str,
        expected_version: i64,
    ) -> Result<ResumeVariantCommitResult, String> {
        let detail = self
            .get_resume_version(version_id)?
            .ok_or_else(|| "简历版本不存在。".to_string())?;
        if detail.summary.resume_id != variant_id {
            return Err("不能把其他简历的历史恢复到当前岗位版本。".into());
        }
        self.commit_resume_variant(
            variant_id,
            detail.profile,
            expected_version,
            "variant-rollback",
            &format!("恢复到 v{} 的内容", detail.summary.version),
            Some(detail.summary.version),
            None,
        )
    }

    pub fn preview_resume_variant_rebase(
        &self,
        variant_id: &str,
    ) -> Result<ResumeRebasePreview, String> {
        let variant = self
            .get_resume_variant(variant_id)?
            .ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
        let master = self
            .active_resume()?
            .ok_or_else(|| "resume_not_found: 当前没有主简历。".to_string())?;
        if master.id != variant.summary.base_resume_id {
            return Err(
                "base_resume_changed: 当前主简历与岗位版本基线不一致，请重新创建岗位版本。".into(),
            );
        }
        let base = self
            .resume_version_profile(
                &variant.summary.base_resume_id,
                variant.summary.base_resume_version,
            )?
            .ok_or_else(|| "base_version_missing: 岗位版本的主简历基线已不存在。".to_string())?;
        let (auto_changes, conflicts) = compute_rebase_changes(&base, &master, &variant.profile)?;
        Ok(ResumeRebasePreview {
            variant_id: variant.summary.id,
            variant_version: variant.summary.version,
            base_resume_version: variant.summary.base_resume_version,
            master_version: master.version,
            auto_changes,
            conflicts,
        })
    }

    pub fn apply_resume_variant_rebase(
        &self,
        variant_id: &str,
        expected_variant_version: i64,
        expected_master_version: i64,
        resolutions: &[ResumeRebaseResolution],
    ) -> Result<ResumeVariantCommitResult, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| error.to_string())?;

        let variant_record: Option<(String, i64, String, String, i64)> = transaction
            .query_row(
                "SELECT payload_json,version,job_id,base_resume_id,base_resume_version FROM resume_variants WHERE id=?1",
                [variant_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let (variant_payload, variant_version, job_id, base_resume_id, base_resume_version) =
            variant_record.ok_or_else(|| "variant_not_found: 岗位版本不存在。".to_string())?;
        if variant_version != expected_variant_version {
            return Err(format!(
                "version_conflict: 当前岗位版本为 v{variant_version}，请刷新后重试。"
            ));
        }
        let variant: ResumeProfile =
            serde_json::from_str(&variant_payload).map_err(|error| error.to_string())?;

        let master_payload: String = transaction
            .query_row(
                "SELECT payload_json FROM resume_profiles WHERE is_active=1 ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "resume_not_found: 当前没有主简历。".to_string())?;
        let master: ResumeProfile =
            serde_json::from_str(&master_payload).map_err(|error| error.to_string())?;
        if master.version != expected_master_version {
            return Err(format!(
                "version_conflict: 当前主简历为 v{}，请重新检查同步差异。",
                master.version
            ));
        }
        if master.id != base_resume_id {
            return Err(
                "base_resume_changed: 当前主简历与岗位版本基线不一致，请重新创建岗位版本。".into(),
            );
        }

        let base_payload: String = transaction
            .query_row(
                "SELECT profile_json FROM resume_versions WHERE resume_id=?1 AND version=?2",
                params![base_resume_id, base_resume_version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "base_version_missing: 岗位版本的主简历基线已不存在。".to_string())?;
        let base: ResumeProfile =
            serde_json::from_str(&base_payload).map_err(|error| error.to_string())?;
        let (auto_changes, conflicts) = compute_rebase_changes(&base, &master, &variant)?;

        let mut value = serde_json::to_value(&variant).map_err(|error| error.to_string())?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "storage_error: 岗位版本结构无效。".to_string())?;
        let resolution_map = resolutions
            .iter()
            .map(|item| (item.path.as_str(), item.choice.as_str()))
            .collect::<std::collections::HashMap<_, _>>();
        for change in &auto_changes {
            object.insert(
                change.path.trim_start_matches('/').into(),
                change.master.clone(),
            );
        }
        for conflict in &conflicts {
            match resolution_map.get(conflict.path.as_str()).copied() {
                Some("master") => {
                    object.insert(
                        conflict.path.trim_start_matches('/').into(),
                        conflict.master.clone(),
                    );
                }
                Some("variant") => {}
                _ => {
                    return Err(format!(
                        "invalid_request: 请处理字段“{}”的同步冲突。",
                        conflict.label
                    ))
                }
            }
        }
        object.insert(
            "facts".into(),
            serde_json::to_value(&master.facts).map_err(|error| error.to_string())?,
        );
        object.insert(
            "preferences".into(),
            serde_json::to_value(&master.preferences).map_err(|error| error.to_string())?,
        );
        let mut candidate: ResumeProfile =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        candidate.id = variant_id.to_string();
        candidate.version = variant_version + 1;
        candidate.updated_at = time::shanghai_rfc3339();
        ensure_resume_item_ids(&mut candidate);
        validate_resume_facts(&mut candidate)?;
        let payload = serde_json::to_string(&candidate).map_err(|error| error.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE resume_variants SET payload_json=?1,version=?2,base_resume_version=?3,updated_at=?4 WHERE id=?5 AND version=?6",
                params![
                    payload,
                    candidate.version,
                    master.version,
                    candidate.updated_at,
                    variant_id,
                    expected_variant_version
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err("version_conflict: 岗位版本已变化，请刷新后重试。".into());
        }
        let version = ResumeVersionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            resume_id: variant_id.to_string(),
            version: candidate.version,
            parent_version: Some(variant_version),
            created_at: candidate.updated_at.clone(),
            source: "variant-rebase".into(),
            summary: format!("同步主简历 v{}", master.version),
            job_id: Some(job_id.clone()),
            proposal_id: None,
            restored_from_version: None,
        };
        transaction
            .execute(
                "INSERT INTO resume_versions(id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,NULL,?9)",
                params![
                    version.id,
                    version.resume_id,
                    version.version,
                    version.parent_version,
                    version.created_at,
                    version.source,
                    version.summary,
                    job_id,
                    payload
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM resume_coverage_cache WHERE target_kind='variant' AND target_id=?1",
                [variant_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;

        let variant = self
            .get_resume_variant(variant_id)?
            .ok_or_else(|| "storage_error: 岗位版本同步后无法读取。".to_string())?;
        Ok(ResumeVariantCommitResult { variant, version })
    }

    fn resume_version_profile(
        &self,
        resume_id: &str,
        version: i64,
    ) -> Result<Option<ResumeProfile>, String> {
        let connection = self.connect()?;
        let payload: Option<String> = connection
            .query_row(
                "SELECT profile_json FROM resume_versions WHERE resume_id=?1 AND version=?2",
                params![resume_id, version],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn resume_coverage_cache(
        &self,
        cache_key: &str,
    ) -> Result<Option<ResumeCoverageReport>, String> {
        let connection = self.connect()?;
        let payload: Option<String> = connection
            .query_row(
                "SELECT payload_json FROM resume_coverage_cache WHERE cache_key=?1",
                [cache_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn save_resume_coverage_cache(
        &self,
        cache_key: &str,
        job_fingerprint: &str,
        provider_fingerprint: &str,
        skill_version: &str,
        report: &ResumeCoverageReport,
    ) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(report).map_err(|error| error.to_string())?;
        connection.execute(
            "INSERT INTO resume_coverage_cache(cache_key,target_kind,target_id,target_version,job_id,job_fingerprint,provider_fingerprint,skill_version,generated_at,payload_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(cache_key) DO UPDATE SET generated_at=excluded.generated_at,payload_json=excluded.payload_json",
            params![cache_key, report.target.kind, report.target.id, report.target_version, report.job_id, job_fingerprint, provider_fingerprint, skill_version, report.generated_at, payload],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_resume_versions(
        &self,
        resume_id: &str,
    ) -> Result<Vec<ResumeVersionSummary>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version FROM resume_versions WHERE resume_id=?1 ORDER BY version DESC")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([resume_id], resume_version_from_row)
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn get_resume_version(&self, id: &str) -> Result<Option<ResumeVersionDetail>, String> {
        let connection = self.connect()?;
        let record: Option<(ResumeVersionSummary, String)> = connection
            .query_row(
                "SELECT id,resume_id,version,parent_version,created_at,source,summary,job_id,proposal_id,restored_from_version,profile_json FROM resume_versions WHERE id=?1",
                [id],
                |row| Ok((resume_version_from_row(row)?, row.get(10)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        record
            .map(|(summary, payload)| {
                let mut profile: ResumeProfile =
                    serde_json::from_str(&payload).map_err(|error| error.to_string())?;
                ensure_resume_item_ids(&mut profile);
                Ok(ResumeVersionDetail { summary, profile })
            })
            .transpose()
    }
}
