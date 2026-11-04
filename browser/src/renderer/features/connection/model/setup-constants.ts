/** Stable first-task copy and bounded guide steps shared by setup and Settings. */
export { AGENT_TEST_PROMPT as FIRST_TASK } from '../../../../shared/agent-setup.ts';
/** The guide names existing control-plane surfaces without claiming actions are automatically verified. */
export const SETUP_GUIDE = [
  { title: 'Find your browser', detail: 'Open Browsers and select this desktop.', scene: 'guide-browser' },
  {
    title: 'Choose your workspace',
    detail: 'Check your project. Find API access in Settings.',
    scene: 'guide-workspace',
  },
  {
    title: 'See what’s open',
    detail: 'Select this desktop to view its page and activity.',
    scene: 'guide-page',
  },
  {
    title: 'Take over anytime',
    detail: 'In this browser’s panel, click Take control.',
    scene: 'guide-human',
  },
] as const;
