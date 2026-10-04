/**
 * Lint rules for the desktop browser: the project's shared rules
 * (tooling/eslint) on the main process, preload, renderer and page scripts.
 * Formatting is Prettier's job (npm run format).
 */
import js from '@eslint/js';
import jsdoc from 'eslint-plugin-jsdoc';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import { localPlugin, baseRules, designRules, docRules, MAX_COMPONENT_LINES } from '../tooling/eslint/index.js';

/**
 * Code that runs inside the web pages a person visits. A page can read these
 * functions' source and shape, so restructuring them changes what anti-bot
 * checks see; they keep their form on purpose.
 */
const PAGE_SIDE = ['scripts/analyzer.js', 'anonymity/stealth.js', 'anonymity/fingerprint.js', 'anonymity/inject.js'];

/**
 * The managed browser's host matcher: a byte-for-byte copy of the one in
 * server/src/modules/control/egress.ts (server/tests/integration/egress-rules.test.js
 * compares them), so it keeps that file's shape and numbers, and skips type checks.
 */
const HOST_RULES = 'src/main/identity/host-rules.ts';

export default [
  { ignores: ['node_modules', 'dist', 'build', 'coverage', 'out'] },
  js.configs.recommended,
  {
    languageOptions: { ecmaVersion: 'latest', sourceType: 'commonjs', globals: { ...globals.node } },
    plugins: { local: localPlugin, jsdoc },
    rules: {
      ...baseRules,
      ...docRules(),
      'no-unused-vars': [
        'error',
        { args: 'after-used', argsIgnorePattern: '^_', caughtErrors: 'none', ignoreRestSiblings: true },
      ],
    },
  },
  { files: ['**/*.mjs'], languageOptions: { sourceType: 'module' } },
  // The renderer and page scripts run in a browser context.
  {
    files: ['src/renderer/public/**/*.js', 'scripts/analyzer.js', 'anonymity/stealth.js'],
    languageOptions: { sourceType: 'script', globals: { ...globals.browser } },
  },
  { files: ['**/*.{js,cjs,mjs}'], ignores: [...PAGE_SIDE, 'tests/**'], rules: designRules() },
  // Page-side files keep their exact text: no added headers or doc comments either.
  { files: PAGE_SIDE, rules: { 'local/file-header': 'off', 'jsdoc/require-jsdoc': 'off', 'no-unused-vars': 'off' } },
  // TypeScript: the same rules, through typescript-eslint. React views get the component budget.
  ...tseslint.configs.recommended.map((config) => ({ ...config, files: ['**/*.{ts,tsx}'] })),
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: { sourceType: 'module' },
    rules: {
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { args: 'after-used', argsIgnorePattern: '^_', caughtErrors: 'none', ignoreRestSiblings: true },
      ],
    },
  },
  { files: ['**/*.ts'], ignores: ['tests/**', HOST_RULES], rules: designRules() },
  { files: [HOST_RULES], rules: { '@typescript-eslint/ban-ts-comment': 'off' } },
  // Tests are not type-checked (their fakes are partial by design), so they may say `any`.
  { files: ['tests/**/*.ts'], rules: { '@typescript-eslint/no-explicit-any': 'off' } },
  { files: ['**/*.tsx'], ignores: ['tests/**'], rules: designRules(MAX_COMPONENT_LINES) },
  { files: ['src/renderer/**/*.{ts,tsx}'], languageOptions: { globals: { ...globals.browser } } },
  { files: ['**/*.tsx'], ...reactHooks.configs.flat['recommended-latest'] },
  // Electron comes in through constructors: only the composition root imports it, so tests hand over fakes.
  {
    files: ['src/main/**/*.ts'],
    ignores: ['src/main/main.ts'],
    rules: {
      'no-restricted-imports': [
        'error',
        { paths: [{ name: 'electron', message: 'Take it from your deps.', allowTypeImports: true }] },
      ],
    },
  },
  prettier,
];
