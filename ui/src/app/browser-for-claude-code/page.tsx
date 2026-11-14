/** /browser-for-claude-code: The page on giving Claude Code, Codex and Cursor a browser of their own. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { CODING_AGENT_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(CODING_AGENT_BROWSER);

/** The page on giving Claude Code, Codex and Cursor a browser of their own. */
export default function BrowserForClaudeCode() {
  return <AnswerPage content={CODING_AGENT_BROWSER} />;
}
