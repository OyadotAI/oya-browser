/**
 * Which sites a person allowed a device or notifications on, per profile, on disk.
 * Allow is remembered, as Chrome does, so a meeting site asks once, not at every join.
 * A device Block is never written, so a mis-click never locks someone out of their
 * calls; a notifications Block is, as 'notifications-blocked', or a mail site would
 * ask on every load. Forgetting the site clears both. Grants are keyed by partition,
 * so one profile never answers for another.
 */
import type { JsonFile } from './json-file.ts';

/** What a site can be allowed (the microphone, the camera, the screen, notifications), or refused notifications. */
export type SiteKind = 'audio' | 'video' | 'screen' | 'notifications' | 'notifications-blocked';

/** Partition, then origin, then what it may use. */
type Grants = Record<string, Record<string, SiteKind[]>>;

/** The remembered grants. Read on every call: the file is tiny and nothing can then go stale. */
export class SitePermissionStore {
  /** The file the grants live in. */
  private readonly file: JsonFile<Grants>;

  /** `file` is where the grants are kept. */
  constructor(file: JsonFile<Grants>) {
    this.file = file;
  }

  /** Every grant, or none when the file is missing or broken. */
  private all(): Grants {
    const grants = this.file.read();
    return grants && typeof grants === 'object' ? grants : {};
  }

  /** Whether `origin` may use `kind` in `partition`. */
  has(partition: string, origin: string, kind: SiteKind): boolean {
    const site = this.all()[partition];
    return !!site && Object.hasOwn(site, origin) && site[origin].includes(kind);
  }

  /** Remembers that `origin` may use `kinds` in `partition`. */
  grant(partition: string, origin: string, kinds: SiteKind[]): void {
    const all = this.all();
    const site = (all[partition] ??= {});
    site[origin] = [...new Set([...(Object.hasOwn(site, origin) ? site[origin] : []), ...kinds])];
    this.file.write(all);
  }

  /** Forgets every grant in `partition`; the sites ask again. */
  forget(partition: string): void {
    const all = this.all();
    delete all[partition];
    this.file.write(all);
  }

  /** The sites with a grant in `partition`. */
  origins(partition: string): string[] {
    return Object.keys(this.all()[partition] ?? {});
  }
}
