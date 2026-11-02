/**
 * What a persona group may be: its name, its member list and which fields a
 * request may carry. Every refusal is an HttpError with the status and a code a
 * client can branch on, since admission passes the code through as it is.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { GROUP_NAME_MAX } from './constants.ts';
import type { Persona } from './model.ts';

/** A group name: what a caller's code passes, so it stays a plain identifier. */
const GROUP_NAME = new RegExp(`^[A-Za-z0-9._-]{1,${GROUP_NAME_MAX}}$`);

/** The fields a create may carry; a change carries only the members. */
export const CREATE_FIELDS = ['name', 'personaIds'];
/** The fields a change may carry. */
export const REPLACE_FIELDS = ['personaIds'];

/** A refusal with its status and code. */
const refuse = (status: number, code: string, message: string) => new HttpError(status, message, { code });

/** A 400 for a request that is not what it should be. */
const invalid = (message: string) => refuse(Status.BAD_REQUEST, 'invalid_request', message);

/** Refuses a body that is not an object, or that carries a field the route does not take. */
export function onlyFields(body: unknown, fields: string[]): Record<string, unknown> {
  if (!body || typeof body !== 'object' || Array.isArray(body)) throw invalid('The body must be a JSON object');
  const extra = Object.keys(body).find((k) => !fields.includes(k));
  if (extra) throw invalid(`Unknown field: ${extra}`);
  return body as Record<string, unknown>;
}

/** The group name, or a 400. */
export function groupName(name: unknown): string {
  if (typeof name === 'string' && GROUP_NAME.test(name)) return name;
  throw invalid(`Group names are 1-${GROUP_NAME_MAX} letters, digits, dot, dash or underscore`);
}

/** The member ids as given, without repeats, or a 400 when they are not a list of strings. */
export function memberIds(ids: unknown): string[] {
  if (!Array.isArray(ids) || !ids.every((id) => typeof id === 'string'))
    throw invalid('personaIds must be a list of persona ids');
  return [...new Set(ids)];
}

/**
 * Checks one member: the caller's own persona, and not the default one. The
 * default persona is uncapped, so in a group it would always look free and take
 * every start; a group exists to spread starts.
 */
export function checkMember(persona: Persona | null, id: string) {
  if (!persona) throw refuse(Status.NOT_FOUND, 'not_found', `No such persona: ${id}`);
  if (persona.isDefault) throw invalid('The default persona cannot be in a group');
}

/** The answer for a group name the caller has none of; another key's group reads the same. */
export const noSuchGroup = (name: string) => refuse(Status.NOT_FOUND, 'not_found', `No such persona group: ${name}`);

/** The answer for a name the caller already has a group of. */
export const groupExists = (name: string) =>
  refuse(Status.CONFLICT, 'group_exists', `A persona group named ${name} already exists`);

/** The answer for a group whose members are all gone or were never added. */
export const emptyGroup = (name: string) =>
  invalid(`Persona group ${name} has no personas; add one with PUT /persona-groups/${name}`);

/** The tie-break between members: the one used longest ago first, a never-used one before all. */
export const byLastUse = (a: Persona, b: Persona) =>
  String(a.lastUsedAt || '').localeCompare(String(b.lastUsedAt || ''));
