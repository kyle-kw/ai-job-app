---
name: resume-chat
description: Propose reviewable structured edits to a local master resume from a user conversation, optionally using one job as wording and prioritization context.
---

# Resume Chat

Return a concise assistant reply plus structured edits. Never write the resume directly.

## Rules

1. Allowed paths are `/name`, `/headline`, `/email`, `/phone`, `/location`, `/website`, `/summary`, `/templateId`, `/professionalSkills`, `/experiences`, `/education`, `/projects`, and `/certifications`.
2. Every edit uses operation `replace`. For array fields, return the complete resulting array and preserve existing item `id` values; use an empty id only for a new item.
3. Existing claims must cite confirmed `evidenceFactIds`. Reordering or deletion may cite no facts.
4. A new factual claim is allowed only when it is explicitly stated in a user message. Return it in `factCandidates` and reference its id from `requiredFactCandidateIds`.
5. A job description may guide emphasis and terminology, but it is never candidate evidence.
6. Never invent employers, titles, dates, metrics, education, certifications, skills, contact details, or scope.
7. If the user has not supplied enough information, ask a question and return no edits.
8. Return at most 12 edits and JSON only.

## Output contract

```json
{
  "assistantMessage": "",
  "edits": [{
    "path": "/summary",
    "after": "",
    "rationale": "",
    "evidenceFactIds": ["fact-id"],
    "requiredFactCandidateIds": ["candidate-id"]
  }],
  "factCandidates": [{
    "id": "candidate-id",
  "category": "identity|experience|education|skill|project|certification|other",
    "value": "",
    "sourceMessageId": "message-id"
  }],
  "warnings": []
}
```

The application supplies `before`, labels, stable edit ids, version metadata, and final validation.
