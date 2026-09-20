/**
 * The control plane's durable state machine: projects, sessions and their
 * capacity, budgets and queue, credentials, human takeover, and webhooks. Every
 * change runs in a store transaction and emits the events deliveries are built from.
 *
 * This is the entry point; the operations live in service/, one concern per file.
 */
import { ControlService } from './service/control-service.ts';

export { instanceId, terminal, live, attachOnly, holdsSlot, hash, projectId, fault } from './service/model.ts';
export { validatePolicy } from './service/policy.ts';
export { WEBHOOK_EVENTS, SLACK_EVENTS } from './service/webhooks.ts';
export { ControlService };

/**
 * The API key a project was created with, by id — what background work uses when
 * it has a project and no caller. Null rather than throwing where the caller is a
 * worker or a redirect that can only drop the job, not report a 503 to anyone.
 */
export async function keyOfProject(id, service = control()) {
  const p = await service.store.get('project', id);
  if (!p?.key) return null;
  try {
    return service.projectKey(p);
  } catch {
    return null;
  }
}

let singleton;
/** The process-wide ControlService, created on first use. */
export const control = () => (singleton ||= new ControlService());
