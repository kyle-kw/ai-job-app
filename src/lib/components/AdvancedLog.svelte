<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  export let logs: string[] = [];
  let host: HTMLDivElement;
  let terminal: import('@xterm/xterm').Terminal | null = null;
  let fitAddon: import('@xterm/addon-fit').FitAddon | null = null;

  onMount(async () => {
    const [{ Terminal }, { FitAddon }] = await Promise.all([import('@xterm/xterm'), import('@xterm/addon-fit')]);
    await import('@xterm/xterm/css/xterm.css');
    terminal = new Terminal({ rows: 8, fontSize: 11, fontFamily: 'JetBrains Mono, Consolas, monospace', theme: { background: '#18201d', foreground: '#dce9e4', cursor: '#65bda3' }, disableStdin: true });
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    fitAddon.fit();
    logs.forEach((line) => terminal?.writeln(line));
  });

  $: if (terminal) {
    terminal.clear();
    logs.forEach((line) => terminal?.writeln(line));
  }
  onDestroy(() => terminal?.dispose());
</script>

<div bind:this={host} class="min-h-[150px] overflow-hidden rounded-xl bg-[#18201d] p-2"></div>
