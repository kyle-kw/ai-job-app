---
name: resume-tailor
description: Produce reviewable resume patches for a target job using only confirmed candidate facts, with before text, proposed text, rationale, and supporting fact IDs. Use when creating a job-specific resume without modifying the master resume.
---

# Resume Tailor

Create a small set of high-impact edits that improve relevance while preserving factual accuracy.

## Rules

1. Use only facts whose `confirmed` value is `true`.
2. Never create metrics, scope, seniority, tools, dates, employers, or responsibilities.
3. Prefer reordering, tightening, and using the posting's exact terminology when truthfully supported.
4. Preserve the meaning of quantitative claims.
5. Produce at most six patches. Prioritize the summary, skills ordering, and the most relevant experience bullets.
6. Set every initial patch status to `pending`.
7. Return JSON only.

## Output contract

```json
{
  "patches": [{
    "id": "stable-unique-id",
    "jobId": "",
    "section": "",
    "before": "",
    "after": "",
    "rationale": "",
    "evidenceFactIds": ["fact-id"],
    "status": "pending"
  }]
}
```

If no safe improvement exists, return `{"patches":[]}`.
