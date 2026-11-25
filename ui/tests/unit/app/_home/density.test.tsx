/** A visible-copy budget keeps the landing story concise while its technical detail remains available. */
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import Home from '@/app/page';

vi.mock('next/headers', () => ({ headers: async () => new Headers() }));
vi.stubGlobal(
  'IntersectionObserver',
  class {
    observe() {}
    unobserve() {}
    disconnect() {}
  },
);
vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
afterEach(cleanup);

/** Counts text a reader encounters before opening optional technical disclosures. */
function readingWords(container: HTMLElement) {
  const regions = container.querySelectorAll(
    '[aria-label="From first workflow to ongoing operation"], #capabilities, #deployment',
  );
  return [...regions].flatMap((region) => {
    const copy = region.cloneNode(true) as HTMLElement;
    copy.querySelectorAll('[hidden]').forEach((node) => node.remove());
    copy
      .querySelectorAll('details:not([open])')
      .forEach((node) => node.replaceChildren(node.querySelector('summary')!.cloneNode(true)));
    const walker = document.createTreeWalker(copy, NodeFilter.SHOW_TEXT),
      text: string[] = [];
    while (walker.nextNode()) text.push(walker.currentNode.textContent ?? '');
    return text.join(' ').trim().split(/\s+/);
  });
}

it('keeps the initial maintenance, capability and deployment story within its reading budget', async () => {
  const { container } = render(await Home());
  const count = readingWords(container).length;
  // Narrative context supersedes the earlier130-word fragment budget; visible qualifications remain mandatory.
  expect(count).toBeLessThanOrEqual(240);
});

/** Removes optional depth before checking that material scope is visible on the initial reading path. */
function initialCopy(container: HTMLElement) {
  const copy = container.cloneNode(true) as HTMLElement;
  copy.querySelectorAll('[hidden]').forEach((node) => node.remove());
  copy
    .querySelectorAll('details:not([open])')
    .forEach((node) => node.replaceChildren(node.querySelector('summary')!.cloneNode(true)));
  return copy;
}

it('keeps development, PHI and provider boundaries visible beside the story', async () => {
  const { container } = render(await Home());
  const copy = initialCopy(container);
  for (const phrase of [
    'synthetic data',
    'protected health information (PHI)',
    'Oya Cloud',
    'enterprise edition',
    'Business Associate Agreement (BAA)',
    'Connected providers have different runtime guarantees',
    'HIPAA mode uses Oya’s own browsers',
  ])
    expect(copy.textContent).toContain(phrase);
  for (const paragraph of copy.querySelectorAll(
    '[aria-label="From first workflow to ongoing operation"] p, #capabilities p, #deployment p',
  ))
    expect(paragraph.textContent?.trim().split(/\s+/).length).toBeLessThanOrEqual(60);
});
