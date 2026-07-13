import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig(({ mode }) => ({
  plugins: [sveltekit()],
  clearScreen: false,
  resolve: mode === 'test' ? { conditions: ['browser'] } : undefined,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: [
        '**/src-tauri/target/**',
        '**/src-tauri/binaries/**',
        '**/sidecar/.venv/**',
        '**/sidecar/.build-venv/**',
        '**/sidecar/tests/output/**'
      ]
    }
  },
  envPrefix: ['VITE_'],
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    setupFiles: ['./src/test-setup.ts']
  }
}));
