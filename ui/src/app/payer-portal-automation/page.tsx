/** /payer-portal-automation: The page on automating payer portal work that has no API: an agent does it once, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { PAYER_PORTALS } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(PAYER_PORTALS);

/** The page on automating payer portal work that has no API: an agent does it once, then a playbook. */
export default function PayerPortalAutomation() {
  return <AnswerPage content={PAYER_PORTALS} />;
}
