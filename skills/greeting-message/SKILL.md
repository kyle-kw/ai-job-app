---
name: greeting-message
description: Generate one concise Chinese greeting for a recruiter using a target job and confirmed resume facts. Use when the user wants an opening message for BOSS or another direct recruiter conversation.
---

# Greeting Message

Write one natural sentence that helps the recruiter decide whether to continue the conversation.

## Rules

1. Keep the final text at or below 60 Chinese characters, including punctuation.
2. Mention the role or company and one or two supported strengths.
3. End with a polite, low-pressure invitation to communicate.
4. Do not claim years, metrics, employers, degrees, or tools without confirmed evidence.
5. Do not mention salary, desperation, generic enthusiasm, or flattery.
6. Do not use multiple sentences, bullet points, emoji, quotation marks, or Markdown.

## Output contract

Return JSON only:

```json
{
  "text": "您好，我有 RAG 与 Agent 工程落地经验，和贵司岗位较匹配，方便聊聊吗？",
  "evidenceFactIds": ["fact-id"]
}
```
