/** Public plan details restate the existing FAQ and billing allowances without changing billing. */
import { foundersCall } from './content';

/** One visible plan gives visitors a price, allowance and honest next step. */
export interface PublicPlan {
  /** The billing plan's public name. */
  name: string;
  /** The existing monthly price, shown as published. */
  price: string;
  /** The unit or commitment accompanying the price. */
  period: string;
  /** The work this plan helps a visitor start. */
  description: string;
  /** Monthly usage allowances and any published browser-hour overage. */
  includes: string[];
  /** A navigation action, never a direct subscription. */
  action: string;
  /** Where the visitor can evaluate or begin using the plan. */
  href: string;
}

/** Prices and overages retain the existing public FAQ and previously published per-plan mapping. */
export const PUBLIC_PLANS: PublicPlan[] = [
  {
    name: 'Free',
    price: '$0',
    period: 'to start',
    description: 'Try your first task on desktop.',
    includes: ['Free desktop app', '500 agent steps a month', '1 cloud browser hour'],
    action: 'Download free',
    href: '/#download',
  },
  {
    name: 'Developer',
    price: '$500',
    period: '/month',
    description: 'Take your workflow to the cloud.',
    includes: ['100 cloud browser hours', '1 GB of proxy', '5,000 agent steps', 'Additional browser hours: $0.10/hour'],
    action: 'Choose Developer',
    href: '/dashboard',
  },
  {
    name: 'Startup',
    price: '$2,500',
    period: '/month',
    description: 'Run more of your team’s portal work.',
    includes: [
      '500 cloud browser hours',
      '5 GB of proxy',
      '50,000 agent steps',
      'Additional browser hours: $0.08/hour',
    ],
    action: 'Choose Startup',
    href: '/dashboard',
  },
  {
    name: 'Enterprise',
    price: 'Custom',
    period: 'pricing',
    description: 'Scope the capacity your team needs.',
    includes: ['For larger teams', 'Self-hosting above 5 cloud browsers', 'PHI workloads under a signed BAA'],
    action: 'Book a meeting',
    href: foundersCall,
  },
];

/** Shared usage charges apply only to paid plans after their included allowances. */
export const PAID_USAGE_NOTE = 'Paid-plan usage beyond these allowances: $8/GB of proxy and $0.002/agent step.';
