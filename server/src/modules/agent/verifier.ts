/**
 * A check before a run is accepted as done. A separate, short model call reads the
 * task, the agent's report, the calls it made, and the page it ended on (and a
 * screenshot when nothing secret could be on it), and says whether the report is
 * supported. Agents claim success they did not reach; a checker that looks at the
 * page catches it, and the run goes on with the reason instead of ending wrong.
 */
import { chatCompletion } from '../../platform/llm.ts';
import { sendCommand } from '../browsers/socket.ts';
import { takeScreenshot } from './screenshot.ts';
import { analysisText } from './element-index.ts';
import { redact } from './placeholders.ts';
import { PAGE_FORMAT, VERIFY_CALLS_SHOWN, VERIFY_PAGE_CHARS } from './constants.ts';

/** What the checker is told. */
const CHECKER = `You check a browser agent's work before it is accepted. You get the task, the agent's final report, the calls it made, and the page it ended on.
Decide whether the report is supported by what the page shows: the task was done as asked, or the answer is right and complete. Judge only from the evidence; a claim the page does not show is not supported.
Answer with one JSON object and nothing else: {"verdict":"pass"} or {"verdict":"fail","reason":"<one concrete, checkable problem, and what to do about it>"}.
Fail only for a concrete problem (a missing step, a wrong value, an unconfirmed submission, an incomplete answer), never for style.`;

/** The verdict: pass, or fail with the reason the run goes on with. */
export type Verdict = {
  /** Whether the report stands. */
  pass: boolean;
  /** Why it does not, as the agent is told. */
  reason?: string;
};

/** The task the run was given: its first user message. */
const taskOf = (messages: any[]) => String(messages.find((m) => m.role === 'user')?.content ?? '');

/** The last calls the run made, one per line. */
function callsOf(messages: any[]) {
  const calls = messages.flatMap((m) =>
    (m.tool_calls || []).map((tc) => `${tc.function?.name} ${tc.function?.arguments || ''}`),
  );
  return calls.slice(-VERIFY_CALLS_SHOWN).join('\n');
}

/** The page the run ended on, as text, capped. */
async function pageNow(ctx) {
  const r = await sendCommand(ctx.browserId, 'analyze', { format: PAGE_FORMAT }).catch(() => null);
  if (!r?.ok) return '(the page could not be read)';
  return redact(analysisText(r.data), ctx.secrets).slice(0, VERIFY_PAGE_CHARS);
}

/** Everything the checker is told about the run, redacted: the checker is a model like any other. */
async function evidenceFor(ctx, messages: any[], answer: string) {
  const said = `TASK:\n${taskOf(messages)}\n\nREPORT:\n${answer}\n\nCALLS MADE:\n${callsOf(messages)}`;
  return `${redact(said, ctx.secrets)}\n\nPAGE NOW:\n${await pageNow(ctx)}`;
}

/** The checker's question: the evidence as text, and the screenshot when one may be taken. */
async function question(ctx, messages: any[], answer: string) {
  const text = await evidenceFor(ctx, messages, answer);
  const shot = await takeScreenshot(ctx.browserId, ctx.secrets).catch(() => ({ image: undefined }));
  if (!shot.image) return text;
  return [
    { type: 'text', text },
    { type: 'image_url', image_url: { url: shot.image } },
  ];
}

/** The verdict in the checker's reply; one that does not parse passes, so a checker's slip never blocks a run. */
function verdictOf(reply: string): Verdict {
  try {
    const parsed = JSON.parse(/\{[\s\S]*\}/.exec(reply)?.[0] ?? '{}');
    return parsed.verdict === 'fail'
      ? { pass: false, reason: String(parsed.reason || 'the report is not supported by the page') }
      : { pass: true };
  } catch {
    return { pass: true };
  }
}

/** Asks the checker about `answer`; `bill` counts its tokens like any other call of the run. */
export async function verify(ctx, messages: any[], answer: string, bill: (usage: any) => void): Promise<Verdict> {
  const { baseUrl, openaiKey, model } = ctx.llm;
  const checking = [
    { role: 'system', content: CHECKER },
    { role: 'user', content: await question(ctx, messages, answer) },
  ];
  const completion = await chatCompletion({ baseUrl, apiKey: openaiKey, model, messages: checking });
  bill(completion.usage);
  return verdictOf(String(completion.choices?.[0]?.message?.content ?? ''));
}

/** What the agent is told when the check fails. */
export const recheck = (reason: string) =>
  `Before this is accepted, a check of the page found a problem: ${reason}\nFix it if you can, then report again. If you cannot, report FAILED: with what blocked you.`;
