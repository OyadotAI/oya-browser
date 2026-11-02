/**
 * What a browser start runs as, read from its body: one persona (an id,
 * 'default' or 'auto'), or a persona group whose members the reservation picks
 * from. The group may be named personaGroup or profileGroup, matching the
 * persona/profile pair; naming a group and a persona together is refused.
 */
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import type { StartTarget } from './admission.ts';

/** What a start's target is resolved with. */
export interface StartAsDeps {
  /** Resolves a persona id, 'default' or 'auto'. */
  personas: { resolve(apiKey: string, persona?: string | null): any };
  /** Lists a group's members. */
  groups: { choices(apiKey: string, name: string): Promise<StartTarget['personaChoices']> };
}

/** A 400 for a start body that names its target wrongly. */
const invalid = (message: string) => new HttpError(Status.BAD_REQUEST, message, { code: 'invalid_request' });

/** The group a body names, or undefined for none; 400 for two different names or one that is not text. */
function groupOf(body): string | undefined {
  const [group, alias] = [body?.personaGroup, body?.profileGroup];
  if (group != null && alias != null && group !== alias) throw invalid('Pass personaGroup or profileGroup, not both');
  const name = group ?? alias;
  if (name == null) return undefined;
  if (typeof name !== 'string' || !name) throw invalid('personaGroup must be a group name');
  return name;
}

/** The start's target: its persona, or its group's members. */
export async function startAs(deps: StartAsDeps, key: string, body): Promise<StartTarget> {
  const group = groupOf(body);
  if (group === undefined) return { persona: deps.personas.resolve(key, body?.profile || body?.persona) };
  if (body?.profile || body?.persona) throw invalid('Pass personaGroup or persona, not both');
  return { personaChoices: await deps.groups.choices(key, group) };
}
