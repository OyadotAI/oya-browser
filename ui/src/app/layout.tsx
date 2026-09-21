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
  title: { default: `${SITE_NAME}, ${SITE_TAGLINE}`, template: `%s · ${SITE_NAME}` },
  description: SITE_DESCRIPTION,
  applicationName: SITE_NAME,
  keywords: [
    'AI agent browser',
    'record and replay browser automation',
    'browser automation without an LLM in the loop',
    'undetected browser automation',
    'agent browser sign-in and session reuse',
    'MCP browser',
    'browser fingerprint persona',
    'Browserbase alternative',
  ],
  alternates: { canonical: '/' },
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
        'Playbooks: record an agent run once, replay it with new inputs and no model in the loop',
        'Every playbook exports as a readable Playwright module you own',
        'A changed page is repaired into a draft playbook a person promotes',
        'Automation inside the browser process, not attached over the debugging protocol',
        'Desktop sign-in that carries cookies and localStorage to every browser on that persona',
        'Personas: fingerprint, cookies and proxy as one identity that never changes',
        'Hash-chained audit trail and host allow-listing',
        'Live view with human takeover mid-run for CAPTCHA and 2FA',
        'MCP endpoint for Claude Code, Claude Desktop and Cursor',
        'REST, WebSocket, JavaScript SDK, CLI and a CDP URL Playwright connects to',
        'Runs on Oya Cloud, Browserbase, Steel, Anchor, Browser Use or your own Chrome',
        'Self-hostable with one installer command',
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
