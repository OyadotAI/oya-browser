/**
 * A scripted page for the challenge solvers: `evaluate` answers each script
 * with the next of the answers the test gave for it, and records what ran.
 */
import { mock } from 'node:test';

/**
 * An evaluate() over scripted answers. Each rule is [script, answers]: the
 * answers are used in order and the last one repeats; an Error is thrown.
 */
export function scriptedPage(rules: [string, any[]][]) {
  const queues = rules.map(([script, answers]) => ({ script, answers: [...answers] }));
  return mock.fn(async (script: string) => {
    const rule = queues.find((q) => q.script === script);
    if (!rule) throw new Error(`unexpected script: ${script.slice(0, 60)}`);
    const next = rule.answers.length > 1 ? rule.answers.shift() : rule.answers[0];
    if (next instanceof Error) throw next;
    return next;
  });
}

/** The scripts an evaluate() mock was asked to run, in order. */
export const scriptsRun = (evaluate: any) => evaluate.mock.calls.map((c: any) => c.arguments[0]);
