/**
 * Lint rules for the server: the project's shared rules (tooling/eslint) on
 * TypeScript sources. Formatting is Prettier's job (npm run format).
 */
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import jsdoc from 'eslint-plugin-jsdoc';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import { localPlugin, baseRules, designRules, docRules, ROUTE_DOC_CONTEXTS } from '../tooling/eslint/index.js';

export default tseslint.config(
  { ignores: ['node_modules', 'data', 'downloads', 'src/public', 'coverage'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: { globals: globals.node },
    plugins: { local: localPlugin },
    rules: {
      ...baseRules,
      // Unused catch bindings and leading callback parameters are how Node APIs read.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { args: 'after-used', argsIgnorePattern: '^_', caughtErrors: 'none', ignoreRestSiblings: true },
      ],
      // Migration ratchet with tsconfig's noImplicitAny: the renamed JS still carries explicit anys.
      '@typescript-eslint/no-explicit-any': 'off',
    },
  },
  {
    files: ['src/**/*.ts'],
    plugins: { jsdoc },
    rules: docRules(ROUTE_DOC_CONTEXTS),
  },
  {
    files: ['src/**/*.ts'],
    // Its host matcher must stay byte-identical to browser/governance.js (tests/integration/egress-rules.test.js).
    ignores: ['src/modules/control/egress.ts'],
    rules: designRules(),
  },
  {
    files: ['tests/**'],
    rules: {
      // Tests are scripts: top-level await, fixtures that tolerate anything.
      '@typescript-eslint/no-unused-expressions': 'off',
    },
  },
  prettier,
);
