import js from '@eslint/js';
import globals from 'globals';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';

const unusedOptions = {
  argsIgnorePattern: '^_',
  caughtErrorsIgnorePattern: '^_',
  destructuredArrayIgnorePattern: '^_',
  varsIgnorePattern: '^_'
};

export default tseslint.config(
  {
    ignores: [
      '.svelte-kit/**',
      '.uv-cache/**',
      'build/**',
      'node_modules/**',
      'release-assets/**',
      'sidecar/**',
      'src-tauri/**'
    ]
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    files: ['src/**/*.{ts,svelte}', 'vite.config.ts'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.es2024
      }
    },
    rules: {
      'no-unused-vars': 'off',
      'no-control-regex': 'off',
      '@typescript-eslint/no-unused-vars': ['error', unusedOptions]
    }
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser
      }
    },
    rules: {
      'no-useless-assignment': 'off',
      '@typescript-eslint/no-unused-expressions': 'off',
      'svelte/infinite-reactive-loop': 'off',
      'svelte/no-at-html-tags': 'off',
      'svelte/no-navigation-without-resolve': 'off',
      'svelte/prefer-svelte-reactivity': 'off',
      'svelte/require-each-key': 'off'
    }
  },
  {
    files: ['*.{js,cjs}', 'scripts/**/*.mjs'],
    languageOptions: {
      globals: globals.node
    }
  },
  {
    files: ['src/**/*.test.ts'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node
      }
    },
    rules: {
      'svelte/no-navigation-without-resolve': 'off'
    }
  }
);
