/**
 * Playbooks: an ask() run frozen into steps that replay without the LLM.
 *
 * Steps carry the analyzer's stable element metadata (testId, DOM id, aria-label,
 * text, name), never the numeric id, which dies with each analysis. Every value that
 * was typed or picked is a `{{name}}` placeholder filled at replay, with the
 * recorded value kept as its default, so a playbook is a form, not a transcript, and
 * a run that passes nothing still does what was demonstrated. Passwords are the one
 * value with no default: they never leave the page. Replay re-analyzes and matches,
 * so it works on every provider. The Playwright code is an export to read or run
 * yourself; nothing here ever evaluates it.
 *
 * This is the module's entry point; the work lives in the files it re-exports.
 */
export { matchElement } from './match.ts';
export { sanitizeSteps, validateWorkflow } from './sanitize.ts';
export { variablesOf, missingVariables, templateValues } from './variables.ts';
export { renderPlaywright } from './playwright.ts';
export { create, promote, rename, list, remove } from './catalog.ts';
export { play } from './replay.ts';
