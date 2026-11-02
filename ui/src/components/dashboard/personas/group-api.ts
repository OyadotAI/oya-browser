/**
 * Every request the Groups dialog makes: the key's persona groups, which
 * browser starts name with personaGroup to spread across several profiles.
 */
import { api } from '@/lib/api-client';

/** A persona group as the server answers it. */
export interface PersonaGroup {
  /** What a start passes as personaGroup; cannot be renamed. */
  name: string;
  /** Its member profiles that still exist. */
  personaIds: string[];
  /** When it was made (ISO). */
  createdAt: string;
  /** When its members last changed (ISO). */
  updatedAt: string;
}

/** What GET /persona-groups answers. */
interface GroupList {
  /** The key's groups. */
  groups: PersonaGroup[];
}

/** A path segment, escaped. */
const seg = encodeURIComponent;

/** The key's groups. */
export const listGroups = async (key: string) => (await api<GroupList>('/persona-groups', { key })).groups;

/** A new group. */
export const createGroup = (key: string, name: string, personaIds: string[]) =>
  api<PersonaGroup>('/persona-groups', { key, method: 'POST', body: { name, personaIds } });

/** Replaces a group's members. */
export const replaceGroup = (key: string, name: string, personaIds: string[]) =>
  api<PersonaGroup>(`/persona-groups/${seg(name)}`, { key, method: 'PUT', body: { personaIds } });

/** Deletes a group; its profiles are not touched. */
export const deleteGroup = (key: string, name: string) =>
  api(`/persona-groups/${seg(name)}`, { key, method: 'DELETE' });
