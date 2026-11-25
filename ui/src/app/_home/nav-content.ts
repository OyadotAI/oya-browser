/** Existing navigation destinations, kept separate from disclosure state and presentation. */
/** The main navigation words and their destinations. */
export const WORDS = [
  { word: 'Workflows', href: '/#workflows' },
  { word: 'Pricing', href: '/pricing' },
  { word: 'Docs', href: '/docs' },
  { word: 'Security', href: '/trust' },
  { word: 'Sign in', href: '/login' },
];

/** The product menu's original groups, descriptions and links. */
export const PRODUCT = [
  {
    group: 'Browsers',
    links: [
      ['AI browser', 'Free desktop app, an agent in a tab', '/ai-browser'],
      ['For coding agents', 'Connect Claude Code, Codex or Cursor', '/browser-for-claude-code'],
      ['Agentic browser', 'Many browsers, run by agents', '/agentic-browser'],
      ['HIPAA-compliant browser', 'Oya Cloud and enterprise, with a BAA', '/hipaa-compliant-browser'],
    ],
  },
  {
    group: 'Playbooks',
    links: [
      ['Record once', 'A run saved as Playwright code', '/#playbooks'],
      ['Three steps', 'Run, save, replay', '/#steps'],
      ['Control plane', 'Operate, govern and observe workflows', '/#control-plane'],
      ['Audit trail', 'Every action hash-chained', '/#audit'],
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
