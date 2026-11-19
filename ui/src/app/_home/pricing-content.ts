/**
 * The plans on the home page. The three priced plans restate, figure for figure, the pricing answer in content.ts;
 * the enterprise plan restates the sign-in page's note to larger teams. Change a price in both places.
 */
import { ENTERPRISE_SIGNUP } from '@/components/auth/enterprise-nudge';

/** One plan card. */
export interface Plan {
  /** The plan's name. */
  name: string;
  /** Who it is for, in a line. */
  line: string;
  /** The price as shown. */
  price: string;
  /** What the price is per. */
  per: string;
  /** What the plan includes. */
  includes: string[];
  /** The button's label. */
  action: string;
  /** Where the button goes. */
  href: string;
  /** Drawn as the lit card. */
  lit?: boolean;
}

/** The plans, cheapest first, then the one priced by conversation. */
export const PLANS: Plan[] = [
  {
    name: 'Free',
    line: 'Run your first task on your own desktop.',
    price: '$0',
    per: 'to start',
    includes: [
      'The desktop app',
      '500 agent steps a month',
      '1 cloud browser hour',
      'Self-hosting, up to 5 cloud browsers at once',
    ],
    action: 'Download free',
    href: '/#download',
  },
  {
    name: 'Developer',
    line: 'Replay workflows in Oya Cloud.',
    price: '$20',
    per: 'a month',
    includes: ['100 cloud browser hours', '1 GB of proxy', '5,000 agent steps', 'Then $0.10 a browser hour'],
    action: 'Choose Developer',
    href: '/dashboard',
  },
  {
    name: 'Startup',
    line: 'Run a team’s portal work every day.',
    price: '$99',
    per: 'a month',
    includes: ['500 cloud browser hours', '5 GB of proxy', '50,000 agent steps', 'Then $0.08 a browser hour'],
    action: 'Choose Startup',
    href: '/dashboard',
    lit: true,
  },
  {
    name: 'Enterprise',
    line: 'Roll Oya out across a team or a company.',
    price: 'Custom',
    per: 'pricing',
    includes: [
      'HIPAA workloads under a signed BAA',
      'Self-hosting above 5 cloud browsers',
      'More hours, proxy and agent steps',
      'A call with the founders to scope it',
    ],
    action: 'Talk to sales',
    href: ENTERPRISE_SIGNUP,
  },
];

/** What every priced plan pays past its allowance. */
export const PLAN_NOTE = 'Past the allowance: $8 a GB of proxy and $0.002 an agent step.';
