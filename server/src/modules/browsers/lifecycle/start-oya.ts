/**
 * Starting Oya browsers: a governed one in the managed runtime, or one in an
 * Oya Cloud sandbox. Either dials in on its own once it is up.
 */
import { createSandbox, isConfigured as sandboxConfigured } from '../../../drivers/sandbox.ts';
import { createManaged } from '../../control/managed.ts';
import { sandboxMissing } from './sandbox-settings.ts';
import { started, starting, type Start } from './start-reply.ts';

/** What the caller is told about a sandbox browser. */
const JOIN_NOTE = 'The browser connects on its own; it appears in GET /browsers within ~90s.';

/** What the managed runtime is asked to run. */
const managedSpec = ({ req, key, persona }: Start) => ({
  apiKey: key,
  browserId: req.controlSession.id,
  persona: persona.id,
  name: req.body?.name,
  policies: req.controlSession.policies,
});

/** Starts a governed browser in the managed runtime. */
export async function launchManaged(start: Start) {
  const created = await createManaged(managedSpec(start));
  started(start, starting(start, created.browserId, { effective: created.runtime }));
}

/** What Oya Cloud is asked to launch. */
const sandboxSpec = ({ req, key, persona }: Start) => ({
  apiKey: key,
  name: req.body?.name,
  persona: persona.id,
  browserId: req.controlSession.id,
});

/** Oya browsers dial in on their own once the sandbox is up. */
export async function launchSandbox(start: Start) {
  if (!sandboxConfigured(start.key)) return sandboxMissing(start.res, start.key);
  const created = await createSandbox(sandboxSpec(start));
  started(start, starting(start, created.browserId, { note: JOIN_NOTE }));
}
