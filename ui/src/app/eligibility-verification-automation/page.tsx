/** /eligibility-verification-automation: The page on automating insurance eligibility checks on payer portals with an agent, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { ELIGIBILITY } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(ELIGIBILITY);

/** The page on automating insurance eligibility checks on payer portals with an agent, then a playbook. */
export default function EligibilityVerificationAutomation() {
  return <AnswerPage content={ELIGIBILITY} />;
}
