/**
 * Persona groups: a named set of one key's personas that a browser start names
 * instead of one persona, so its starts spread across the set. This service keeps
 * the groups and lists a start's members; which member the start runs as is
 * chosen inside the control plane's reservation, under the project lock, from the
 * sessions that hold slots there (control/service/capacity.ts leastLoaded).
 */
import type { GroupRepository, PersonaGroup } from './group-repository.ts';
import type { Persona } from './model.ts';
import { DEFAULT_MAX_CONCURRENT } from './constants.ts';
import {
  byLastUse,
  checkMember,
  CREATE_FIELDS,
  emptyGroup,
  groupExists,
  groupName,
  memberIds,
  noSuchGroup,
  onlyFields,
  REPLACE_FIELDS,
} from './group-rules.ts';

/** One member a start may run as, with the cap the reservation holds it to. */
export interface PersonaChoice {
  /** The member's persona id. */
  persona: string;
  /** Its concurrency cap; null when uncapped. */
  personaLimit: number | null;
}

/** What the group service needs from the persona service. */
export interface GroupPersonas {
  /** The caller's persona with this id, or null. */
  get(apiKey: string, id: string): Persona | null;
}

/** What the group service is built with. */
export interface GroupDeps {
  /** Where groups are kept. */
  repository: GroupRepository;
  /** The personas they name. */
  personas: GroupPersonas;
  /** The fingerprint a key's groups are owned by. */
  ownerOf: (apiKey: string) => string;
}

/** A group as the API answers it: its live members only, and never its owner. */
export interface GroupView {
  /** Its name. */
  name: string;
  /** Its members that still exist. */
  personaIds: string[];
  /** When it was made. */
  createdAt: string;
  /** When its members last changed. */
  updatedAt: string;
}

/** A persona's cap as the reservation takes it: a number, or null for none. */
const limitOf = (p: Persona) => {
  const cap = p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT;
  return Number.isFinite(cap) ? cap : null;
};

/** Creates, lists, changes and deletes groups, and lists the members a start may run as. */
export class GroupService {
  /** Storage, the persona service and key ownership. */
  declare private readonly deps: GroupDeps;
  constructor(deps: GroupDeps) {
    this.deps = deps;
  }

  /** The caller's groups. */
  async list(apiKey: string): Promise<GroupView[]> {
    const groups = await this.deps.repository.list(this.deps.ownerOf(apiKey));
    return groups.map((g) => this.view(apiKey, g));
  }

  /** A new group; 409 group_exists when the caller has one of that name. */
  async create(apiKey: string, input: unknown): Promise<GroupView> {
    const group = this.newGroup(apiKey, onlyFields(input, CREATE_FIELDS));
    if (!(await this.deps.repository.insert(group))) throw groupExists(group.name);
    return this.view(apiKey, group);
  }

  /** The group a create body describes, its name and members checked. */
  private newGroup(apiKey: string, body: Record<string, unknown>): PersonaGroup {
    const now = new Date().toISOString();
    const [name, personaIds] = [groupName(body.name), this.members(apiKey, body.personaIds)];
    return { owner: this.deps.ownerOf(apiKey), name, personaIds, createdAt: now, updatedAt: now };
  }

  /** Replaces a group's members; 404 when the caller has no such group. */
  async replace(apiKey: string, name: string, input: unknown): Promise<GroupView> {
    const personaIds = this.members(apiKey, onlyFields(input, REPLACE_FIELDS).personaIds);
    if (!(await this.deps.repository.replace(this.deps.ownerOf(apiKey), name, personaIds))) throw noSuchGroup(name);
    return this.view(apiKey, await this.owned(apiKey, name));
  }

  /** Deletes a group; 404 when the caller has no such group. */
  async remove(apiKey: string, name: string): Promise<void> {
    if (!(await this.deps.repository.remove(this.deps.ownerOf(apiKey), name))) throw noSuchGroup(name);
  }

  /**
   * The members a start on this group may run as, the one used longest ago
   * first: the order the reservation breaks ties in. Members deleted since the
   * group was saved, or by another replica, are left out; 400 when none is left.
   */
  async choices(apiKey: string, name: string): Promise<PersonaChoice[]> {
    const members = this.live(apiKey, await this.owned(apiKey, name)).sort(byLastUse);
    if (!members.length) throw emptyGroup(name);
    return members.map((p) => ({ persona: p.id, personaLimit: limitOf(p) }));
  }

  /** The caller's group of that name, or 404. */
  private async owned(apiKey: string, name: string): Promise<PersonaGroup> {
    const group = await this.deps.repository.get(this.deps.ownerOf(apiKey), name);
    if (!group) throw noSuchGroup(name);
    return group;
  }

  /** The members a request names, each checked; an absent list is none. */
  private members(apiKey: string, ids: unknown): string[] {
    const personaIds = memberIds(ids ?? []);
    for (const id of personaIds) checkMember(this.deps.personas.get(apiKey, id), id);
    return personaIds;
  }

  /** The group's members that still exist and are still not the default persona. */
  private live(apiKey: string, group: PersonaGroup): Persona[] {
    const members = group.personaIds.map((id) => this.deps.personas.get(apiKey, id));
    return members.filter((p): p is Persona => Boolean(p) && !p.isDefault);
  }

  /** A group as the API answers it, members that were deleted left out. */
  private view(apiKey: string, g: PersonaGroup): GroupView {
    const personaIds = this.live(apiKey, g).map((p) => p.id);
    return { name: g.name, personaIds, createdAt: g.createdAt, updatedAt: g.updatedAt };
  }
}
