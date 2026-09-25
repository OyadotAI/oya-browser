/**
 * robots.txt: public pages are open to every crawler, including named AI
 * crawlers; the per-account console and the API are not.
 */
import type { MetadataRoute } from 'next';
import { SITE_URL } from '@/lib/site';

/**
 * Answer engines only cite what they are allowed to fetch, and several of them
 * use a separate crawler from the one that answers. Both are named explicitly
 * so a future blanket tightening of `*` does not silently drop them.
 *
 * The dashboard and live views are per-account and behind auth: nothing to
 * index, and crawling them only burns budget on redirects.
 */
const AI_AGENTS = [
  'GPTBot', // OpenAI training/index
  'OAI-SearchBot', // ChatGPT search results
  'ChatGPT-User', // fetches on a user's behalf in ChatGPT
  'ClaudeBot', // Anthropic index
  'Claude-User', // fetches on a user's behalf in Claude
  'Claude-SearchBot', // Claude search results
  'PerplexityBot', // Perplexity index
  'Perplexity-User', // fetches on a user's behalf in Perplexity
  'Google-Extended', // Gemini grounding
  'Applebot-Extended',
  'CCBot', // Common Crawl, feeds many models
  'cohere-ai',
  'DuckAssistBot',
  'Meta-ExternalAgent',
  'Bytespider',
  'Amazonbot', // Alexa and Rufus answers
  'MistralAI-User', // fetches on a user's behalf in Le Chat
  'Google-CloudVertexBot', // Vertex AI agents
  'YouBot', // You.com
];

/** Paths behind auth, not worth crawling. */
const DISALLOW = ['/dashboard', '/dashboard/', '/live/', '/api/'];

/** The rules, the sitemap and the canonical host. */
export default function robots(): MetadataRoute.Robots {
  return {
    rules: [
      { userAgent: '*', allow: '/', disallow: DISALLOW },
      ...AI_AGENTS.map((userAgent) => ({ userAgent, allow: '/', disallow: DISALLOW })),
    ],
    sitemap: `${SITE_URL}/sitemap.xml`,
    host: SITE_URL,
  };
}
