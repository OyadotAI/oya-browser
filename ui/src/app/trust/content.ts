/**
 * The words on the trust page: where Oya stands on compliance, how the service is
 * secured, and who processes customer data. Kept apart from the page so a status
 * change or a new vendor is one edit here, and nothing on the page claims more
 * than this file says.
 */
import { repository } from '../_home/content';

/** Where BAA requests and security questionnaires go. */
export const SALES_EMAIL = 'sales@getoya.ai';

/** Where a vulnerability is reported privately. */
export const REPORT_VULNERABILITY = `${repository}/security/advisories/new`;

/** The self-hosting guide. */
export const SELF_HOST_GUIDE = `${repository}/blob/main/docs/self-hosting.md`;

/** The generated evidence pack an auditor reads. */
export const EVIDENCE_PACK = `${repository}/blob/main/compliance/EVIDENCE.md`;

/** Each compliance programme and where it stands today, worded so nothing is overclaimed. */
export const STATUS = [
  {
    title: 'HIPAA, self-hosted',
    state: 'Available',
    body: 'Run the control plane and browsers in your own cloud. Pages, recordings and audit records stay in your infrastructure.',
  },
  {
    title: 'HIPAA, Oya Cloud',
    state: 'In progress',
    body: 'A per-project HIPAA mode, with business associate agreements across our subprocessors. Write to us to be told when it opens.',
  },
  {
    title: 'Evidence pack',
    state: 'Every release',
    body: 'Each HIPAA Security Rule control is mapped to the code that implements it and a check that runs against it. Gaps are listed, not hidden.',
  },
] as const;

/** How the service protects customer data, each one implemented in the code base. */
export const PRACTICES = [
  {
    title: 'Encryption',
    body: 'TLS in transit. Credentials, MFA seeds, cookies, proxy credentials and recordings are sealed with AES-256-GCM, bound to their tenant, with key rotation.',
  },
  {
    title: 'Tamper-evident audit trail',
    body: 'Every privileged action is written to a hash-chained log. The database refuses edits and deletes, and the chain head is anchored outside it.',
  },
  {
    title: 'Access control',
    body: 'TOTP multi-factor sign-in, owner, operator and viewer roles per project, API keys stored only as hashes, and every staff sign-in as a customer audited.',
  },
  {
    title: 'Network containment',
    body: 'A project can limit the hosts its browsers may reach. Anything off the list is refused before it leaves.',
  },
  {
    title: 'Retention and deletion',
    body: 'Recordings expire after 7 days by default. Audit records keep a retention floor. Deleting a project or an account erases its data.',
  },
  {
    title: 'Minimal telemetry',
    body: 'Error reports drop request bodies, headers and cookies and scrub emails. Product analytics never carries URLs or page content.',
  },
] as const;

/** Third parties that process customer data for Oya Cloud, and what each one receives. */
export const SUBPROCESSORS = [
  { name: 'Google Cloud', purpose: 'Hosting of the control plane', data: 'All service traffic' },
  { name: 'Supabase', purpose: 'Database, sign-in and recording storage', data: 'Account data, settings, recordings' },
  { name: 'Daytona', purpose: 'Cloud browser sandboxes', data: 'Pages visited by cloud browsers' },
  {
    name: 'OpenAI, Anthropic, Google',
    purpose: 'Agent models, when Oya’s key is used',
    data: 'Page content the agent reads',
  },
  { name: 'Stripe', purpose: 'Billing', data: 'Billing contact and payment details' },
  { name: 'Sentry', purpose: 'Error reporting', data: 'Scrubbed error reports' },
  { name: 'PostHog', purpose: 'Product analytics', data: 'Account identifier and email' },
  { name: 'Cloudflare', purpose: 'Bot check at sign-up', data: 'Visitor IP and browser signals' },
  { name: 'Slack', purpose: 'Internal sign-up and billing alerts', data: 'Account email' },
] as const;
