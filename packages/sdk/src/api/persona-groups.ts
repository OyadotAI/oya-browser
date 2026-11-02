/**
 * `oya.personaGroups` (and its alias `oya.profileGroups`): named sets of personas
 * that `oya.browser.start({ personaGroup })` spreads its browsers across. A group's
 * name goes into a path through `segment`, which encodes it and refuses one that
 * would change the path, before any request.
 */
import { segment } from '../client.js';
import type { PersonaGroup } from '../types/index.js';
import type { HttpRef } from './shapes.js';

/** What GET /api/persona-groups answers. */
interface GroupList {
  /** The key's groups. */
  groups: PersonaGroup[];
}

/** The group calls, over the client's HTTP. */
export const personaGroupApi = (http: HttpRef) => ({
  /** Every group on this key. */
  list: async (): Promise<PersonaGroup[]> => (await http().request<GroupList>('GET', '/api/persona-groups')).groups,
  /**
   * A new group of this key's personas (not the default one). Its name is what a
   * start passes, so it cannot change later; a name already used is a 409.
   */
  create: (name: string, personaIds: string[] = []): Promise<PersonaGroup> =>
    http().request<PersonaGroup>('POST', '/api/persona-groups', { name, personaIds }),
  /** Replaces the group's members. */
  update: (name: string, personaIds: string[]): Promise<PersonaGroup> =>
    http().request<PersonaGroup>('PUT', `/api/persona-groups/${segment(name, 'name')}`, { personaIds }),
  /** Deletes the group; its personas and their browsers are not touched. */
  remove: async (name: string): Promise<void> => {
    await http().request('DELETE', `/api/persona-groups/${segment(name, 'name')}`);
  },
});
