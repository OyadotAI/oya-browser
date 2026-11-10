/** /ai-browser: The page that answers "what is an AI browser?", and says Oya's desktop app is a free one. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { AI_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(AI_BROWSER);

/** The page that answers "what is an AI browser?", and says Oya's desktop app is a free one. */
export default function AiBrowser() {
  return <AnswerPage content={AI_BROWSER} />;
}
