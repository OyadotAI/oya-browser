/** /denial-management-automation: The page on working claim denials and filing appeals on payer portals with an agent, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { DENIALS } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(DENIALS);

/** The page on working claim denials and filing appeals on payer portals with an agent, then a playbook. */
export default function DenialManagementAutomation() {
  return <AnswerPage content={DENIALS} />;
}
