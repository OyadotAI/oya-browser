/**
 * The start page's words and example tasks.
 */

/** The example tasks: the task sent, and the button's label. */
export const EXAMPLES = [
  { task: 'Go to amazon.com and find Jordans under $150', label: 'Jordans under $150' },
  { task: 'Find the top story on Hacker News and tell me what it is about', label: 'Top story on Hacker News' },
  { task: 'Find the weather in San Francisco for this weekend', label: 'Weekend weather in SF' },
] as const;

/** What the start page says. */
export const TEXT = {
  sub: 'Give Oya a task. It works right here, with your logins, and you can take the wheel at any moment.',
  input: 'A task for Oya',
  placeholder: 'Ask Oya to do anything on the web',
  go: 'Start the task',
  examples: 'Example tasks',
} as const;

/** What an incognito window's start page says instead: what is and is not kept, and that it is not a new identity. */
export const INCOGNITO_TEXT = {
  title: 'Incognito',
  kept: "Incognito pages aren't saved to history, and their cookies and site data are deleted when you close every incognito window. Downloads and bookmarks you make here are kept.",
  identity:
    "This isn't a different identity. Sites see the same device and network as this profile, and signing in still tells them who you are. Oya's agents can't see or use this window.",
} as const;

/** The key that starts a task (Shift with it is a new line). */
export const SUBMIT_KEY = 'Enter';

/** The class that plays the page's arrival. */
export const ARRIVING = 'arriving';
