/**
 * Lint rules for the published packages (packages/sdk, packages/cli): the
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
  { ignores: ['**/node_modules', '**/dist', '**/coverage', 'server', 'ui', 'browser', 'examples', 'qa', 'scripts', 'skills', 'k8s', 'supabase', 'docs'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['packages/**/*.{ts,js,mjs}', 'tooling/**/*.js'],
    languageOptions: { globals: globals.node },
    plugins: { local: localPlugin, jsdoc },
    rules: {
      ...baseRules,
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', caughtErrors: 'none', ignoreRestSiblings: true }],
    },
  },
  { files: ['compliance/**/*.mjs'], languageOptions: { globals: globals.node } },
  { files: ['packages/*/src/**/*.ts'], rules: { ...docRules(), ...designRules() } },
  prettier,
);
