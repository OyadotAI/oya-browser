/**
 * The Terms of Service: the agreement between a customer and Oya AI Corp for
 * Oya Browser and Oya Cloud. Written plainly; the parts that matter most to an
 * automation product (what agents may be told to do, and who answers for it)
 * come early.
 */
import type { Metadata } from 'next';
import { SITE_ALTERNATE_TYPES } from '@/lib/site';
import Link from 'next/link';
import { LegalPage, type LegalSection } from '../_legal/legal-page';
import { COMPANY, EFFECTIVE_DATE, GOVERNING_LAW, LEGAL_EMAIL } from '../_legal/content';

/** The page's title and description for search and link previews. */
export const metadata: Metadata = {
  title: 'Terms of Service',
  description: 'The terms for using Oya Browser and Oya Cloud.',
  alternates: { canonical: '/terms', types: SITE_ALTERNATE_TYPES },
};

/** The email link used throughout. */
const mail = <a href={`mailto:${LEGAL_EMAIL}`}>{LEGAL_EMAIL}</a>;

/** The terms, section by section. */
const SECTIONS: readonly LegalSection[] = [
  {
    heading: 'The agreement',
    body: (
      <p>
        These terms are an agreement between you and {COMPANY} (&quot;Oya&quot;, &quot;we&quot;) for Oya Browser, Oya
        Cloud at oyabrowser.com, the SDK, CLI and APIs (the &quot;Service&quot;). If you use the Service for a company,
        you accept them for that company and confirm you may. A signed order form or enterprise agreement takes
        precedence over these terms where they differ.
      </p>
    ),
  },
  {
    heading: 'Your account',
    body: (
      <p>
        Give accurate details, keep your sign-in and API keys secret, and tell us at {mail} if you think someone else
        has used them. You are responsible for what happens under your account and your keys, including what agents and
        members of your projects do.
      </p>
    ),
  },
  {
    heading: 'What agents may be told to do',
    body: (
      <>
        <p>
          Oya acts in websites on your instructions, with your sessions. You are responsible for those instructions and
          their results. Do not use the Service to:
        </p>
        <ul>
          <li>break the law, or the terms of a site in a way that exposes others to harm;</li>
          <li>access accounts, systems or data you are not authorised to use;</li>
          <li>send spam, run fraud, impersonate others, or harvest personal data without a lawful basis;</li>
          <li>attack, overload or probe the Service or any site, or get around our usage limits;</li>
          <li>
            build or train a competing product from the Service&apos;s output or code outside its open-source licence.
          </li>
        </ul>
        <p>We may stop a run, a browser or an account that breaks these rules, and will tell you why when we can.</p>
      </>
    ),
  },
  {
    heading: 'Your data',
    body: (
      <p>
        Your sessions, pages, recordings, tasks and other content stay yours. You give us permission to process them
        only to run, secure and support the Service for you, as the <Link href="/privacy">Privacy Policy</Link>{' '}
        describes. You confirm you have the right to give us the content and sessions you connect.
      </p>
    ),
  },
  {
    heading: 'AI output',
    body: (
      <p>
        Agents use AI models and can make mistakes: click the wrong thing, misread a page, or report something untrue.
        Review what matters before you rely on it, and keep a person in the loop for actions that cannot be undone, such
        as payments or sending messages. Third-party model providers you connect apply their own terms.
      </p>
    ),
  },
  {
    heading: 'Plans and payment',
    body: (
      <p>
        Paid plans are billed in advance through Stripe and renew until cancelled. You can cancel at any time; the plan
        runs to the end of the period already paid, and fees are not refunded except where the law requires. We may
        change prices with at least 30 days&apos; notice, starting from your next period. Taxes are yours.
      </p>
    ),
  },
  {
    heading: 'Open source and self-hosting',
    body: (
      <p>
        Parts of Oya are published under an open-source licence, which governs that code. Running the self-hosted
        edition beyond its free limits needs a licence from us; write to {mail}.
      </p>
    ),
  },
  {
    heading: 'Availability and changes',
    body: (
      <p>
        We work to keep the Service available and secure, but it may be interrupted for maintenance or reasons outside
        our control. We improve the Service continually and may change or retire features; if we retire one you pay for,
        we will give reasonable notice.
      </p>
    ),
  },
  {
    heading: 'Ending the agreement',
    body: (
      <p>
        You can stop using the Service and delete your account at any time. We may suspend or end your access if you
        break these terms or do not pay, or if the law requires it. When the agreement ends, your data is deleted as the
        Privacy Policy describes; ask {mail} beforehand if you need an export.
      </p>
    ),
  },
  {
    heading: 'Warranties',
    body: (
      <p>
        THE SERVICE IS PROVIDED &quot;AS IS&quot; AND &quot;AS AVAILABLE&quot;. TO THE EXTENT THE LAW ALLOWS, WE
        DISCLAIM ALL WARRANTIES, EXPRESS OR IMPLIED, INCLUDING MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
        NON-INFRINGEMENT, AND DO NOT PROMISE THAT AGENTS WILL COMPLETE TASKS CORRECTLY.
      </p>
    ),
  },
  {
    heading: 'Limitation of liability',
    body: (
      <p>
        TO THE EXTENT THE LAW ALLOWS, NEITHER PARTY IS LIABLE FOR INDIRECT, INCIDENTAL, SPECIAL OR CONSEQUENTIAL
        DAMAGES, OR LOST PROFITS, REVENUE OR DATA. EACH PARTY&apos;S TOTAL LIABILITY IS LIMITED TO THE GREATER OF THE
        FEES YOU PAID IN THE 12 MONTHS BEFORE THE CLAIM AND US$100. THESE LIMITS DO NOT APPLY TO YOUR PAYMENT
        OBLIGATIONS OR TO LIABILITY THAT CANNOT BE LIMITED BY LAW.
      </p>
    ),
  },
  {
    heading: 'Indemnity',
    body: (
      <p>
        You will defend and compensate Oya against third-party claims arising from your content or from instructions you
        gave agents in breach of these terms.
      </p>
    ),
  },
  {
    heading: 'Law and disputes',
    body: (
      <p>
        These terms are governed by the laws of {GOVERNING_LAW}, without regard to its conflict-of-law rules. Disputes
        are heard in the state or federal courts located in Delaware, and both parties consent to their jurisdiction.
        Nothing here removes rights a consumer has under the law of where they live.
      </p>
    ),
  },
  {
    heading: 'Changes and contact',
    body: (
      <p>
        We may update these terms. If a change matters, we will tell you by email or in the product at least 30 days
        before it applies; continuing to use the Service after that means you accept it. Questions go to {mail}.
      </p>
    ),
  },
];

/** The Terms of Service page. */
export default function TermsOfService() {
  return (
    <LegalPage
      title="Terms of Service"
      effective={EFFECTIVE_DATE}
      lead={<p>The agreement for using Oya Browser and Oya Cloud, in plain words where we could.</p>}
      sections={SECTIONS}
    />
  );
}
