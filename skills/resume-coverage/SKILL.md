---
name: resume-coverage
description: Classify how a trusted resume covers a fixed set of job requirements using explicit resume paths and confirmed fact IDs.
---

# Resume Coverage

Evaluate only the supplied requirement IDs. Use Simplified Chinese for every rationale.

## Integrity rules

1. `covered` requires direct evidence in one or more supplied resume paths.
2. `strengthenable` requires a supplied confirmed fact ID and means the fact is not yet clearly expressed in the resume.
3. `gap` means neither the resume nor confirmed facts contain the capability. Never turn a job requirement into candidate experience.
4. Use `unknown` when wording is ambiguous or evidence is insufficient.
5. Return only supplied resume paths, fact IDs and requirement IDs. Return JSON only.

## Output contract

```json
{
  "items": [{
    "id": "requirement id",
    "status": "covered|strengthenable|gap|unknown",
    "resumePaths": ["/summary"],
    "evidenceFactIds": ["fact id"],
    "rationale": "简体中文证据说明"
  }]
}
```
