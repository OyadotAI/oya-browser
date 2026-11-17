/** /agentic-browser: the page that answers "what is an agentic browser?", and says Oya Cloud and the enterprise edition are one for teams. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { AGENTIC_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(AGENTIC_BROWSER);

/** The page that answers "what is an agentic browser?". */
export default function AgenticBrowser() {
  return <AnswerPage content={AGENTIC_BROWSER} />;
}
