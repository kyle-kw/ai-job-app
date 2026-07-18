use super::*;

fn list_job_cities_with_connection(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT DISTINCT city FROM jobs WHERE city<>'' ORDER BY city COLLATE NOCASE")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

impl Database {
    pub fn list_jobs(&self) -> Result<Vec<Job>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT payload_json FROM jobs ORDER BY last_seen DESC, title ASC")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        let mut jobs = rows
            .map(|row| {
                let json = row.map_err(|error| error.to_string())?;
                serde_json::from_str(&json).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        mark_latest_new_jobs(&connection, &mut jobs)?;
        Ok(jobs)
    }

    pub fn list_jobs_page(&self, query: &JobQuery) -> Result<JobPage, String> {
        let sort = normalize_job_sort(&query.sort);
        let connection = self.connect()?;
        let (where_without_cursor, count_values) = job_query_where(query, false)?;
        let total = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM jobs WHERE {where_without_cursor}"),
                params_from_iter(count_values.iter()),
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let pending_detail_count = connection
            .query_row(
                "SELECT COUNT(*) FROM jobs WHERE has_description=1 AND has_structured_details=0",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let (where_clause, values) = job_query_where(query, true)?;
        let sql = format!(
            "SELECT payload_json,COALESCE(fit_score,0),last_seen,{JOB_SALARY_SORT_SQL},id FROM jobs WHERE {where_clause} ORDER BY {} LIMIT {}",
            job_order_by(sort),
            JOB_PAGE_SIZE + 1
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut records = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let has_more = records.len() > JOB_PAGE_SIZE;
        records.truncate(JOB_PAGE_SIZE);
        let next_cursor = if has_more {
            records
                .last()
                .map(|(_, score, last_seen, salary_mid, id)| {
                    encode_job_cursor(&JobCursor {
                        sort: sort.to_string(),
                        score: *score,
                        last_seen: last_seen.clone(),
                        salary_mid: *salary_mid,
                        id: id.clone(),
                    })
                })
                .transpose()?
        } else {
            None
        };
        let mut items = records
            .into_iter()
            .map(|(payload, _, _, _, _)| {
                serde_json::from_str(&payload).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        mark_latest_new_jobs(&connection, &mut items)?;
        Ok(JobPage {
            items,
            total,
            pending_detail_count,
            next_cursor,
        })
    }

    pub fn list_job_options(&self, query: &str) -> Result<Vec<JobOption>, String> {
        let connection = self.connect()?;
        let normalized = query.trim().to_lowercase();
        let pattern = escaped_like_pattern(&normalized);
        let mut statement = connection
            .prepare(
                "SELECT id,title,company,last_seen FROM jobs WHERE ?1='' OR search_text LIKE ?2 ESCAPE '\\' ORDER BY last_seen DESC,id ASC LIMIT 50",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![normalized, pattern], |row| {
                Ok(JobOption {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    company: row.get(2)?,
                    last_seen: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn list_job_cities(&self) -> Result<Vec<String>, String> {
        let connection = self.connect()?;
        list_job_cities_with_connection(&connection)
    }

    pub fn list_job_filter_options(&self) -> Result<JobFilterOptions, String> {
        let connection = self.connect()?;
        let cities = list_job_cities_with_connection(&connection)?;
        let experiences = {
            let mut statement = connection
                .prepare(
                    "SELECT DISTINCT trim(json_extract(payload_json,'$.experience')) AS experience \
                     FROM jobs WHERE trim(COALESCE(json_extract(payload_json,'$.experience'),''))<>'' \
                     ORDER BY experience COLLATE NOCASE",
                )
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            rows
        };
        let skills = {
            let mut statement = connection
                .prepare(
                    "SELECT MIN(trim(skill.value)) AS label, COUNT(DISTINCT jobs.id) AS job_count \
                     FROM jobs JOIN json_each(jobs.payload_json,'$.skills') AS skill \
                     WHERE trim(COALESCE(skill.value,''))<>'' \
                     GROUP BY lower(trim(skill.value)) \
                     ORDER BY job_count DESC, label COLLATE NOCASE",
                )
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok(JobFilterSkillOption {
                        label: row.get(0)?,
                        count: row.get(1)?,
                    })
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            rows
        };
        Ok(JobFilterOptions {
            cities,
            experiences,
            skills,
        })
    }

    pub fn job_ids_for_query(&self, query: &JobQuery) -> Result<Vec<String>, String> {
        let mut query = query.clone();
        query.cursor = None;
        let (where_clause, values) = job_query_where(&query, false)?;
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT id FROM jobs WHERE {where_clause} ORDER BY {}",
                job_order_by(normalize_job_sort(&query.sort))
            ))
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn jobs_for_query(&self, query: &JobQuery) -> Result<Vec<Job>, String> {
        let mut query = query.clone();
        query.cursor = None;
        let (where_clause, values) = job_query_where(&query, false)?;
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT payload_json FROM jobs WHERE {where_clause} ORDER BY {}",
                job_order_by(normalize_job_sort(&query.sort))
            ))
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        let mut jobs = rows
            .map(|row| {
                let json = row.map_err(|error| error.to_string())?;
                serde_json::from_str(&json).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        mark_latest_new_jobs(&connection, &mut jobs)?;
        Ok(jobs)
    }

    pub fn pending_detail_jobs(&self) -> Result<Vec<Job>, String> {
        self.list_json(
            "SELECT payload_json FROM jobs WHERE has_description=1 AND has_structured_details=0 ORDER BY last_seen DESC",
        )
    }

    pub fn delete_job(&self, job_id: &str) -> Result<i64, String> {
        let connection = self.connect()?;
        let changed = connection
            .execute("DELETE FROM jobs WHERE id=?1", [job_id])
            .map_err(|error| error.to_string())?;
        Ok(changed as i64)
    }

    pub fn delete_missing_description_jobs(&self, query: &JobQuery) -> Result<i64, String> {
        let mut query = query.clone();
        query.cursor = None;
        query.missing_description = true;
        let (where_clause, values) = job_query_where(&query, false)?;
        let connection = self.connect()?;
        let changed = connection
            .execute(
                &format!("DELETE FROM jobs WHERE {where_clause}"),
                params_from_iter(values.iter()),
            )
            .map_err(|error| error.to_string())?;
        Ok(changed as i64)
    }

    pub fn list_report_keywords(&self) -> Result<Vec<ReportKeyword>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                r#"SELECT keyword_key, MAX(keyword_label), COUNT(DISTINCT job_id), MAX(last_seen)
                   FROM job_keywords
                   GROUP BY keyword_key
                   ORDER BY MAX(last_seen) DESC, MAX(keyword_label) COLLATE NOCASE ASC"#,
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(ReportKeyword {
                    key: row.get(0)?,
                    label: row.get(1)?,
                    job_count: row.get(2)?,
                    last_seen: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn report_keywords_for_keys(
        &self,
        keyword_keys: &[String],
    ) -> Result<Vec<ReportKeyword>, String> {
        let requested = normalize_keyword_keys(keyword_keys);
        let requested = requested.into_iter().collect::<HashSet<_>>();
        Ok(self
            .list_report_keywords()?
            .into_iter()
            .filter(|keyword| requested.contains(&keyword.key))
            .collect())
    }

    pub fn list_jobs_by_keyword_keys(&self, keyword_keys: &[String]) -> Result<Vec<Job>, String> {
        let keyword_keys = normalize_keyword_keys(keyword_keys);
        if keyword_keys.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = (1..=keyword_keys.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            r#"SELECT DISTINCT jobs.payload_json
               FROM jobs
               INNER JOIN job_keywords ON job_keywords.job_id = jobs.id
               WHERE job_keywords.keyword_key IN ({placeholders})
               ORDER BY jobs.last_seen DESC, jobs.title ASC"#
        );
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(&query)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(keyword_keys.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            let payload = row.map_err(|error| error.to_string())?;
            serde_json::from_str(&payload).map_err(|error| error.to_string())
        })
        .collect()
    }

    pub fn completed_detail_external_ids(&self, source: &str) -> Result<Vec<String>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT DISTINCT external_key FROM jobs WHERE lower(source)=lower(?1) AND has_description=1 AND external_key NOT LIKE 'fp:%' ORDER BY external_key")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([source], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn get_job(&self, id: &str) -> Result<Option<Job>, String> {
        let connection = self.connect()?;
        let json = connection
            .query_row("SELECT payload_json FROM jobs WHERE id = ?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| error.to_string())?;
        let mut job = json
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()?;
        if let Some(job) = job.as_mut() {
            mark_latest_new_jobs(&connection, std::slice::from_mut(job))?;
        }
        Ok(job)
    }

    pub fn upsert_jobs(&self, jobs: Vec<Job>) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(jobs, false, UpsertMode::Generic, None, None)
    }

    fn upsert_jobs_internal(
        &self,
        jobs: Vec<Job>,
        preserve_is_new_on_update: bool,
        mode: UpsertMode,
        keyword: Option<&str>,
        scrape_run_id: Option<&str>,
    ) -> Result<UpsertStats, String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let mut stats = UpsertStats::default();
        for job in jobs {
            let item = upsert_job_in_transaction(
                &transaction,
                job,
                preserve_is_new_on_update,
                mode,
                keyword,
                scrape_run_id,
            )?;
            stats.inserted += item.inserted;
            stats.updated += item.updated;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(stats)
    }

    pub fn upsert_job(&self, job: Job) -> Result<UpsertStats, String> {
        self.upsert_jobs(vec![job])
    }

    pub fn update_streamed_job(&self, job: Job) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(vec![job], true, UpsertMode::Generic, None, None)
    }

    pub fn upsert_scrape_list_job(&self, job: Job, keyword: &str) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(
            vec![job],
            false,
            UpsertMode::ScrapeList,
            Some(keyword),
            None,
        )
    }

    pub fn upsert_scrape_list_job_for_run(
        &self,
        job: Job,
        keyword: &str,
        scrape_run_id: &str,
    ) -> Result<UpsertStats, String> {
        self.upsert_jobs_internal(
            vec![job],
            false,
            UpsertMode::ScrapeList,
            Some(keyword),
            Some(scrape_run_id),
        )
    }

    pub fn upsert_scrape_detail_job(&self, job: Job, keyword: &str) -> Result<UpsertStats, String> {
        if job.description.trim().is_empty() {
            return Err("岗位详情为空，未写入数据库。".into());
        }
        self.upsert_jobs_internal(
            vec![job],
            true,
            UpsertMode::ScrapeDetail,
            Some(keyword),
            None,
        )
    }

    pub fn upsert_scrape_detail_job_for_run(
        &self,
        job: Job,
        keyword: &str,
        scrape_run_id: &str,
    ) -> Result<UpsertStats, String> {
        if job.description.trim().is_empty() {
            return Err("岗位详情为空，未写入数据库。".into());
        }
        self.upsert_jobs_internal(
            vec![job],
            true,
            UpsertMode::ScrapeDetail,
            Some(keyword),
            Some(scrape_run_id),
        )
    }

    pub fn save_job(&self, job: &Job) -> Result<(), String> {
        let connection = self.connect()?;
        let payload = serde_json::to_string(job).map_err(|error| error.to_string())?;
        let meta = JobQueryMetadata::from_job(job);
        let changed = connection
            .execute(
                "UPDATE jobs SET payload_json=?1,last_seen=?2,search_text=?3,salary_min=?4,salary_max=?5,company_scale_code=?6,city=?7,query_is_new=?8,fit_score=?9,has_description=?10,has_structured_details=?11 WHERE id=?12",
                params![payload, job.last_seen, meta.search_text, meta.salary_min, meta.salary_max, meta.company_scale_code, meta.city, meta.is_new, meta.fit_score, meta.has_description, meta.has_structured_details, job.id],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err(format!(
                "Job {} does not exist; changes were not saved.",
                job.id
            ));
        }
        Ok(())
    }
}
