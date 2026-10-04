/**
 * Finds a step's target in the page: turns a locator candidate into a
 * Playwright locator, with `{{variables}}` in its value filled in.
 */
import type { FrameLocator, Locator, Page } from '@playwright/test';
import { PLACEHOLDER, roleName, type Candidate, type Draft } from '../workflow/index.ts';

/** Where locators are made: a page, or a frame inside it. */
export type Scope = Page | FrameLocator;

/** The ARIA role getByRole takes. */
type Role = Parameters<Page['getByRole']>[0];

/** The Playwright call behind each locator kind. */
const LOCATE: Record<string, (page: Scope, c: Candidate) => Locator> = {
  css: (page, c) => page.locator(c.value),
  role: (page, c) => page.getByRole(c.role as Role, { name: roleName(c.value) }),
  testId: (page, c) => page.getByTestId(c.value),
  label: (page, c) => page.getByLabel(c.value, { exact: true }),
  text: (page, c) => page.getByText(c.value, { exact: true }),
  placeholder: (page, c) => page.getByPlaceholder(c.value, { exact: true }),
};

/** A Playwright locator for `candidate` under `page` (a page or a frame). */
export function locate(page: Scope, candidate: Candidate): Locator {
  if (!Object.hasOwn(LOCATE, candidate.kind)) throw new TypeError(`Unknown locator kind: ${candidate.kind}`);
  return LOCATE[candidate.kind](page, candidate);
}

/** The candidate with its placeholders filled from the run's values, then the draft's defaults. */
export function resolveCandidate(
  candidate: Candidate,
  vars: Record<string, unknown> | undefined,
  draft: Draft,
): Candidate {
  const fill = (_: string, key: string) => String(vars?.[key] ?? draft.variables[key]?.default ?? '');
  return { ...candidate, value: candidate.value.replace(PLACEHOLDER, fill) };
}
