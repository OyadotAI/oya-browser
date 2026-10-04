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

/** The key that starts a task (Shift with it is a new line). */
export const SUBMIT_KEY = 'Enter';

/** The class that plays the page's arrival. */
export const ARRIVING = 'arriving';
