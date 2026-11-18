/** Explicit paths and subject families give all22 answer pages an intentional visual treatment. */
import type { AnswerPresentation } from './presentation';

/** Configuration describes existing content; unknown pages fall back to complete prose. */
export const PRESENTATIONS: Record<string, AnswerPresentation> = {
  '/hipaa-compliant-browser': { family: 'controls', eyebrow: 'Healthcare & compliance' },
  '/agentic-browser': { family: 'fleet', eyebrow: 'Browser infrastructure' },
  '/vs/browserbase': { family: 'comparison', eyebrow: 'Compare approaches' },
  '/vs/steel': { family: 'comparison', eyebrow: 'Compare approaches' },
  '/vs/anchor': { family: 'comparison', eyebrow: 'Compare approaches' },
  '/vs/browser-use': { family: 'comparison', eyebrow: 'Compare approaches' },
  '/ai-browser': { family: 'desktop', eyebrow: 'Your browser, with an agent' },
  '/browser-for-healthcare': { family: 'controls', eyebrow: 'Healthcare & compliance' },
  '/hipaa-browser-automation': { family: 'controls', eyebrow: 'Healthcare & compliance' },
  '/prior-authorization-automation': { family: 'workflow', eyebrow: 'Healthcare workflows' },
  '/payer-portal-automation': { family: 'workflow', eyebrow: 'Healthcare workflows' },
  '/eligibility-verification-automation': {
    family: 'workflow',
    eyebrow: 'Healthcare workflows',
  },
  '/claim-status-automation': { family: 'workflow', eyebrow: 'Healthcare workflows' },
  '/denial-management-automation': { family: 'workflow', eyebrow: 'Healthcare workflows' },
  '/provider-enrollment-automation': { family: 'workflow', eyebrow: 'Healthcare workflows' },
  '/state-registry-automation': { family: 'workflow', eyebrow: 'Healthcare workflows' },
  '/browser-for-claude-code': {
    family: 'desktop',
    eyebrow: 'A browser for your agent',
  },
  '/ai-browser-chatgpt-claude': { family: 'desktop', eyebrow: 'Bring your AI plan' },
  '/best-browser-for-ai-agents': {
    family: 'comparison',
    eyebrow: 'Compare approaches',
  },
  '/best-payer-portal-automation': { family: 'comparison', eyebrow: 'Compare approaches' },
  '/best-ai-browser': { family: 'comparison', eyebrow: 'Compare approaches' },
  '/baa': { family: 'documents', eyebrow: 'Healthcare & compliance' },
};
