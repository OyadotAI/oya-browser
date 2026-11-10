/**
 * Lint rules for the packages (packages/sdk, packages/cli, packages/extension): the
 * project's shared rules (tooling/eslint). The server, ui and browser each
 * have their own config in their folder.
 */
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import jsdoc from 'eslint-plugin-jsdoc';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import { localPlugin, baseRules, designRules, docRules } from './tooling/eslint/index.js';

export default tseslint.config(
  // Installed third-party skill assets are tooling artifacts outside application lint scope.
  {
    ignores: [
      '**/node_modules',
      '**/dist',
      '**/coverage',
      '.claude/skills/**',
      '.claude/worktrees/**',
      '.keel/worktrees/**',
      '.codex/worktrees/**',
      'server',
      'ui',
      'browser',
      'examples',
      'qa',
      'scripts',
      'skills',
      'k8s',
      'supabase',
      'docs',
      'mars',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  { languageOptions: { parserOptions: { tsconfigRootDir: import.meta.dirname } } },
  {
    files: ['packages/**/*.{ts,js,mjs}', 'tooling/**/*.js'],
    languageOptions: { globals: globals.node },
    plugins: { local: localPlugin, jsdoc },
    rules: {
      ...baseRules,
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', caughtErrors: 'none', ignoreRestSiblings: true },
      ],
    },
  },
  { files: ['compliance/**/*.mjs'], languageOptions: { globals: globals.node } },
  { files: ['packages/*/src/**/*.ts', 'packages/extension/src/**/*.js'], rules: { ...docRules(), ...designRules() } },
  // The extension runs in the browser, not Node: plain JS with no build step, so the store reviewers read what runs.
  {
    files: ['packages/extension/src/**/*.js'],
    languageOptions: { globals: { ...globals.browser, ...globals.webextensions } },
  },
  prettier,
);
