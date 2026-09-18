/**
 * Lint rules for the console: Next.js's recommended rules plus the project's
 * shared rules (tooling/eslint). React components get a larger line budget for
 * their JSX; their logic belongs in hooks, which get the standard one.
 */
import { defineConfig, globalIgnores } from 'eslint/config';
import nextVitals from 'eslint-config-next/core-web-vitals';
import nextTs from 'eslint-config-next/typescript';
import jsdoc from 'eslint-plugin-jsdoc';
import prettier from 'eslint-config-prettier';
import { localPlugin, baseRules, designRules, docRules, MAX_COMPONENT_LINES } from '../tooling/eslint/index.js';

export default defineConfig([
  ...nextVitals,
  ...nextTs,
  globalIgnores(['.next/**', 'out/**', 'build/**', 'coverage/**', 'next-env.d.ts', 'playwright-report/**', 'test-results/**']),
  {
    files: ['src/**/*.{ts,tsx}'],
    plugins: { local: localPlugin, jsdoc },
    rules: { ...baseRules, ...docRules() },
  },
  { files: ['src/**/*.ts'], rules: designRules() },
  { files: ['src/**/*.tsx'], rules: designRules(MAX_COMPONENT_LINES) },
  prettier,
]);
