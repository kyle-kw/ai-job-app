use crate::models::{FitDimension, FitReport, HardConstraint, Job, ResumeProfile};
use crate::time;
use std::collections::HashSet;

pub fn deterministic_fit(job: &Job, resume: &ResumeProfile) -> FitReport {
    let resume_skills: HashSet<String> = resume
        .flattened_skills()
        .iter()
        .map(|skill| skill.to_lowercase())
        .collect();
    let matched: Vec<String> = job
        .skills
        .iter()
        .filter(|skill| resume_skills.contains(&skill.to_lowercase()))
        .cloned()
        .collect();
    let technical = if job.skills.is_empty() {
        60
    } else {
        (((matched.len() as f64 / job.skills.len() as f64) * 70.0) + 25.0).round() as i64
    }
    .min(100);
    let resume_text = resume
        .experiences
        .iter()
        .flat_map(|experience| experience.highlights.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let title_root = job
        .title
        .split(['（', '('])
        .next()
        .unwrap_or(&job.title)
        .to_lowercase();
    let related = resume_text.contains(&title_root)
        || matched.len() >= 2
        || resume
            .experiences
            .iter()
            .any(|item| item.position.to_lowercase().contains("ai"));
    let experience_score = if related {
        82
    } else {
        (48 + matched.len() as i64 * 6).min(76)
    };
    let career_score = if resume.preferences.target_roles.is_empty() {
        None
    } else if resume.preferences.target_roles.iter().any(|role| {
        job.title.to_lowercase().contains(&role.to_lowercase())
            || role
                .to_lowercase()
                .split_whitespace()
                .any(|word| job.title.to_lowercase().contains(word))
    }) {
        Some(88)
    } else {
        Some(62)
    };
    let behavior_score = if resume.preferences.energizing_tasks.is_empty() {
        None
    } else {
        Some(72)
    };
    let dimensions = vec![
        FitDimension {
            key: "technical".into(),
            label: "技能匹配".into(),
            score: Some(technical),
            weight: 30,
            note: if matched.is_empty() {
                "暂未发现直接技能命中".into()
            } else {
                format!("命中 {} 项核心技能", matched.len())
            },
            evidence: matched.clone(),
        },
        FitDimension {
            key: "experience".into(),
            label: "经验匹配".into(),
            score: Some(experience_score),
            weight: 25,
            note: if related {
                "存在直接或高度可迁移的岗位经历".into()
            } else {
                "需要在材料中建立经验关联".into()
            },
            evidence: resume
                .experiences
                .iter()
                .take(2)
                .map(|item| format!("{} · {}", item.company, item.position))
                .collect(),
        },
        FitDimension {
            key: "behavior".into(),
            label: "行为与文化".into(),
            score: behavior_score,
            weight: 15,
            note: if behavior_score.is_some() {
                "根据偏好与岗位任务推断".into()
            } else {
                "完善偏好后可评估".into()
            },
            evidence: resume.preferences.energizing_tasks.clone(),
        },
        FitDimension {
            key: "career".into(),
            label: "职业方向".into(),
            score: career_score,
            weight: 30,
            note: if career_score.is_some() {
                "根据目标角色判断".into()
            } else {
                "尚未设置目标岗位".into()
            },
            evidence: resume.preferences.target_roles.clone(),
        },
    ];
    let known_weight: i64 = dimensions
        .iter()
        .filter(|dimension| dimension.score.is_some())
        .map(|dimension| dimension.weight)
        .sum();
    let weighted: i64 = dimensions
        .iter()
        .filter_map(|dimension| dimension.score.map(|score| score * dimension.weight))
        .sum();
    let normalized = if known_weight == 0 {
        0
    } else {
        (weighted as f64 / known_weight as f64).round() as i64
    };
    let city_known = !resume.preferences.cities.is_empty();
    let city_matched = !city_known
        || resume
            .preferences
            .cities
            .iter()
            .any(|city| job.location.contains(city));
    let score = if city_known && !city_matched {
        normalized.min(44)
    } else {
        normalized
    };
    FitReport {
        overall_score: score,
        confidence: known_weight,
        verdict: verdict(score).into(),
        recommendation: if score >= 60 {
            "建议申请，并围绕命中技能定制简历。".into()
        } else {
            "建议先核对关键缺口，再决定是否投入申请。".into()
        },
        summary: if matched.is_empty() {
            "当前更依赖可迁移经验，需要用项目成果证明匹配度。".into()
        } else {
            format!(
                "你的 {} 与岗位要求直接对应。",
                matched
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("、")
            )
        },
        dimensions,
        hard_constraints: vec![HardConstraint {
            label: "工作地点".into(),
            status: if !city_known {
                "unknown"
            } else if city_matched {
                "pass"
            } else {
                "fail"
            }
            .into(),
            note: if !city_known {
                "尚未设置目标城市"
            } else if city_matched {
                "符合地点偏好"
            } else {
                "不在目标城市范围"
            }
            .into(),
        }],
        strengths: matched
            .iter()
            .take(4)
            .map(|skill| format!("{skill} 与 JD 明确匹配"))
            .collect(),
        gaps: job
            .skills
            .iter()
            .filter(|skill| !resume_skills.contains(&skill.to_lowercase()))
            .take(4)
            .cloned()
            .collect(),
        evidence: resume
            .experiences
            .iter()
            .flat_map(|item| item.highlights.iter().take(1))
            .take(3)
            .cloned()
            .collect(),
        generated_at: time::shanghai_rfc3339(),
        skill_version: "job-fit@1.1.0".into(),
        input_hash: String::new(),
        analysis_source: "local".into(),
        fallback_reason: None,
        cache_status: "fresh".into(),
    }
}

pub fn fallback_greeting(job: &Job, resume: &ResumeProfile) -> String {
    let confirmed_skills = resume
        .facts
        .iter()
        .filter(|fact| fact.confirmed && fact.category == "skill")
        .map(|fact| fact.value.to_lowercase())
        .collect::<Vec<_>>();
    let strengths = job
        .skills
        .iter()
        .filter(|skill| {
            confirmed_skills
                .iter()
                .any(|candidate| candidate.contains(&skill.to_lowercase()))
        })
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    let mut message = if strengths.is_empty() {
        format!(
            "您好，我关注贵司{}岗位，想进一步了解，方便沟通一下吗？",
            job.title
        )
    } else {
        format!(
            "您好，我熟悉{}，和贵司{}较匹配，方便沟通一下吗？",
            strengths.join(" 与 "),
            job.title
        )
    };
    if message.chars().count() > 60 {
        let title = job.title.chars().take(10).collect::<String>();
        message = format!("您好，我关注贵司{title}岗位，方便沟通一下吗？");
    }
    message.chars().take(60).collect()
}

fn verdict(score: i64) -> &'static str {
    match score {
        75.. => "strong",
        60..=74 => "good",
        45..=59 => "moderate",
        30..=44 => "weak",
        _ => "poor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_stays_within_contract() {
        let job = Job {
            id: "1".into(),
            source: "boss".into(),
            external_id: "1".into(),
            title: "非常非常长的人工智能大模型多智能体应用研发高级工程师".into(),
            company: "公司".into(),
            salary: String::new(),
            location: String::new(),
            experience: String::new(),
            degree: String::new(),
            company_scale: String::new(),
            company_stage: String::new(),
            industry: String::new(),
            skills: vec!["Python".into()],
            welfare: vec![],
            description: String::new(),
            source_url: String::new(),
            boss_name: None,
            boss_title: None,
            first_seen: String::new(),
            last_seen: String::new(),
            is_new: true,
            fit: None,
            greeting: None,
            patches: vec![],
            structured_details: None,
        };
        let resume = ResumeProfile {
            id: "r".into(),
            name: String::new(),
            headline: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            website: String::new(),
            summary: String::new(),
            template_id: "ai-engineering".into(),
            professional_skills: vec![crate::models::ProfessionalSkillGroup {
                id: "skills".into(),
                label: "核心技能".into(),
                items: vec!["Python".into()],
            }],
            experiences: vec![],
            education: vec![],
            projects: vec![],
            certifications: vec![],
            facts: vec![],
            preferences: Default::default(),
            source_file_name: String::new(),
            updated_at: String::new(),
            version: 1,
        };
        assert!(fallback_greeting(&job, &resume).chars().count() <= 60);
        assert!(!fallback_greeting(&job, &resume).contains("AI 应用工程落地"));
        assert!(!fallback_greeting(&job, &resume).contains("经验"));
    }

    #[test]
    fn greeting_uses_only_confirmed_matching_skill_facts() {
        let mut job = Job {
            id: "1".into(),
            source: "boss".into(),
            external_id: "1".into(),
            title: "财务会计".into(),
            company: "示例公司".into(),
            salary: String::new(),
            location: String::new(),
            experience: String::new(),
            degree: String::new(),
            company_scale: String::new(),
            company_stage: String::new(),
            industry: String::new(),
            skills: vec!["Excel".into()],
            welfare: vec![],
            description: String::new(),
            source_url: String::new(),
            boss_name: None,
            boss_title: None,
            first_seen: String::new(),
            last_seen: String::new(),
            is_new: true,
            fit: None,
            greeting: None,
            patches: vec![],
            structured_details: None,
        };
        let mut resume = ResumeProfile {
            id: "r".into(),
            name: String::new(),
            headline: String::new(),
            email: String::new(),
            phone: String::new(),
            location: String::new(),
            website: String::new(),
            summary: String::new(),
            template_id: "finance-accounting".into(),
            professional_skills: vec![],
            experiences: vec![],
            education: vec![],
            projects: vec![],
            certifications: vec![],
            facts: vec![crate::models::ResumeFact {
                id: "f".into(),
                category: "skill".into(),
                value: "Excel".into(),
                source: "手工".into(),
                confidence: 1.0,
                confirmed: false,
            }],
            preferences: Default::default(),
            source_file_name: String::new(),
            updated_at: String::new(),
            version: 1,
        };
        assert!(!fallback_greeting(&job, &resume).contains("熟悉Excel"));
        resume.facts[0].confirmed = true;
        assert!(fallback_greeting(&job, &resume).contains("熟悉Excel"));
        job.skills = vec!["SQL".into()];
        assert!(!fallback_greeting(&job, &resume).contains("熟悉Excel"));
    }
}
