/**
 * Lint rules for the desktop browser: the project's shared rules
 * (tooling/eslint) on the main process, preload, renderer and page scripts.
 * Formatting is Prettier's job (npm run format).
 */
import js from '@eslint/js';
import jsdoc from 'eslint-plugin-jsdoc';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import { localPlugin, baseRules, designRules, docRules } from '../tooling/eslint/index.js';

/**
 * Code that runs inside the web pages a person visits. A page can read these
 * functions' source and shape, so restructuring them changes what anti-bot
 * checks see; they keep their form on purpose. governance.js shares a matcher
 * with the server that must stay byte-identical.
 */
const PAGE_SIDE = [
  'scripts/analyzer.js',
  'anonymity/stealth.js',
  'anonymity/fingerprint.js',
  'anonymity/inject.js',
  'governance.js',
];

export default [
  { ignores: ['node_modules', 'dist', 'build', 'coverage'] },
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
    files: ['renderer/**/*.js', 'scripts/analyzer.js', 'anonymity/stealth.js'],
    languageOptions: { sourceType: 'script', globals: { ...globals.browser } },
  },
  { files: ['**/*.{js,cjs,mjs}'], ignores: [...PAGE_SIDE, 'tests/**'], rules: designRules() },
  // Page-side files keep their exact text: no added headers or doc comments either.
  { files: PAGE_SIDE, rules: { 'local/file-header': 'off', 'jsdoc/require-jsdoc': 'off', 'no-unused-vars': 'off' } },
  prettier,
];
