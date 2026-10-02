/**
 * The enterprise path of sign-up as plain data and functions: the questions
 * asked before a sales call, what makes the answers usable, and the Calendly
 * link that books the call with the answers already filled in. Calendly owns
 * the calendar and the confirmation, so there is no booking backend here.
 */
import { STATS, type PitchContent } from '@/components/auth/pitch';
import { foundersCall } from '../_home/content';

/** The roles a visitor can pick. */
export const ROLES = [
  'Engineering leader',
  'Platform / infrastructure',
  'Operations / revenue cycle',
  'Security / compliance',
  'Other',
];

/** The kinds of site the agents have to reach. */
export const PORTALS = ['Payer portals', 'EHRs', 'Registries', 'Other web portals'];

/** Where the browsers would run: id, label and the line under it. */
export const DEPLOYS = [
  { id: 'cloud', label: 'Oya Cloud', sub: 'Fully managed' },
  { id: 'self', label: 'Our cloud', sub: 'Self-hosted' },
  { id: 'unsure', label: 'Not sure yet', sub: "We'll help" },
];

/** Monthly run volumes. */
export const VOLUMES = ['Under 1,000', '1,000 – 10,000', '10,000 – 100,000', '100,000+', 'Not sure yet'];

/** Compliance and identity requirements. */
export const REQUIREMENTS = ['HIPAA', 'SOC 2', 'SSO'];

/** Why a team books a call, beside the questions. */
export const ENTERPRISE_PITCH: PitchContent = {
  kicker: 'OYA FOR ENTERPRISE',
  title: 'Run portal automation inside your own network.',
  lead:
    "Book 30 minutes with the founders. We'll scope the portals you need to reach, how you want to deploy, and " +
    'what your compliance team will ask for.',
  points: [
    [
      'Self-host in your own cloud',
      'Run the control plane on Docker, ECS, GCP or Kubernetes so HIPAA and SOC 2 workloads stay in your network.',
    ],
    [
      'Scale past 5 concurrent browsers',
      'Self-hosting is free up to 5 at once. Beyond that, we price for your volume.',
    ],
    [
      'An audit trail you can hand to security',
      'Hash-chained run history, and an allow-list of hosts each browser may reach.',
    ],
    [
      'Secrets the model never sees',
      'Credentials are typed through placeholders and redacted from everything the model reads.',
    ],
    ['No lock-in', 'Every playbook exports as Playwright code you keep.'],
  ],
  stats: STATS,
};

/** The answers to the questions before the call. */
export interface SalesDetails {
  /** Given name. */
  firstName: string;
  /** Family name. */
  lastName: string;
  /** Where the invite goes. */
  email: string;
  /** The company's name. */
  company: string;
  /** One of ROLES. */
  role: string;
  /** Picked from PORTALS. */
  portals: string[];
  /** One of DEPLOYS' ids. */
  deploy: string;
  /** One of VOLUMES. */
  volume: string;
  /** Picked from REQUIREMENTS. */
  requirements: string[];
  /** Free text. */
  notes: string;
}

/** Nothing answered yet; the selects start on their first sensible option. */
export const EMPTY_DETAILS: SalesDetails = {
  firstName: '',
  lastName: '',
  email: '',
  company: '',
  role: ROLES[0],
  portals: [],
  deploy: 'unsure',
  volume: VOLUMES[VOLUMES.length - 1],
  requirements: [],
  notes: '',
};

/** The first thing stopping the answers from booking a call, or '' when none. */
export function salesProblem(d: SalesDetails): string {
  if (!d.firstName.trim()) return 'First name is required';
  if (!d.email.trim()) return 'Work email is required';
  return d.company.trim() ? '' : 'Company is required';
}

/** `list` with `item` added, or removed when it is there already. */
export function toggled(list: string[], item: string): string[] {
  return list.includes(item) ? list.filter((x) => x !== item) : [...list, item];
}

/** Each line of the founders' note: the name they read it by, and its answer. */
const NOTE_LINES: Array<[string, (d: SalesDetails) => string]> = [
  ['Company', (d) => d.company.trim()],
  ['Role', (d) => d.role],
  ['Needs to reach', (d) => d.portals.join(', ')],
  ['Runs on', (d) => DEPLOYS.find((x) => x.id === d.deploy)?.label ?? d.deploy],
  ['Runs / month', (d) => d.volume],
  ['Requirements', (d) => d.requirements.join(', ')],
  ['Notes', (d) => d.notes.trim()],
];

/** The answers as one note for the founders, lines left out when unanswered. */
export function bookingNotes(d: SalesDetails): string {
  return NOTE_LINES.map(([k, answer]) => [k, answer(d)])
    .filter(([, v]) => v)
    .map(([k, v]) => `${k}: ${v}`)
    .join('\n');
}

/**
 * The inline Calendly page for the founders' call, prefilled. `a1` answers
 * the event's first custom question, so the notes land there; `embed_domain`
 * makes Calendly post its events back to this page.
 */
export function bookingUrl(d: SalesDetails, embedDomain: string): string {
  const name = `${d.firstName} ${d.lastName}`.trim();
  const q = { name, email: d.email.trim(), a1: bookingNotes(d), embed_domain: embedDomain, embed_type: 'Inline' };
  return `${foundersCall}?${new URLSearchParams({ ...q, hide_gdpr_banner: '1' })}`;
}

/** What Calendly posts from its frame. */
interface CalendlyMessage {
  /** The event's name, such as 'calendly.event_scheduled'. */
  event?: unknown;
}

/** Whether a message is Calendly saying the call was booked. */
export function isBooked(e: MessageEvent): boolean {
  const data = e.data as CalendlyMessage | null;
  return e.origin === 'https://calendly.com' && data?.event === 'calendly.event_scheduled';
}
