/** Explicit local-only library import never creates remote personas or changes signed-in sessions. */
import fs from 'node:fs';
import path from 'node:path';
import { installedSources, sourceBrowser, type ChromiumSource } from './locate.ts';
import { readLibraryDetailed, type LibraryReadResult } from './library-read.ts';
import type { LibraryImportSource, LibraryImportResult } from '../../shared/ipc.ts';
import type { AppServices } from '../app/services.ts';

/** OS/file effects are injected so fixtures never read the person's source profiles. */
export interface LibraryImportSeams {
  /** Fresh source discovery for every listing and mutation. */
  sources(): ChromiumSource[];
  /** Profile reader reports separate part failures. */
  read(root: string, profile: string): LibraryReadResult;
  /** Canonical paths expose symlinks and Windows junctions before reads. */
  realpath(file: string): string;
}
/** Offline discovery does not need an executable to read profile files. */
function chromiumSources(): ChromiumSource[] {
  return installedSources()
    .map((item) => sourceBrowser(item.id))
    .filter((item): item is ChromiumSource => item?.kind === 'chromium');
}
/** Only direct profile directory names belong under a discovered user-data root. */
export function safeProfileId(value: string): boolean {
  return !!value && value !== '.' && value !== '..' && !/[\\/:\0]/.test(value);
}
/** Canonical containment refuses a directory or junction outside its source root. */
function contained(source: ChromiumSource, profile: string, realpath: LibraryImportSeams['realpath']): boolean {
  if (!safeProfileId(profile)) return false;
  try {
    const root = realpath(source.userDataDir);
    const relative = path.relative(root, realpath(path.join(source.userDataDir, profile)));
    return !!relative && !relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative);
  } catch {
    return false;
  }
}

/** Projection removes paths and unsafe profiles before exposing a selection to the renderer. */
function sourceChoice(source: ChromiumSource, realpath: LibraryImportSeams['realpath']): LibraryImportSource {
  const profiles = source.profiles
    .filter((p) => contained(source, p.dir, realpath))
    .map((p) => ({ id: p.dir, name: typeof p.name === 'string' ? p.name : p.dir }));
  return { id: source.id, name: source.name, sessions: process.platform === 'darwin', profiles };
}

/** Both fresh profile membership and canonical containment are required before reading. */
function selectable(source: ChromiumSource, profile: string, realpath: LibraryImportSeams['realpath']): boolean {
  return source.profiles.some((p) => p.dir === profile) && contained(source, profile, realpath);
}

/** Coordinates an explicit source/profile selection into one snapshotted local partition. */
export class LibraryImport {
  /** Library and profile identity remain owned by the existing services. */
  private readonly deps: Pick<AppServices, 'library' | 'persona'>;
  /** Read/discovery effects are replaceable in hermetic fixtures. */
  private readonly seams: LibraryImportSeams;
  /** Only one selected import may mutate the local library at a time. */
  private running = false;

  /** Production sources stay offline; tests provide scratch profiles and readers. */
  constructor(
    deps: Pick<AppServices, 'library' | 'persona'>,
    seams: LibraryImportSeams = { sources: chromiumSources, read: readLibraryDetailed, realpath: fs.realpathSync },
  ) {
    this.deps = deps;
    this.seams = seams;
  }

  /** Returns only safe selectable IDs/names, never a filesystem path. */
  sources(): LibraryImportSource[] {
    return this.seams
      .sources()
      .map((source) => sourceChoice(source, this.seams.realpath))
      .filter((source) => source.profiles.length > 0);
  }

  /** Revalidates the selection against current discovery before touching local data. */
  import(sourceId: string, profileId: string): LibraryImportResult {
    const partition = this.deps.persona.partitionName();
    const source = this.selected(sourceId, profileId);
    this.running = true;
    try {
      return this.readInto(source, profileId, partition);
    } finally {
      this.running = false;
    }
  }

  /** Fresh discovery prevents old UI IDs from becoming arbitrary filesystem reads. */
  private selected(sourceId: string, profileId: string): ChromiumSource {
    if (this.running) throw Error('An import is already running.');
    const source = this.seams.sources().find((item) => item.id === sourceId);
    if (!source || !selectable(source, profileId, this.seams.realpath))
      throw Error('Choose an available browser profile, then try again.');
    return source;
  }

  /** Counts only entries retained by the existing validation, deduplication and history cap. */
  private readInto(source: ChromiumSource, profile: string, partition: string): LibraryImportResult {
    const result = this.seams.read(source.userDataDir, profile);
    if (result.warnings.length && !result.bookmarks.length && !result.history.length)
      throw Error(result.warnings.join(' '));
    const counts = this.deps.library.importInto(partition, result);
    return { ...counts, warnings: result.warnings };
  }
}
