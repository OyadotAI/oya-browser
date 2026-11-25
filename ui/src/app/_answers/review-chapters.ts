/** Security and comparison stories turn a reader's task into a review of scope, evidence and fit. */
import type { QA } from '@/lib/site';
import type { AnswerSection } from './content';

/** Shared FAQ answer on self-hosting the enterprise and community editions. */
export const SELF_HOST_FAQ: QA = {
  q: 'Can I self-host a HIPAA-compliant agentic browser?',
  a: 'Yes, with the enterprise edition: it runs on Docker, ECS, GCP or Kubernetes, comes with a BAA, and keeps HIPAA workloads in your own infrastructure. Write to sales@getoya.ai. The source-available community edition self-hosts too, without the HIPAA offering. Every playbook exports as Playwright code you keep.',
};
/** The covered edition is established before the reader sees operating controls. */
export const REGULATED_OPENING: AnswerSection[] = [
  {
    heading: 'Follow the task’s data path.',
    body: 'Identify the pages, browser runtime, recordings and model services the task may involve. Use that scope to review the controls.',
  },
  {
    heading: 'Choose the covered edition.',
    body: 'Oya’s HIPAA offering applies to Oya Cloud and the enterprise edition. The source-available community edition and the desktop app do not include that offering.',
    layout: 'choices',
  },
];
/** Evidence review is a prerequisite to using PHI rather than a certification claim. */
export const REGULATED_CLOSING: AnswerSection = {
  heading: 'Review the evidence before you begin.',
  body: 'Request the Business Associate Agreement (BAA) and supporting security evidence. Review their stated scope, dates and qualifications with your team.',
};
/** The decision criteria follow from the task rather than from an unsupported winner claim. */
export const COMPARISON_REQUIREMENTS: AnswerSection = {
  heading: 'Compare the requirements that follow from that task.',
  body: 'Look at agent connections, reuse of completed work, deployment and human intervention. If protected health information (PHI) is involved, check the covered edition and Business Associate Agreement (BAA).',
  layout: 'controls',
};
/** The table retains its original cells, sources and review date. */
export const PUBLISHED_DIFFERENCES: AnswerSection = {
  heading: 'Check the published differences.',
  body: 'The table below uses the linked vendor pages and their stated review date. “Not stated” means the pages reviewed did not say; it does not establish that a capability is absent.',
  layout: 'comparison',
};
/** Comparison is a starting point for evaluation, without a fabricated winner or outcome. */
export const COMPARISON_CLOSING: AnswerSection = {
  heading: 'Test the fit with your own work.',
  body: 'Use the comparison to shortlist an approach, then evaluate it against a representative workflow.',
};
