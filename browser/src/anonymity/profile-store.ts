/**
 * Profile store, persistent profile management on disk.
 * Profiles stored as JSON in app userData directory.
 */
import fs from 'node:fs';
import path from 'node:path';
import { PROFILE_JSON_INDENT } from './constants.ts';

/** A saved anonymity profile: its id, and whatever else the fingerprint holds. */
export interface StoredProfile {
  /** The profile's id, also its file name. */
  id: string;
  /** The fingerprint's own fields. */
  [field: string]: unknown;
}

/** Repository for anonymity profiles (`T` is the caller's profile shape): one JSON file each, plus which one is active. */
export class ProfileStore<T extends StoredProfile = StoredProfile> {
  /** The profiles folder. */
  readonly dir: string;

  /** Keeps profiles under `<basePath>/profiles`, creating the folder if needed. */
  constructor(basePath: string) {
    this.dir = path.join(basePath, 'profiles');
    fs.mkdirSync(this.dir, { recursive: true });
  }

  /** The file for a profile id; an id that could escape the folder is refused. */
  private profilePath(id: string): string {
    if (!/^[a-zA-Z0-9_-]+$/.test(id)) throw new Error('Invalid profile id');
    return path.join(this.dir, `${id}.json`);
  }

  /** Writes a profile and returns it. */
  save(profile: T): T {
    fs.writeFileSync(this.profilePath(profile.id), JSON.stringify(profile, null, PROFILE_JSON_INDENT));
    return profile;
  }

  /** A saved profile, or null when it is missing or unreadable. */
  get(id: string): T | null {
    const p = this.profilePath(id);
    if (!fs.existsSync(p)) return null;
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8')) as T;
    } catch {
      return null;
    }
  }

  /** The id of the profile in use, or null when none has been chosen. */
  getActiveId(): string | null {
    try {
      const data = JSON.parse(fs.readFileSync(path.join(this.dir, 'active.json'), 'utf8')) as Partial<
        Record<'activeId', string>
      >;
      return data.activeId || null;
    } catch {
      return null;
    }
  }

  /** Records which profile is in use. */
  setActiveId(id: string): void {
    fs.writeFileSync(path.join(this.dir, 'active.json'), JSON.stringify({ activeId: id }));
  }
}
