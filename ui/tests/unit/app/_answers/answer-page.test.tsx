/**
 * Unit tests for the answer pages (/hipaa-compliant-browser, /vs/browserbase and the rest), through what a
 * visitor and a crawler see: the answer first, every answer visible, structured data that repeats
 * the visible questions, the canonical address, and a place in the sitemap.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import HipaaCompliantBrowser, { metadata as hipaaMetadata } from '@/app/hipaa-compliant-browser/page';
import VsBrowserbase, { metadata as vsMetadata } from '@/app/vs/browserbase/page';
import VsSteel, { metadata as steelMetadata } from '@/app/vs/steel/page';
import VsAnchor, { metadata as anchorMetadata } from '@/app/vs/anchor/page';
import VsBrowserUse, { metadata as browserUseMetadata } from '@/app/vs/browser-use/page';
import AiBrowser, { metadata as aiMetadata } from '@/app/ai-browser/page';
import BrowserForHealthcare, { metadata as healthcareMetadata } from '@/app/browser-for-healthcare/page';
import HipaaBrowserAutomation, { metadata as automationMetadata } from '@/app/hipaa-browser-automation/page';
import PriorAuthorizationAutomation, { metadata as priorAuthMetadata } from '@/app/prior-authorization-automation/page';
import PayerPortalAutomation, { metadata as payerPortalsMetadata } from '@/app/payer-portal-automation/page';
import EligibilityVerificationAutomation, {
  metadata as eligibilityMetadata,
} from '@/app/eligibility-verification-automation/page';
import ClaimStatusAutomation, { metadata as claimStatusMetadata } from '@/app/claim-status-automation/page';
import DenialManagementAutomation, { metadata as denialsMetadata } from '@/app/denial-management-automation/page';
import ProviderEnrollmentAutomation, {
  metadata as enrollmentMetadata,
} from '@/app/provider-enrollment-automation/page';
import StateRegistryAutomation, { metadata as registriesMetadata } from '@/app/state-registry-automation/page';
import {
  AI_BROWSER,
  ANSWER_PAGES,
  CLAIM_STATUS,
  DENIALS,
  ELIGIBILITY,
  ENROLLMENT,
  REGISTRIES,
  HEALTHCARE_BROWSER,
  HIPAA_AUTOMATION,
  HIPAA_BROWSER,
  PAYER_PORTALS,
  PRIOR_AUTH,
  VS_ANCHOR,
  VS_BROWSERBASE,
  VS_BROWSER_USE,
  VS_STEEL,
} from '@/app/_answers/content';
import { SiteFooter } from '@/app/_home/site-footer';
import { Closing } from '@/app/_home/closing';
import { AnswerPage } from '@/app/_answers/answer-page';
import { SITE_DESCRIPTION, SITE_URL } from '@/lib/site';
import sitemap from '@/app/sitemap';

const PAGES = [
  { name: 'hipaa-compliant-browser', Page: HipaaCompliantBrowser, metadata: hipaaMetadata, content: HIPAA_BROWSER },
  { name: 'vs/browserbase', Page: VsBrowserbase, metadata: vsMetadata, content: VS_BROWSERBASE },
  { name: 'vs/steel', Page: VsSteel, metadata: steelMetadata, content: VS_STEEL },
  { name: 'vs/anchor', Page: VsAnchor, metadata: anchorMetadata, content: VS_ANCHOR },
  { name: 'vs/browser-use', Page: VsBrowserUse, metadata: browserUseMetadata, content: VS_BROWSER_USE },
  { name: 'ai-browser', Page: AiBrowser, metadata: aiMetadata, content: AI_BROWSER },
  {
    name: 'browser-for-healthcare',
    Page: BrowserForHealthcare,
    metadata: healthcareMetadata,
    content: HEALTHCARE_BROWSER,
  },
  {
    name: 'hipaa-browser-automation',
    Page: HipaaBrowserAutomation,
    metadata: automationMetadata,
    content: HIPAA_AUTOMATION,
  },
  {
    name: 'prior-authorization-automation',
    Page: PriorAuthorizationAutomation,
    metadata: priorAuthMetadata,
    content: PRIOR_AUTH,
  },
  {
    name: 'payer-portal-automation',
    Page: PayerPortalAutomation,
    metadata: payerPortalsMetadata,
    content: PAYER_PORTALS,
  },
  {
    name: 'eligibility-verification-automation',
    Page: EligibilityVerificationAutomation,
    metadata: eligibilityMetadata,
    content: ELIGIBILITY,
  },
  {
    name: 'claim-status-automation',
    Page: ClaimStatusAutomation,
    metadata: claimStatusMetadata,
    content: CLAIM_STATUS,
  },
  {
    name: 'denial-management-automation',
    Page: DenialManagementAutomation,
    metadata: denialsMetadata,
    content: DENIALS,
  },
  {
    name: 'provider-enrollment-automation',
    Page: ProviderEnrollmentAutomation,
    metadata: enrollmentMetadata,
    content: ENROLLMENT,
  },
  {
    name: 'state-registry-automation',
    Page: StateRegistryAutomation,
    metadata: registriesMetadata,
    content: REGISTRIES,
  },
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

  it('has its own social card, pointing at its own address', () => {
    expect(metadata.openGraph).toMatchObject({ title: content.title, description: content.lead, url: content.path });
    expect(metadata.twitter).toMatchObject({ title: content.title, description: content.lead });
  });

  it('is in the sitemap', () => {
    expect(sitemap().map((e) => e.url)).toContain(`${SITE_URL}${content.path}`);
  });

  it('is linked from the site footer, so crawlers reach it from every page', () => {
    render(<SiteFooter />);
    expect(screen.getByRole('link', { name: content.label }).getAttribute('href')).toBe(content.path);
  });

  it('is linked from the home page footer too, which is not the site footer', () => {
    render(<Closing />);
    const answers = screen.getByRole('navigation', { name: 'Answers' });
    expect(answers.querySelector(`a[href="${content.path}"]`)?.textContent).toBe(content.label);
  });

  it('never claims HIPAA without naming Oya Cloud or the enterprise edition', () => {
    const text = [content.lead, ...content.sections.map((s) => s.body), ...content.faq.map((f) => f.a)].join(' ');
    if (/HIPAA[- ]compliant|sign BAAs/i.test(text)) expect(text).toMatch(/Oya Cloud|enterprise edition/);
  });
});

describe('the answer page list', () => {
  it('has a test row for every page, so a new page cannot skip these rules', () => {
    expect(PAGES.map((p) => p.content)).toEqual(expect.arrayContaining(ANSWER_PAGES));
    expect(PAGES.length).toBe(ANSWER_PAGES.length);
  });
});

describe('the comparison pages', () => {
  const VS = [VS_BROWSERBASE, VS_STEEL, VS_ANCHOR, VS_BROWSER_USE];

  it('each open with a different answer, so they are not one page with the name swapped', () => {
    expect(new Set(VS.map((page) => page.lead)).size).toBe(VS.length);
    expect(new Set(VS.map((page) => page.sections[0].heading)).size).toBe(VS.length);
  });

  it('sit together at the end of the list, which groups them in both footers', () => {
    expect(ANSWER_PAGES.slice(-VS.length)).toEqual(VS);
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
