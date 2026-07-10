---
name: job-fit
description: Evaluate a job posting against a confirmed resume and stated job preferences, producing evidence-backed fit dimensions, hard constraints, strengths, gaps, confidence, and a recommendation. Use when ranking jobs or opening a job's match analysis.
---

# Job Fit

Evaluate capability and desirability separately. Use only confirmed resume evidence and explicit preferences.

## Scoring

- Technical skills: weight 30.
- Experience: weight 25.
- Behavioral and culture fit: weight 15.
- Career alignment: weight 30.
- Location and explicit hard constraints: pass, flag, fail, or unknown; do not include them in the weighted average.

Set a dimension score to `null` when evidence is insufficient. Re-normalize the overall score over known dimensions and set `confidence` to the sum of known weights. A failed hard constraint caps the overall score at 44.

Use verdicts: `strong` for 75+, `good` for 60–74, `moderate` for 45–59, `weak` for 30–44, and `poor` below 30.

## Integrity rules

1. Cite concrete resume facts and exact JD requirements.
2. Do not treat a keyword alone as proof of professional experience.
3. State meaningful gaps without converting them into experience.
4. Do not infer company culture beyond the posting; mark it unknown.
5. Return JSON only.

## Output contract

```json
{
  "overallScore": 0,
  "confidence": 0,
  "verdict": "strong|good|moderate|weak|poor",
  "recommendation": "",
  "summary": "",
  "dimensions": [{
    "key": "technical|experience|behavior|career",
    "label": "",
    "score": null,
    "weight": 0,
    "note": "",
    "evidence": [""]
  }],
  "hardConstraints": [{"label": "", "status": "pass|flag|fail|unknown", "note": ""}],
  "strengths": [""],
  "gaps": [""],
  "evidence": [""],
  "generatedAt": "ISO-8601",
  "skillVersion": "job-fit@1.0.0"
}
```
