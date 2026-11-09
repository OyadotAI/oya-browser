/** Enterprise capability summaries use shipped control-plane behavior and retain runtime boundaries. */
import { repository } from './content';

/** Three buyer questions organize detail without hiding the platform's breadth. */
export const CAPABILITY_GROUPS = [
  {
    id: 'orchestrate',
    title: 'Orchestrate',
    summary: 'Operate the fleet, not individual scripts.',
    detail: 'Execution providers, queues, concurrency, estimated budgets, playbooks and scheduled routines.',
    items: [
      ['Execution choice', 'Oya Cloud, self-hosted browsers and connected providers behind common interfaces.'],
      ['Sessions and capacity', 'Durable session state, queued admission and project concurrency controls.'],
      [
        'Repeatable work',
        'Playbooks, variables and scheduled routines, with estimated budgets for managed provisioning.',
      ],
    ],
    href: '/docs#control-plane',
  },
  {
    id: 'govern',
    title: 'Govern',
    summary: 'Set the boundaries agents work within.',
    detail:
      'Roles, scoped credentials, stable personas, sealed credentials, host policies and model-provider controls.',
    items: [
      ['Project access', 'Administrator, operator and viewer roles, with scoped, revocable credentials.'],
      [
        'Identity and authentication',
        'Persistent personas, cookies and proxies, plus sealed passwords and TOTP secrets.',
      ],
      ['Data and host policy', 'Host rules in the managed runtime and project model-provider allowlists.'],
    ],
    href: `${repository}/blob/main/docs/control-plane.md#access-and-human-control`,
  },
  {
    id: 'observe',
    title: 'Observe',
    summary: 'See the run. Intervene when needed.',
    detail: 'Live view, human control, hash-chained audit history and lifecycle webhooks.',
    items: [
      ['Human control', 'Watch sessions, take control and explicitly resume the agent when you are ready.'],
      ['Audit evidence', 'Trace actions to actors, roles and keys, with hash-chained history and export.'],
      ['Operational events', 'Signed lifecycle webhooks, delivery retries and manual replay for your systems.'],
    ],
    href: `${repository}/blob/main/docs/control-plane.md#budgets-audit-and-recordings`,
  },
];
