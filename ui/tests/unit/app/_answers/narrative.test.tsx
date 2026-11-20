/** Narrative copy follows the reader's task while search metadata, compliance evidence and rendering contracts stay stable. */
import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { AI_BROWSER, ANSWER_PAGES, BAA } from '@/app/_answers/content';
import { AnswerSection } from '@/app/_answers/answer-section';
import { leadParagraphs, layoutOf } from '@/app/_answers/presentation';

afterEach(cleanup);

it.each(ANSWER_PAGES)('explains the BAA acronym in the visible narrative on $path', (content) => {
  const narrative = [
    ...leadParagraphs(content),
    ...content.sections.map(({ body }) => body),
    ...content.faq.map(({ a }) => a),
  ].join(' ');
  expect(narrative).toMatch(/Business Associate Agreements? \(BAAs?\)/);
});

it('preserves the established metadata values across all 28 routes', () => {
  const metadata = ANSWER_PAGES.map(({ path, title, lead }) => [path, title, lead]).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  // Captured from the pre-narrative records and independently compared with HEAD on 2026-11-18.
  // Recaptured on 2026-11-20 for the regulated-industries page, and again after adding the five
  // new /vs/* comparison pages while keeping all 23 earlier routes unchanged.
  expect(createHash('sha256').update(JSON.stringify(metadata)).digest('hex')).toBe(
    'a9d92d3b8f1299201080f3ff06334c866fa8597cbe8db3729fbd0716f3136133',
  );
});

it('takes a desktop reader from choosing a start through delegation, intervention, reuse and team scope', () => {
  expect(AI_BROWSER.sections.map(({ composition }) => composition)).toEqual([
    'start',
    'tabs',
    'takeover',
    'saved',
    'deployment',
  ]);
  expect(AI_BROWSER.sections.find(({ composition }) => composition === 'saved')?.body).toContain(
    'a playbook: readable Playwright code',
  );
  expect(AI_BROWSER.hero?.action.href).toBe('/#download');
});

it('keeps every BAA factual body verbatim, including dates and verification qualifications', () => {
  expect(BAA.sections.map(({ body }) => body)).toEqual([
    'Both run the same browsers, agents and playbooks.',
    'HIPAA mode narrows where PHI can go, so every party on its path is covered.',
    'HIPAA mode on Oya Cloud runs on Google Cloud, under Google Cloud’s BAA. Oya signed BAAs with Google Cloud, Vertex AI (under the Google Cloud BAA) and Sentry on 2026-10-07.',
    'The audit log is append-only and tamper-evident, and audit records are kept for six years.',
    '14 of 15 HIPAA Security Rule controls are verified, from evidence regenerated on 2026-10-11. The trust center offers these on request.',
  ]);
  const facts = BAA.sections.flatMap(({ items }) => items ?? []).join(' ');
  for (const fact of [
    'Models run only on hosts covered by a BAA.',
    'No outside CAPTCHA solvers, proxies or browser vendors',
    'dedicated GKE cluster',
    'Cloud SQL for PostgreSQL',
    'Vertex AI',
  ])
    expect(facts).toContain(fact);
});

describe('explicit composition metadata', () => {
  it('retains the saved-steps visual when a writer changes its chapter heading', () => {
    const section = {
      ...AI_BROWSER.sections.find(({ composition }) => composition === 'saved')!,
      heading: 'A newly worded chapter',
    };
    render(<AnswerSection section={section} path="/ai-browser" />);
    expect(screen.getByRole('heading', { name: 'A newly worded chapter' })).toBeTruthy();
    expect(screen.getByText('Saved steps')).toBeTruthy();
    expect(screen.getByText('Run again')).toBeTruthy();
    expect(layoutOf(section)).toBe('flow');
  });
  it('keeps legacy content complete when no narrative payload exists', () => {
    const legacy = { ...AI_BROWSER, hero: undefined };
    expect(leadParagraphs(legacy)).toEqual([legacy.lead]);
  });
});
