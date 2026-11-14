/** /ai-browser-chatgpt-claude: The page on an AI browser that runs on the ChatGPT or Claude plan you already pay for. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { PLAN_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(PLAN_BROWSER);

/** The page on an AI browser that runs on the ChatGPT or Claude plan you already pay for. */
export default function AiBrowserChatgptClaude() {
  return <AnswerPage content={PLAN_BROWSER} />;
}
