/** /hipaa-browser-automation: The page that answers "what is HIPAA-compliant browser automation?" and how Oya runs it. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { HIPAA_AUTOMATION } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(HIPAA_AUTOMATION);

/** The page that answers "what is HIPAA-compliant browser automation?" and how Oya runs it. */
export default function HipaaBrowserAutomation() {
  return <AnswerPage content={HIPAA_AUTOMATION} />;
}
