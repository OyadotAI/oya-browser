/** /claim-status-automation: The page on automating claim status checks on payer portals with an agent, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { CLAIM_STATUS } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(CLAIM_STATUS);

/** The page on automating claim status checks on payer portals with an agent, then a playbook. */
export default function ClaimStatusAutomation() {
  return <AnswerPage content={CLAIM_STATUS} />;
}
