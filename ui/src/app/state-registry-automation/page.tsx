/** /state-registry-automation: The page on immunization, licence and sanction lookups in state registries with an agent, then a playbook. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { REGISTRIES } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(REGISTRIES);

/** The page on immunization, licence and sanction lookups in state registries with an agent, then a playbook. */
export default function StateRegistryAutomation() {
  return <AnswerPage content={REGISTRIES} />;
}
