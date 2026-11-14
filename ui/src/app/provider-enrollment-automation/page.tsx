/** /provider-enrollment-automation: The page on keeping provider enrollment and payer rosters current with an agent, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { ENROLLMENT } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(ENROLLMENT);

/** The page on keeping provider enrollment and payer rosters current with an agent, then a playbook. */
export default function ProviderEnrollmentAutomation() {
  return <AnswerPage content={ENROLLMENT} />;
}
