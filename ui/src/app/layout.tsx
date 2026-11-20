/**
 * The root layout: fonts, site-wide metadata and structured data, the
 * no-flash theme script, and the auth provider around every page.
 */
import type { PropsWithChildren } from 'react';
import type { Metadata } from 'next';
import { headers } from 'next/headers';
import { DM_Sans, Archivo_Black } from 'next/font/google';
import { AuthProvider } from '@/components/auth-provider';
import { ImpersonationBanner } from '@/components/impersonation-banner';
import { Analytics } from '@/components/analytics';
import { Rb2b } from '@/components/rb2b';
import { MetaPixel } from '@/components/meta-pixel';
import { VisitorSource } from '@/components/visitor-source';
import { ErrorReporting } from '@/components/error-reporting';
import { sentryDsn } from '@/lib/sentry';
import { validRb2bId } from '@/lib/rb2b';
import { validMetaPixelId } from '@/lib/meta-pixel';
import { SITE_URL, SITE_NAME, SITE_TAGLINE, SITE_DESCRIPTION, SITE_ALTERNATE_TYPES } from '@/lib/site';
import { jsonLd, siteStructuredData } from '@/lib/structured-data';
import { THEME_SCRIPT } from '@/lib/theme-script';
import './globals.css';

/** The body font. */
const dmSans = DM_Sans({
  subsets: ['latin'],
  display: 'swap',
  variable: '--font-dm-sans',
  weight: ['400', '500', '600', '700'],
});

/** The display font. */
const archivo = Archivo_Black({
  subsets: ['latin'],
  display: 'swap',
  variable: '--font-archivo',
  weight: '400',
});

/** Titles, description, canonical, social cards and robots for the whole site. */
export const metadata: Metadata = {
  // Without metadataBase, Next emits relative og:image URLs and most crawlers
  // and preview fetchers drop them.
  metadataBase: new URL(SITE_URL),
  title: { default: `${SITE_NAME}, ${SITE_TAGLINE}`, template: `%s · ${SITE_NAME}` },
  description: SITE_DESCRIPTION,
  applicationName: SITE_NAME,
  // <link rel="alternate"> to machine-readable docs and the feed so crawlers and agents find them from any page.
  alternates: {
    canonical: '/',
    types: SITE_ALTERNATE_TYPES,
  },
  openGraph: {
    type: 'website',
    siteName: SITE_NAME,
    url: SITE_URL,
    title: `${SITE_NAME}, ${SITE_TAGLINE}`,
    description: SITE_DESCRIPTION,
    images: [{ url: '/oya-ask-poster.jpg', width: 1280, height: 800, alt: `${SITE_NAME}, asked to do a task` }],
  },
  twitter: {
    card: 'summary_large_image',
    title: `${SITE_NAME}, ${SITE_TAGLINE}`,
    description: SITE_DESCRIPTION,
    images: ['/oya-ask-poster.jpg'],
  },
  robots: { index: true, follow: true, 'max-image-preview': 'large', 'max-snippet': -1 },
};

/**
 * Structured data, kept to claims the site actually makes.
 *
 * No aggregateRating or review markup: inventing either would be fabricating
 * social proof, and search engines treat self-serving review markup as spam.
 */
const STRUCTURED_DATA = jsonLd(siteStructuredData());

/**
 * Where product analytics goes, read at request time from the server's own
 * settings, or null when the operator set none. Read here and not at build
 * time because one image serves hosted and self-host consoles alike, and a
 * key baked into the bundle would turn analytics on for everyone.
 */
function analyticsSettings() {
  const { POSTHOG_KEY: key, POSTHOG_HOST: host } = process.env;
  return key && host ? { key, host: host.replace(/\/+$/, '') } : null;
}

/** The RB2B account to load on public pages, read at request time like PostHog's settings; null when unset or malformed. */
function rb2bId() {
  const id = process.env.RB2B_ID?.trim();
  return validRb2bId(id) ? id : null;
}

/** The Meta pixel to run on public pages, read at request time like RB2B's account; null when unset or malformed. */
function metaPixelId() {
  const id = process.env.META_PIXEL_ID?.trim();
  return validMetaPixelId(id) ? id : null;
}

/** The document shell every page renders in. */
export default async function RootLayout({ children }: PropsWithChildren) {
  // Set by src/proxy.ts, which also sends the Content-Security-Policy this
  // nonce belongs to. Without it these two inline scripts do not run.
  const nonce = (await headers()).get('x-nonce') ?? undefined;
  const analytics = analyticsSettings();
  const rb2b = rb2bId();
  const metaPixel = metaPixelId();
  const sentry = sentryDsn();
  return (
    <html lang="en" data-theme="dark" suppressHydrationWarning className={`${dmSans.variable} ${archivo.variable}`}>
      <head>
        <script
          nonce={nonce}
          dangerouslySetInnerHTML={{
            __html: THEME_SCRIPT,
          }}
        />
        <script nonce={nonce} type="application/ld+json" dangerouslySetInnerHTML={{ __html: STRUCTURED_DATA }} />
      </head>
      <body className="antialiased">
        <AuthProvider>
          <ImpersonationBanner />
          {children}
          {analytics && <Analytics posthogKey={analytics.key} host={analytics.host} />}
          {rb2b && <Rb2b id={rb2b} />}
          {metaPixel && <MetaPixel id={metaPixel} />}
          <VisitorSource />
          {sentry && <ErrorReporting dsn={sentry} />}
        </AuthProvider>
      </body>
    </html>
  );
}
