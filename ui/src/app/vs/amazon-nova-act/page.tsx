/** /vs/amazon-nova-act: Oya next to Amazon Nova Act, from each vendor's own public pages. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_NOVA_ACT } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_NOVA_ACT);

/** The Oya vs Amazon Nova Act page. */
export default function VsNovaAct() {
  return <AnswerPage content={VS_NOVA_ACT} />;
}
