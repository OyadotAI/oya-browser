/** /best-ai-browser: Oya's desktop app next to the AI browsers people shortlist, from each vendor's own pages. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { BEST_AI_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(BEST_AI_BROWSER);

/** Oya's desktop app next to the AI browsers people shortlist, from each vendor's own pages. */
export default function BestAiBrowser() {
  return <AnswerPage content={BEST_AI_BROWSER} />;
}
