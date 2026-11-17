/** Existing navigation destinations, kept separate from disclosure state and presentation. */
/** The main navigation words and their destinations. */
export const WORDS = [
  { word: 'Control plane', href: '#control-plane' },
  { word: 'Workflows', href: '#workflows' },
  { word: 'Compare', href: '/vs' },
  { word: 'Research', href: '/research' },
  { word: 'Audit', href: '#audit' },
  { word: 'Docs', href: '/docs' },
  { word: 'Questions', href: '#faq' },
];

/** The product menu's original groups, descriptions and links. */
export const PRODUCT = [
  {
    group: 'Browsers',
    links: [
      ['AI browser', 'Free desktop app, an agent in a tab', '/ai-browser'],
      ['Agentic browser', 'Many browsers, run by agents', '/agentic-browser'],
      ['HIPAA-compliant browser', 'Oya Cloud and enterprise, with a BAA', '/hipaa-compliant-browser'],
    ],
  },
  {
    group: 'Playbooks',
    links: [
      ['Record once', 'A run saved as Playwright code', '#playbooks'],
      ['Three steps', 'Run, save, replay', '#steps'],
      ['Audit trail', 'Every action hash-chained', '#audit'],
    ],
  },
  {
    group: 'Start',
    links: [
      ['Console', 'Browsers, profiles, playbooks', '/dashboard'],
      ['Docs', 'SDK, MCP and self-hosting', '/docs'],
      ['Security', 'HIPAA, BAAs, the audit', '/trust'],
    ],
  },
];
