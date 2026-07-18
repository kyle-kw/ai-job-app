<script lang="ts">
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';

  export let source = '';
  $: html = DOMPurify.sanitize(marked.parse(source, { async: false }) as string);
</script>

<div class="markdown-view text-sm leading-7 body-muted">{@html html}</div>

<style>
  .markdown-view :global(h1),
  .markdown-view :global(h2),
  .markdown-view :global(h3) {
    margin: 0 0 10px;
    color: var(--ink);
    font-size: 14px;
    font-weight: 700;
  }
  .markdown-view :global(p) {
    margin: 8px 0;
  }
  .markdown-view :global(ul),
  .markdown-view :global(ol) {
    margin: 8px 0;
    padding-left: 20px;
  }
  .markdown-view :global(ul) {
    list-style: disc;
  }
  .markdown-view :global(ol) {
    list-style: decimal;
  }
  .markdown-view :global(blockquote) {
    margin: 12px 0 0;
    border-left: 3px solid var(--brand);
    padding: 8px 12px;
    background: var(--brand-faint);
    color: var(--ink);
  }
  .markdown-view :global(strong) {
    color: var(--ink);
  }
</style>
