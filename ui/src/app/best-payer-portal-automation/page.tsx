/** /best-payer-portal-automation: Oya next to the payer portal automation vendors, from each vendor's own pages. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { BEST_PORTAL } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(BEST_PORTAL);

/** Oya next to the payer portal automation vendors, from each vendor's own pages. */
export default function BestPayerPortalAutomation() {
  return <AnswerPage content={BEST_PORTAL} />;
}
