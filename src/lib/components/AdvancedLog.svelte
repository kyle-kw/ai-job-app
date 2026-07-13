<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  export let logs: string[] = [];
  let host: HTMLDivElement;
  let terminal: import('@xterm/xterm').Terminal | null = null;
  let fitAddon: import('@xterm/addon-fit').FitAddon | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let renderedLogs: string[] = [];

  function syncLogs() {
    if (!terminal) return;
    const prefixUnchanged = renderedLogs.length <= logs.length
      && renderedLogs.every((line, index) => logs[index] === line);
    if (!prefixUnchanged) {
      terminal.reset();
      logs.forEach((line) => terminal?.writeln(line));
    } else {
      logs.slice(renderedLogs.length).forEach((line) => terminal?.writeln(line));
    }
    renderedLogs = [...logs];
  }

  onMount(async () => {
    const [{ Terminal }, { FitAddon }] = await Promise.all([import('@xterm/xterm'), import('@xterm/addon-fit')]);
    await import('@xterm/xterm/css/xterm.css');
    terminal = new Terminal({ rows: 8, fontSize: 11, fontFamily: 'JetBrains Mono, Consolas, monospace', theme: { background: '#18201d', foreground: '#dce9e4', cursor: '#65bda3' }, disableStdin: true });
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    fitAddon.fit();
    syncLogs();
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => fitAddon?.fit());
      resizeObserver.observe(host);
    }
  });

  $: logs, syncLogs();
  onDestroy(() => { resizeObserver?.disconnect(); terminal?.dispose(); });
</script>

<div bind:this={host} aria-hidden="true" class="min-h-[150px] overflow-hidden rounded-xl bg-[#18201d] p-2"></div>
<pre class="sr-only" aria-live="polite" aria-label="任务运行日志">{logs.join('\n')}</pre>
