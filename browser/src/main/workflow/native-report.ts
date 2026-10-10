/** Workflow evidence remains explicit when safe screenshot masking is unavailable. */
import type { WorkerMessage } from './runs.ts';
/** Preserve the existing evidence contract without capturing secret-bearing pixels. */
export function evidenceOmitted(stepId: string): WorkerMessage {
  const message = 'Screenshot omitted: safe masking cannot be guaranteed for this page.';
  return { type: 'event', event: { at: Date.now(), kind: 'evidence', stepId, message } };
}
