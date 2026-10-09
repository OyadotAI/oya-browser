/** Only validated isolated-recorder output is attributed to browser-owned document paths. */
import type { NativeRecordingDocument } from '../native/index.ts';
import type { PageOutput, RecordedStep } from './types.ts';
/** Reject malformed output rather than storing object-coerced secrets or untyped actions. */
function recorderOutput(value: unknown): PageOutput {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid native recording output');
  const data = value as PageOutput;
  if (data.steps !== undefined && (!Array.isArray(data.steps) || !data.steps.every(recordedAction)))
    throw Error('Invalid native recording steps');
  requireSecrets(data.secrets);
  return data;
}
/** Step actions must be real strings; the domain normalizer enforces their supported semantics. */
function recordedAction(step: unknown): step is RecordedStep {
  return (
    !!step && typeof step === 'object' && !Array.isArray(step) && typeof (step as RecordedStep).action === 'string'
  );
}
/** Payload-supplied frame selectors never override immutable authorization-time attribution. */
export function nativeRecordingOutput(value: unknown, document: NativeRecordingDocument): PageOutput {
  const data = recorderOutput(value);
  const frames = [...document.frames];
  return { ...data, steps: data.steps?.map((step) => ({ ...step, frames })) };
}

/** Secret descriptors are names only, never renderer-supplied objects or values. */
function requireSecrets(secrets: unknown): void {
  if (secrets !== undefined && (!Array.isArray(secrets) || !secrets.every((name) => typeof name === 'string')))
    throw Error('Invalid native recording secrets');
}
