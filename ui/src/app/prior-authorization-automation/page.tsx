/** /prior-authorization-automation: The page on automating prior authorization on payer portals with an agent, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { PRIOR_AUTH } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(PRIOR_AUTH);

/** The page on automating prior authorization on payer portals with an agent, then a playbook. */
export default function PriorAuthorizationAutomation() {
  return <AnswerPage content={PRIOR_AUTH} />;
}
