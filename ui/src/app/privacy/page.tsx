/**
 * The Privacy Policy: what Oya collects, why, who else receives it, how long it
 * is kept, and how a person gets it deleted. It says only what the product does;
 * the list of subprocessors lives on the trust center so it is kept in one place.
 */
import type { Metadata } from 'next';
import Link from 'next/link';
import { LegalPage, type LegalSection } from '../_legal/legal-page';
import { COMPANY, EFFECTIVE_DATE, LEGAL_EMAIL, RECORDING_DAYS } from '../_legal/content';

/** The page's title and description for search and link previews. */
export const metadata: Metadata = {
  title: 'Privacy Policy',
  description: 'What Oya Browser and Oya Cloud collect, why, who processes it, and how to have it deleted.',
  alternates: { canonical: '/privacy' },
};

/** The email link used throughout. */
const mail = <a href={`mailto:${LEGAL_EMAIL}`}>{LEGAL_EMAIL}</a>;

/** The policy, section by section. */
const SECTIONS: readonly LegalSection[] = [
  {
    heading: 'Who we are',
    body: (
      <p>
        {COMPANY} (&quot;Oya&quot;, &quot;we&quot;) operates Oya Browser, the desktop app, and Oya Cloud, the hosted
        service at oyabrowser.com. This policy covers both. If you run Oya on your own servers, your data stays in your
        infrastructure and you are its controller; this policy then covers only the website and the desktop app&apos;s
        update checks.
      </p>
    ),
  },
  {
    heading: 'What we collect',
    body: (
      <ul>
        <li>
          <strong>Account details.</strong> Your email address, name, and sign-in method, and your multi-factor settings
          if you turn them on.
        </li>
        <li>
          <strong>Projects and keys.</strong> Your projects, their settings and members. API keys are stored only as
          hashes.
        </li>
        <li>
          <strong>Browser sessions you choose to sync.</strong> When you connect the desktop app, the cookies and site
          storage of the profile you use are sent to your project, so agents and cloud browsers arrive signed in.
          Passwords you import from a password manager go to the same sealed sign-in store.
        </li>
        <li>
          <strong>Logins you send from the browser extension.</strong> When you press Send in the Oya extension for
          Chrome or Firefox, it reads the cookies of the site you are on, only after you allow that site, and sends them
          to the profile you picked in your project, over HTTPS to oyabrowser.com and nowhere else. It runs nothing in
          the background, and your API key stays in that browser.
        </li>
        <li>
          <strong>What agents and cloud browsers do.</strong> The pages they visit, the tasks you give them, and, when
          you record, screen recordings of their runs.
        </li>
        <li>
          <strong>Billing.</strong> Your plan and billing contact. Card details are handled by Stripe, not stored by us.
        </li>
        <li>
          <strong>Usage and errors.</strong> Product analytics tied to your account, never URLs or page content, and
          error reports with request bodies, headers and cookies removed.
        </li>
      </ul>
    ),
  },
  {
    heading: 'How we use it',
    body: (
      <ul>
        <li>To run the service: sign you in, run your browsers and agents, and keep your sessions working.</li>
        <li>To bill you for paid plans.</li>
        <li>To secure the service, investigate abuse, and keep an audit trail of privileged actions.</li>
        <li>To understand which features are used and fix what breaks.</li>
        <li>To write to you about your account and, if you agree, about product news.</li>
      </ul>
    ),
  },
  {
    heading: 'What we never do',
    body: (
      <ul>
        <li>We do not sell your personal data or share it for advertising.</li>
        <li>We do not train AI models on your pages, sessions, recordings or tasks.</li>
        <li>We do not read your synced sessions or recordings except to support you, at your request.</li>
      </ul>
    ),
  },
  {
    heading: 'AI models',
    body: (
      <p>
        When an agent works, the page content it reads and your instructions are sent to the model provider that runs
        it: OpenAI, Anthropic or Google when Oya&apos;s key is used, or the provider and account you connected yourself,
        under that provider&apos;s terms.
      </p>
    ),
  },
  {
    heading: 'Who else receives data',
    body: (
      <p>
        We use a small set of service providers for hosting, sign-in, cloud browsers, models, billing, error reports and
        analytics. Each receives only what its job needs. The current list, with what each one receives, is on our{' '}
        <Link href="/trust#subprocessors">trust center</Link>. We disclose data to authorities only when the law
        requires it, and tell you when we are allowed to.
      </p>
    ),
  },
  {
    heading: 'Security',
    body: (
      <p>
        Data is encrypted in transit. Credentials, MFA secrets, cookies, proxy credentials and recordings are sealed
        with AES-256-GCM and bound to their project. Privileged actions go to a tamper-evident audit log. The{' '}
        <Link href="/trust">trust center</Link> describes these controls in detail.
      </p>
    ),
  },
  {
    heading: 'How long we keep it',
    body: (
      <ul>
        <li>Recordings are deleted after {RECORDING_DAYS} days.</li>
        <li>A deleted project is erased after a one-day grace period, so a mistaken delete can be undone.</li>
        <li>Deleting your account deletes your projects, keys, profile and sign-in.</li>
        <li>Audit records and billing records are kept as long as the law or security requires.</li>
      </ul>
    ),
  },
  {
    heading: 'Your choices and rights',
    body: (
      <p>
        You can see and change your data in the dashboard, disconnect the desktop app at any time, and delete your
        account from its settings. Depending on where you live, you may also have the right to a copy of your data, to
        correct it, to object to or restrict its use, and to complain to your data protection authority. Write to {mail}{' '}
        and we will answer within 30 days.
      </p>
    ),
  },
  {
    heading: 'International transfers',
    body: (
      <p>
        Oya is a US company, and your data may be processed in the United States and wherever our service providers
        operate. When we move personal data from the EU, UK or Switzerland, we rely on the European Commission&apos;s
        standard contractual clauses or an equivalent safeguard.
      </p>
    ),
  },
  {
    heading: 'Cookies on our website',
    body: (
      <p>
        oyabrowser.com uses cookies to keep you signed in and to measure how the site is used. Cloudflare checks
        visitors at sign-up to keep bots out. We do not use advertising cookies.
      </p>
    ),
  },
  {
    heading: 'Children',
    body: (
      <p>
        Oya is for businesses and adults. It is not meant for anyone under 16, and we do not knowingly collect their
        data.
      </p>
    ),
  },
  {
    heading: 'Changes and contact',
    body: (
      <p>
        If we change this policy in a way that matters, we will tell you by email or in the product before it takes
        effect. Questions, requests and complaints go to {mail}.
      </p>
    ),
  },
];

/** The Privacy Policy page. */
export default function PrivacyPolicy() {
  return (
    <LegalPage
      title="Privacy Policy"
      effective={EFFECTIVE_DATE}
      lead={
        <p>
          Oya runs browsers that sign in to your accounts. Here is what we collect to do that, and what we never do.
        </p>
      }
      sections={SECTIONS}
    />
  );
}
