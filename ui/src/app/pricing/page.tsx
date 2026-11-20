/**
 * Dedicated public /pricing page: renders all published plans and pricing/compliance
 * answers directly in SSR HTML without accordions or sign-in gates, alongside matching
 * BreadcrumbList and FAQPage structured data.
 */
import type { Metadata } from 'next';
import Link from 'next/link';
import landing from '../_home/landing.module.css';
import { Nav } from '../_home/nav';
import { Closing } from '../_home/closing';
import { PricingPlans } from '../_home/pricing';
import { PRICING_FAQ } from '../_home/pricing-content';
import { foundersCall } from '../_home/content';
import { SITE_NAME, SITE_ALTERNATE_TYPES } from '@/lib/site';
import { breadcrumbNode, faqNode, jsonLd, jsonLdGraph } from '@/lib/structured-data';
import styles from './pricing-page.module.css';

/** Page description shared across search snippets and social cards (153 chars). */
const DESCRIPTION =
  'Oya Browser pricing: free desktop app, free self-hosting up to 5 cloud browsers, Developer ($500/mo), Startup ($2,500/mo), and Enterprise with HIPAA BAA.';

/** Metadata and canonical URL for /pricing. */
export const metadata: Metadata = {
  title: 'Pricing',
  description: DESCRIPTION,
  alternates: { canonical: '/pricing', types: SITE_ALTERNATE_TYPES },
  openGraph: {
    type: 'website',
    siteName: SITE_NAME,
    url: '/pricing',
    title: `Pricing · ${SITE_NAME}`,
    description: DESCRIPTION,
    images: [{ url: '/oya-ask-poster.jpg', width: 1280, height: 800, alt: `${SITE_NAME} pricing` }],
  },
  twitter: {
    card: 'summary_large_image',
    title: `Pricing · ${SITE_NAME}`,
    description: DESCRIPTION,
    images: ['/oya-ask-poster.jpg'],
  },
};

/** Combined BreadcrumbList and FAQPage JSON-LD matching the visible page verbatim. */
const STRUCTURED_DATA = jsonLd(
  jsonLdGraph([
    breadcrumbNode([
      { name: 'Home', path: '/' },
      { name: 'Pricing', path: '/pricing' },
    ]),
    faqNode(PRICING_FAQ),
  ]),
);

/** Always-visible pricing and compliance questions, readable without JavaScript. */
function PricingFaq() {
  return (
    <section className={`section ${styles.faq}`} id="faq" aria-labelledby="pricing-faq-h">
      <div className="wrap">
        <div className={styles.faqHead}>
          <h2 id="pricing-faq-h">Pricing and compliance questions</h2>
          <a className="link" href={foundersCall}>
            Talk to the founders
          </a>
        </div>
        <div className={styles.faqList}>
          {PRICING_FAQ.map(({ q, a }) => (
            <div key={q} className={styles.faqRow}>
              <h3>{q}</h3>
              <p>{a}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/** The dedicated /pricing page. */
export default function PricingPage() {
  return (
    <div className={`${landing.page} ${styles.pricingRoot}`}>
      <Nav />
      <main id="top" tabIndex={-1}>
        <section className={styles.hero} aria-labelledby="pricing-page-h">
          <div className="wrap">
            <nav className={styles.breadcrumb} aria-label="Breadcrumb">
              <Link href="/">Home</Link>
              <span aria-hidden="true">/</span>
              <span aria-current="page">Pricing</span>
            </nav>
            <h1 id="pricing-page-h" className={styles.title}>
              Pricing
            </h1>
            <p className={styles.lead}>
              Start with one workflow. Choose capacity as it grows. The desktop app is free; choose a cloud plan when
              you’re ready to run workflows there.
            </p>
            <PricingPlans planHeadingAs="h2" />
          </div>
        </section>
        <PricingFaq />
      </main>
      <Closing invitation />
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: STRUCTURED_DATA }} />
    </div>
  );
}
