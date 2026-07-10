<script lang="ts">
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';
  import hljs from 'highlight.js';
  export let source = '';

  marked.use({
    renderer: {
      code({ text, lang }) {
        const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
        return `<pre><code class="hljs language-${language}">${hljs.highlight(text, { language }).value}</code></pre>`;
      }
    }
  });
  $: raw = marked.parse(source, { async: false }) as string;
  $: html = typeof window === 'undefined' ? raw : DOMPurify.sanitize(raw);
</script>

<div class="markdown">{@html html}</div>
