/**
 * The root layout: fonts, site-wide metadata and structured data, the
 * no-flash theme script, and the auth provider around every page.
 */
import type { PropsWithChildren } from 'react';
import type { Metadata } from 'next';
import { headers } from 'next/headers';
import { DM_Sans, Archivo_Black } from 'next/font/google';
import { AuthProvider } from '@/components/auth-provider';
import { SITE_URL, SITE_NAME, SITE_TAGLINE, SITE_DESCRIPTION } from '@/lib/site';
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
  title: { default: `${SITE_NAME} — ${SITE_TAGLINE}`, template: `%s · ${SITE_NAME}` },
  description: SITE_DESCRIPTION,
  applicationName: SITE_NAME,
  keywords: [
    'browser control plane',
    'AI agent browser',
    'headless Chrome API',
    'browser automation for LLMs',
    'MCP browser',
    'browser fingerprint persona',
    'Browserbase alternative',
    'agent browser infrastructure',
  ],
  alternates: { canonical: '/' },
  openGraph: {
    type: 'website',
    siteName: SITE_NAME,
    url: SITE_URL,
    title: `${SITE_NAME} — ${SITE_TAGLINE}`,
    description: SITE_DESCRIPTION,
    images: [{ url: '/oya-browser-poster.jpg', width: 1920, height: 1080, alt: `${SITE_NAME} fleet console` }],
  },
  twitter: {
    card: 'summary_large_image',
    title: `${SITE_NAME} — ${SITE_TAGLINE}`,
    description: SITE_DESCRIPTION,
    images: ['/oya-browser-poster.jpg'],
  },
  robots: { index: true, follow: true, 'max-image-preview': 'large', 'max-snippet': -1 },
};

/**
 * Structured data, kept to claims the site actually makes.
 *
 * No aggregateRating or review markup: inventing either would be fabricating
 * social proof, and search engines treat self-serving review markup as spam.
 */
const STRUCTURED_DATA = {
  '@context': 'https://schema.org',
  '@graph': [
    {
      '@type': 'WebSite',
      '@id': `${SITE_URL}/#website`,
      url: SITE_URL,
      name: SITE_NAME,
      description: SITE_DESCRIPTION,
      publisher: { '@id': `${SITE_URL}/#organization` },
    },
    {
      '@type': 'Organization',
      '@id': `${SITE_URL}/#organization`,
      name: 'Oya',
      url: SITE_URL,
      logo: `${SITE_URL}/icon.png`,
    },
    {
      '@type': 'SoftwareApplication',
      '@id': `${SITE_URL}/#app`,
      name: SITE_NAME,
      applicationCategory: 'DeveloperApplication',
      applicationSubCategory: 'Browser automation',
      operatingSystem: 'macOS, Windows, Linux',
      url: SITE_URL,
      downloadUrl: `${SITE_URL}/docs#download`,
      softwareHelp: `${SITE_URL}/docs`,
      description: SITE_DESCRIPTION,
      featureList: [
        'Drive real Chrome through one API across multiple browser providers',
        'Deterministic personas: fingerprint, cookies and proxy as one identity',
        'Provider failover without rewriting agent code',
        'Live view with human takeover mid-run',
        'MCP endpoint per browser for Claude Code, Claude Desktop and Cursor',
        'REST, WebSocket, JavaScript SDK and CLI',
        'Desktop sign-in that carries a logged-in session to remote browsers',
        'Self-hostable control plane',
      ],
      publisher: { '@id': `${SITE_URL}/#organization` },
    },
  ],
};

/** The document shell every page renders in. */
export default async function RootLayout({ children }: PropsWithChildren) {
  // Set by src/proxy.ts, which also sends the Content-Security-Policy this
  // nonce belongs to. Without it these two inline scripts do not run.
  const nonce = (await headers()).get('x-nonce') ?? undefined;
  return (
    <html lang="en" data-theme="dark" suppressHydrationWarning className={`${dmSans.variable} ${archivo.variable}`}>
      <head>
        <script
          nonce={nonce}
          dangerouslySetInnerHTML={{
            __html: `try{document.documentElement.dataset.theme=localStorage.getItem('oya_theme')==='light'?'light':'dark'}catch{}`,
          }}
        />
        <script
          nonce={nonce}
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(STRUCTURED_DATA) }}
        />
      </head>
      <body className="antialiased">
        <AuthProvider>{children}</AuthProvider>
      </body>
    </html>
  );
}
