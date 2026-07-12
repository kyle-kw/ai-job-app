---
name: interview-preparation
description: Generate a focused interview preparation plan from aggregated local job-market statistics and, when available, confirmed resume facts. Use when a candidate requests interview skill priorities, personal capability gaps, preparation actions, project stories, or practice questions for the jobs stored in this app.
---

# Interview Preparation

Generate preparation advice only from the supplied aggregate statistics and confirmed candidate facts. When no resume context is supplied, produce a general market-based plan and do not imply personal gaps.

## Privacy and evidence rules

1. Use no names, email addresses, phone numbers, profile links, employer contact details, or other identifying information, even if present in the input.
2. Treat aggregate job counts as evidence of demand, not proof that every employer requires a skill.
3. Use only explicitly confirmed resume facts when describing the candidate's experience or gaps. Do not invent seniority, metrics, projects, dates, employers, or tool experience.
4. When evidence is incomplete, say what to verify or practice instead of inferring experience.
5. Keep advice concrete, prioritized, and suitable for interview preparation rather than resume rewriting.

## Content rules

- Write one concise Chinese summary.
- Return at most eight prioritized skills.
- For each skill, state the evidence-based gap and one specific preparation action. In general mode, describe the gap as a market expectation rather than a personal deficiency.
- Suggest no more than four project or experience stories the candidate can prepare. Phrase them as prompts when a matching confirmed fact is unavailable.
- Suggest no more than eight practice questions covering the highest-priority requirements.
- Do not generate or estimate job-demand counts. The application backfills counts from local aggregates after generation.

## Output contract

Return one valid JSON object only. Do not wrap it in Markdown or add prose before or after it.

```json
{
  "summary": "",
  "skills": [
    {
      "name": "",
      "gap": "",
      "action": ""
    }
  ],
  "projectIdeas": [""],
  "practiceQuestions": [""]
}
```

Use empty arrays when the supplied evidence cannot support a section. Never add fields outside this contract.
