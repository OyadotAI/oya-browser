/**
 * Unit tests for the answer pages (/hipaa-compliant-browser and /vs/browserbase), through what a
 * visitor and a crawler see: the answer first, every answer visible, structured data that repeats
 * the visible questions, the canonical address, and a place in the sitemap.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import HipaaCompliantBrowser, { metadata as hipaaMetadata } from '@/app/hipaa-compliant-browser/page';
import VsBrowserbase, { metadata as vsMetadata } from '@/app/vs/browserbase/page';
import { HIPAA_BROWSER, VS_BROWSERBASE } from '@/app/_answers/content';
import { AnswerPage } from '@/app/_answers/answer-page';
import { SITE_DESCRIPTION, SITE_URL } from '@/lib/site';
import sitemap from '@/app/sitemap';

const PAGES = [
  { name: 'hipaa-compliant-browser', Page: HipaaCompliantBrowser, metadata: hipaaMetadata, content: HIPAA_BROWSER },
  { name: 'vs/browserbase', Page: VsBrowserbase, metadata: vsMetadata, content: VS_BROWSERBASE },
];

/** The FAQPage structured data the page rendered. */
function structuredData(container: HTMLElement) {
  const script = container.querySelector('script[type="application/ld+json"]') as HTMLScriptElement;
  return JSON.parse(script.textContent ?? '');
}

describe.each(PAGES)('/$name', ({ Page, metadata, content }) => {
  afterEach(() => cleanup());

  it('puts the answer under the question, as the first paragraph', () => {
    render(<Page />);
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(content.title);
    expect(screen.getByText(content.lead)).toBeTruthy();
  });

  it('shows every answer, not collapsed', () => {
    render(<Page />);
    for (const { q, a } of content.faq) {
      expect(screen.getByRole('heading', { name: q })).toBeTruthy();
      expect(screen.getByText(a)).toBeTruthy();
    }
  });

  it('repeats exactly the visible questions as FAQPage structured data', () => {
    const { container } = render(<Page />);
    const data = structuredData(container);
    expect(data['@type']).toBe('FAQPage');
    expect(
      data.mainEntity.map((e: { name: string; acceptedAnswer: { text: string } }) => [e.name, e.acceptedAnswer.text]),
    ).toEqual(content.faq.map(({ q, a }) => [q, a]));
  });

  it('has its own title, its answer as the description, and a canonical address', () => {
    expect(metadata).toMatchObject({ title: content.title, description: content.lead });
    expect(metadata.alternates?.canonical).toBe(content.path);
  });

  it('is in the sitemap', () => {
    expect(sitemap().map((e) => e.url)).toContain(`${SITE_URL}${content.path}`);
  });
});

describe('answer page structured data', () => {
  afterEach(() => cleanup());

  it('escapes "<", so no answer can close the script tag', () => {
    const hostile = { ...HIPAA_BROWSER, faq: [{ q: 'q', a: '</script><script>alert(1)</script>' }] };
    const { container } = render(<AnswerPage content={hostile} />);
    const scripts = container.querySelectorAll('script');
    expect(scripts.length).toBe(1);
    expect(scripts[0].innerHTML.includes('</')).toBe(false);
    expect(structuredData(container).mainEntity[0].acceptedAnswer.text).toBe(hostile.faq[0].a);
  });
});

describe('the canonical description', () => {
  it('names a HIPAA-compliant browser, an AI browser and an agentic browser within the first 160 characters', () => {
    const shown = SITE_DESCRIPTION.slice(0, 160);
    for (const phrase of ['HIPAA-compliant browser', 'AI browser', 'agentic browser']) expect(shown).toContain(phrase);
  });
});
